use super::{binding, AccountingRelation, Binding};
use crate::jobs::{JobError, JobResult};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::jobs::JobSnapshot;
use rusqlite::{params, Connection, OptionalExtension};

/// Private, process-local evidence gathered by the daemon from native parent
/// links before database admission. There is no wire decoder for this type.
#[derive(Debug, Clone)]
pub struct LineageEvidence {
    observer: ProcessIdentity,
    ancestors: Vec<ProcessIdentity>,
    complete: bool,
    child: Option<ProcessIdentity>,
}
impl LineageEvidence {
    pub fn gather(
        observer: &ProcessIdentity,
        child: Option<&ProcessIdentity>,
        chain: fn(&ProcessIdentity) -> (Vec<ProcessIdentity>, bool),
        owned_child: fn(&ProcessIdentity, &ProcessIdentity) -> bool,
    ) -> Self {
        let live = probe_live(observer) == IdentityStatus::VerifiedMatch;
        let (ancestors, complete) = if live {
            chain(observer)
        } else {
            (vec![], false)
        };
        let valid = ancestors.len() <= 128
            && ancestors
                .iter()
                .all(|a| a.is_valid() && a.boot_id == observer.boot_id && a != observer)
            && ancestors
                .iter()
                .enumerate()
                .all(|(i, a)| !ancestors[..i].contains(a));
        Self {
            observer: observer.clone(),
            complete: complete && valid && live,
            ancestors: if valid && live { ancestors } else { vec![] },
            child: child.filter(|c| live && owned_child(c, observer)).cloned(),
        }
    }
    #[cfg(test)]
    pub(super) fn simulated(
        observer: &ProcessIdentity,
        ancestors: Vec<ProcessIdentity>,
        complete: bool,
        child: Option<ProcessIdentity>,
    ) -> Self {
        Self {
            observer: observer.clone(),
            ancestors,
            complete,
            child,
        }
    }
}

fn native_root(
    connection: &Connection,
    job: &JobSnapshot,
    proof: &LineageEvidence,
) -> JobResult<Option<String>> {
    // Closest matching native ancestor wins; resolve through its already bound
    // canonical root. A stale/ambiguous/unverified candidate fails closed.
    for ancestor in &proof.ancestors {
        let mut statement = connection.prepare(
            "SELECT l.job_id FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
             WHERE j.id!=?4 AND j.state IN ('REGISTERED','LAUNCHING','RUNNING','OUTCOME_UNKNOWN')
             AND ((l.observer_pid=?1 AND l.observer_start_token=?2 AND l.observer_boot_id=?3)
               OR (l.child_pid=?1 AND l.child_start_token=?2 AND l.child_boot_id=?3
                 AND l.child_verified_at_ms IS NOT NULL)) LIMIT 2",
        )?;
        let ids = statement
            .query_map(
                params![
                    ancestor.pid,
                    ancestor.start_token,
                    ancestor.boot_id,
                    job.job_id.as_str()
                ],
                |r| r.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        if ids.len() > 1 {
            return Ok(None);
        }
        if let Some(id) = ids.first() {
            return Ok(connection
                .query_row(
                    "SELECT root_job_id FROM job_meter_summaries
                 WHERE job_id=?1 AND relation IN ('canonical_root','included_in_root')",
                    [id],
                    |row| row.get(0),
                )
                .optional()?
                .flatten());
        }
    }
    Ok(proof.complete.then(|| job.job_id.as_str().to_owned()))
}
pub(super) fn bind(
    connection: &Connection,
    job: &JobSnapshot,
    proof: &LineageEvidence,
) -> JobResult<()> {
    if proof.observer != job.observer {
        return Err(JobError::InvalidRequest);
    }
    let prior = binding(connection, job.job_id.as_str())
        .map_err(|_| JobError::StorageFailure)?
        .ok_or(JobError::StorageFailure)?;
    if prior.report_json.is_some() || prior.expired {
        return Ok(());
    }
    let mut root = native_root(connection, job, proof)?;
    // Traffic's independently verified reservation root must agree. It may
    // itself sit below a lifecycle-only accounting root, so resolve that link.
    let traffic_root: Option<String> = connection
        .query_row(
            "SELECT root_job_id FROM traffic_entries WHERE job_id=?1",
            [job.job_id.as_str()],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if let Some(traffic_root) = traffic_root {
        let resolved: Option<String> = connection
            .query_row(
                "SELECT root_job_id FROM job_meter_summaries WHERE job_id=?1
             AND relation IN ('canonical_root','included_in_root')",
                [traffic_root],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if resolved.is_none() || resolved != root {
            root = None;
        }
    }
    // Reparenting or contradictory evidence cannot silently establish a new
    // additive root after an earlier verified binding.
    if prior.root.is_some() && prior.root != root {
        root = None;
    }
    let relation = match &root {
        None => "unverified",
        Some(root) if root == job.job_id.as_str() => "canonical_root",
        Some(_) => "included_in_root",
    };
    let owned = job.child_verified_at_ms.is_some()
        && job.child.is_some()
        && job.child.as_ref() == proof.child.as_ref();
    connection.execute(
        "UPDATE job_meter_summaries SET relation=?2,root_job_id=?3,
         child_bound=MAX(child_bound,?4) WHERE job_id=?1 AND report_id IS NULL",
        params![job.job_id.as_str(), relation, root, owned],
    )?;
    Ok(())
}
pub(super) fn matches(binding: &Binding, relation: &AccountingRelation, id: &str) -> bool {
    match relation {
        AccountingRelation::Unverified {} => true, // Explicitly non-additive.
        AccountingRelation::CanonicalRoot { root_job_id } => {
            binding.relation == "canonical_root"
                && root_job_id == id
                && binding.root.as_ref() == Some(root_job_id)
        }
        AccountingRelation::IncludedInRoot { root_job_id } => {
            binding.relation == "included_in_root" && binding.root.as_ref() == Some(root_job_id)
        }
    }
}
