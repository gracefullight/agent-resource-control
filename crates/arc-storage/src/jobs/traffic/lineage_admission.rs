//! Verified native lineage and existing reservation lookup, reused at grant and launch.
use crate::jobs::{get, JobError as E, JobResult};
use arc_ipc::{
    jobs::{JobId, JobSnapshot, ProcessIdentity},
    traffic::LineageContext,
};
use rusqlite::{params, Connection};

/// An active ancestry anchor and the root that owns its reservation. The
/// `root` field is the candidate job snapshot, including for a descendant; its
/// name is retained for callers of the original single-snapshot interface.
/// No context is returned, and a snapshot alone is not proof of ancestry.
#[derive(Debug, Clone)]
pub struct LineageSnapshot {
    pub root: JobSnapshot,
    pub permit_job_id: JobId,
    pub generation: u64,
}

/// The daemon constructs this only after verifying the authenticated socket
/// peer's native ancestry to `anchor`. Storage rechecks the ancestry, revision,
/// anchor, root, generation and active permit under the write transaction.
#[derive(Debug, Clone)]
pub struct LineageProof {
    pub(super) root_id: JobId,
    anchor_job_id: JobId,
    revision: u64,
    pub(super) generation: u64,
    anchor: ProcessIdentity,
    observed_owner: Option<ProcessIdentity>,
    recheck: fn(&ProcessIdentity, &ProcessIdentity) -> bool,
}
impl LineageProof {
    /// Freeze an independently repeated native observation before submitting to
    /// the writer. Governor decisions recheck only durable anchors/revisions
    /// inside IMMEDIATE; never perform native inspection while holding SQLite.
    pub fn observe_before_write(mut self, observer: &ProcessIdentity) -> Option<Self> {
        if !(self.recheck)(observer, &self.anchor) {
            return None;
        }
        self.observed_owner = Some(observer.clone());
        self.recheck = |_, _| true;
        Some(self)
    }

    pub(in crate::jobs::traffic) fn matches(&self, root: &JobId, generation: u64) -> bool {
        self.root_id == *root && self.generation == generation
    }
    #[cfg(test)]
    pub fn verified(snapshot: &LineageSnapshot, anchor: &ProcessIdentity) -> Self {
        Self::verified_with_recheck(snapshot, anchor, |_, _| true)
    }

    /// The daemon supplies its native ancestry check for the final recheck
    /// while the writer transaction is held. The claim is never this proof.
    pub fn verified_with_recheck(
        snapshot: &LineageSnapshot,
        anchor: &ProcessIdentity,
        recheck: fn(&ProcessIdentity, &ProcessIdentity) -> bool,
    ) -> Self {
        Self {
            root_id: snapshot.permit_job_id.clone(),
            anchor_job_id: snapshot.root.job_id.clone(),
            revision: snapshot.root.revision,
            generation: snapshot.generation,
            anchor: anchor.clone(),
            recheck,
            observed_owner: None,
        }
    }
}

pub fn lineage_snapshot(
    connection: &Connection,
    context: &LineageContext,
) -> JobResult<Option<LineageSnapshot>> {
    Ok(lineage_snapshots(connection, context)?.into_iter().next())
}

/// Return every active candidate in the lineage. A daemon must check native
/// ancestry against each candidate: an active sibling is not necessarily an
/// ancestor of the requesting process. The permit owner can be terminal while
/// a descendant keeps its reservation active.
pub fn lineage_snapshots(
    connection: &Connection,
    context: &LineageContext,
) -> JobResult<Vec<LineageSnapshot>> {
    let mut statement = connection.prepare(
        "SELECT candidate.job_id, root.job_id, root.generation \
         FROM traffic_entries root \
         JOIN permits p ON p.job_id=root.job_id AND p.state='ACTIVE' AND p.generation=root.generation \
         JOIN traffic_entries candidate ON (candidate.job_id=root.job_id OR candidate.root_job_id=root.job_id) \
         WHERE root.context=?1 AND root.root_job_id IS NULL \
           AND candidate.state IN ('RESERVED','RUNNING','RETAINED') \
           AND candidate.generation=root.generation \
         ORDER BY candidate.sequence",
    )?;
    let mut rows = statement.query([context.as_str()])?;
    let mut snapshots = Vec::new();
    while let Some(row) = rows.next()? {
        let id = JobId::try_from(row.get::<_, String>(0)?).map_err(|_| E::StorageFailure)?;
        let permit_job_id =
            JobId::try_from(row.get::<_, String>(1)?).map_err(|_| E::StorageFailure)?;
        let generation = u64::try_from(row.get::<_, i64>(2)?).map_err(|_| E::StorageFailure)?;
        snapshots.push(LineageSnapshot {
            root: get(connection, &id)?.ok_or(E::StorageFailure)?,
            permit_job_id,
            generation,
        });
    }
    Ok(snapshots)
}

/// Find active lineage members matching verified native ancestors. The caller
/// must verify every parent link; matching a stored identity alone is not proof.
/// No command data or lineage secret is returned to the caller.
pub fn lineage_anchors(
    connection: &Connection,
    ancestors: &[ProcessIdentity],
) -> JobResult<Vec<(LineageSnapshot, ProcessIdentity)>> {
    if ancestors.len() > 128 || ancestors.iter().any(|identity| !identity.is_valid()) {
        return Err(E::InvalidRequest);
    }
    let mut statement = connection.prepare(
        "SELECT candidate.job_id, root.job_id, root.generation \
         FROM traffic_entries root \
         JOIN permits p ON p.job_id=root.job_id AND p.state='ACTIVE' AND p.generation=root.generation \
         JOIN traffic_entries candidate ON (candidate.job_id=root.job_id OR candidate.root_job_id=root.job_id) \
         JOIN job_lifecycle l ON l.job_id=candidate.job_id \
         WHERE root.root_job_id IS NULL AND candidate.state IN ('RESERVED','RUNNING','RETAINED') \
           AND candidate.generation=root.generation \
           AND ((l.observer_pid=?1 AND l.observer_start_token=?2 AND l.observer_boot_id=?3) \
             OR (l.child_pid=?1 AND l.child_start_token=?2 AND l.child_boot_id=?3 AND l.child_verified_at_ms IS NOT NULL)) \
         ORDER BY candidate.sequence",
    )?;
    let mut snapshots = Vec::new();
    for anchor in ancestors {
        let mut rows = statement.query(params![anchor.pid, anchor.start_token, anchor.boot_id])?;
        while let Some(row) = rows.next()? {
            let id = JobId::try_from(row.get::<_, String>(0)?).map_err(|_| E::StorageFailure)?;
            let root_id =
                JobId::try_from(row.get::<_, String>(1)?).map_err(|_| E::StorageFailure)?;
            let generation = u64::try_from(row.get::<_, i64>(2)?).map_err(|_| E::StorageFailure)?;
            snapshots.push((
                LineageSnapshot {
                    root: get(connection, &id)?.ok_or(E::StorageFailure)?,
                    permit_job_id: root_id,
                    generation,
                },
                anchor.clone(),
            ));
        }
    }
    Ok(snapshots)
}

pub(in crate::jobs::traffic) fn validate_lineage(
    connection: &Connection,
    proof: &LineageProof,
    observer: &ProcessIdentity,
) -> JobResult<()> {
    let snapshot = lineage_anchors(connection, std::slice::from_ref(&proof.anchor))?
        .into_iter()
        .map(|(snapshot, _)| snapshot)
        .find(|snapshot| snapshot.root.job_id == proof.anchor_job_id)
        .ok_or(E::IdentityUnavailable)?;
    if snapshot.permit_job_id != proof.root_id
        || snapshot.root.revision != proof.revision
        || snapshot.generation != proof.generation
        || (snapshot.root.observer != proof.anchor
            && snapshot.root.child.as_ref() != Some(&proof.anchor))
        || proof
            .observed_owner
            .as_ref()
            .is_some_and(|owner| owner != observer)
        || !(proof.recheck)(observer, &proof.anchor)
    {
        return Err(E::IdentityUnavailable);
    }
    Ok(())
}
