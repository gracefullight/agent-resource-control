use super::*;

pub(super) fn run() {
    let files = Files::new();
    let id = scanner();
    files.chain(
        &id,
        1,
        &[(T0, T0 + 1), (T0 + 999, T0 + 1_000)],
        (1.0, 0.0, 2.0),
    );
    let mut raw = Connection::open(files.store.path()).unwrap();
    let tx = raw.transaction().unwrap();
    tx.execute_batch(
        "ALTER TABLE attribution_interval_cpu RENAME TO cpu_new;
         CREATE TABLE attribution_interval_cpu (
           interval_id INTEGER NOT NULL REFERENCES attribution_intervals(id) ON DELETE CASCADE,
           confidence TEXT NOT NULL CHECK(confidence IN ('proven','estimated','unattributed')),
           vendor TEXT NOT NULL CHECK(
             (confidence='unattributed' AND vendor='') OR
             (confidence='proven' AND vendor IN ('claude-code','cursor','none')) OR
             (confidence='estimated' AND vendor IN ('claude-code','cursor'))),
           cpu_core_seconds REAL NOT NULL CHECK(cpu_core_seconds >= 0),
           PRIMARY KEY(interval_id,confidence,vendor)) WITHOUT ROWID;
         INSERT INTO attribution_interval_cpu SELECT * FROM cpu_new;
         DROP TABLE cpu_new;
         PRAGMA user_version=4;",
    )
    .unwrap();
    tx.commit().unwrap();
    drop(raw);
    let prior = files.read(T0, T0 + 1_000, Some(&live(&id, None)));
    assert_eq!(prior.proven[0].cpu_core_seconds.value, Some(1.0));
    let mut next = interval(T0 + 999, T0 + 2_000, 0.0, 0.0, 0.0);
    next.proven.push(("codex".into(), 3.0));
    files.store.close(&id, 2, 3, &next, T0 + 1_999, T0).unwrap();
    assert_eq!(count(&files.store, "PRAGMA user_version"), 5);
    let after = files.read(T0, T0 + 2_000, Some(&live(&id, None)));
    assert_eq!(
        after
            .proven
            .iter()
            .find(|g| g.vendor == "claude-code")
            .unwrap()
            .cpu_core_seconds
            .value,
        Some(1.0)
    );
    assert_eq!(
        after
            .proven
            .iter()
            .find(|g| g.vendor == "codex")
            .unwrap()
            .cpu_core_seconds
            .value,
        Some(3.0)
    );
    assert_eq!(after.unattributed.value, Some(2.0));
}
