//! Class-aware assertions over the existing Case/Run and owned-process implementation.
use super::*;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
pub struct ReadyWitness {
    pub job_id: String,
    pub child: ProcessIdentity,
    pub facts: Value,
    pub starts: usize,
}
#[derive(Debug, Serialize)]
pub struct QueueWitness {
    pub job_id: String,
    pub facts: Value,
    pub starts: usize,
}
#[derive(Debug, Serialize)]
pub struct ReleaseWitness {
    pub job_id: String,
    pub facts: Value,
    pub starts: usize,
}

pub fn marker_ready(run: &mut Run) -> bool {
    log(&run.project.join("bodies")).lines().count() == 1
}
pub fn entered<T>(case: &Case<T>, run: &mut Run, prepared: &Prepared) -> ReadyWitness {
    eventually(|| {
        assert!(
            run.wrapper.child.try_wait().unwrap().is_none(),
            "native tool exited before explicit readiness: {} {}",
            log(&run.project.join("stdout")),
            log(&run.project.join("stderr"))
        );
        (prepared.ready)(run)
    });
    let id = case.job(run.wrapper.child.id());
    eventually(|| case.row(&id)["state"] == "RUNNING");
    let row = case.row(&id);
    let child = ProcessIdentity::capture(row["child_pid"].as_u64().unwrap() as u32).unwrap();
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
    run.wrapper.own_process(child.clone());
    assert_eq!(
        executable(child.pid),
        prepared.executable.canonicalize().unwrap()
    );
    assert_eq!(log(&run.project.join("launches")).lines().count(), 1);
    ReadyWitness {
        job_id: id.clone(),
        child,
        facts: case.reconcile(&id, prepared.form.class(), prepared.form.reason()),
        starts: 1,
    }
}
pub fn queued<T>(case: &Case<T>, run: &mut Run, form: Form) -> QueueWitness {
    let id = case.job(run.wrapper.child.id());
    eventually(|| {
        case.row(&id)["state"] == "QUEUED"
            && log(&run.project.join("stderr")).contains("QUEUED\nWaiting to start.")
    });
    assert_eq!(log(&run.project.join("launches")).lines().count(), 0);
    assert_eq!(log(&run.project.join("bodies")).lines().count(), 0);
    assert!(crate::cleanup::children(run.wrapper.child.id())
        .unwrap()
        .is_empty());
    let facts = case.reconcile(&id, form.class(), form.reason());
    assert!(facts["sqlite"]["child_pid"].is_null());
    assert_eq!(facts["diagnostic"]["admission"]["permit_active"], false);
    assert_eq!(
        facts["diagnostic"]["admission"]["wait_reason"],
        "class_window"
    );
    QueueWitness {
        job_id: id,
        facts,
        starts: 0,
    }
}
pub fn terminal<T>(case: &Case<T>, run: &Run, form: Form, code: i32) -> ReleaseWitness {
    let facts = case.terminal(run, form.class(), form.reason(), code);
    assert_eq!(log(&run.project.join("launches")).lines().count(), 1);
    ReleaseWitness {
        job_id: case.job(run.wrapper.child.id()),
        facts,
        starts: 1,
    }
}

/// Reconstruct persisted intervals by the requested class; release sorts before a tied grant.
pub fn permit_history<T>(case: &Case<T>, class: &str, expected: usize, maximum: usize) -> Value {
    let rows = case.rows("SELECT p.job_id,j.job_class,p.state,p.generation,p.released_at,t.granted_at_ms,p.cpu_millis_est,p.memory_bytes_est FROM permits p JOIN jobs j ON j.id=p.job_id JOIN traffic_entries t ON t.job_id=p.job_id ORDER BY t.granted_at_ms,p.job_id");
    assert_eq!(rows.len(), expected);
    let high = interval_high_water(&rows, class, maximum);
    assert_eq!(case.active()["permits"], 0);
    json!({"class":class,"allIntervals":rows,"matchingClassPermitHighWater":high,"released":expected})
}

fn interval_high_water(rows: &[Value], class: &str, maximum: usize) -> i32 {
    let mut events = Vec::new();
    for row in rows {
        assert_eq!(row["state"], "RELEASED");
        let grant = row["granted_at_ms"].as_u64().unwrap();
        let release = row["released_at"].as_u64().unwrap();
        assert!(release >= grant);
        if row["job_class"] == class && grant < release {
            events.extend([(grant, 1_i32), (release, -1_i32)]);
        }
    }
    events.sort_unstable();
    let (mut active, mut high) = (0_i32, 0_i32);
    for (_, delta) in events {
        active += delta;
        high = high.max(active);
        assert!((0..=maximum as i32).contains(&active));
    }
    assert_eq!(active, 0);
    high
}

#[test]
fn class_aware_interval_contract_ignores_other_classes_and_orders_tied_release_first() {
    for class in ["BUILD", "TEST", "LINT"] {
        let rows = [
            json!({"job_class":"UNRELATED","state":"RELEASED","granted_at_ms":0,"released_at":100}),
            json!({"job_class":class,"state":"RELEASED","granted_at_ms":10,"released_at":20}),
            json!({"job_class":class,"state":"RELEASED","granted_at_ms":20,"released_at":30}),
        ];
        assert_eq!(interval_high_water(&rows, class, 1), 1);
    }
}
