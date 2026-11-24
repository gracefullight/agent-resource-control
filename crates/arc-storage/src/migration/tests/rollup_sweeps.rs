use super::*;

#[test]
fn rollup_sweeps_upgrade_handles_absent_and_existing_objects_without_losing_pending_data() {
    const NOW: u64 = 1_800_000_000_000;
    let base = (NOW - 80 * 60 * 60 * 1_000) / 60_000 * 60_000;
    let ts = format!("{:019}", base * 1_000_000);

    for existing in [false, true] {
        let database = Database::new();
        let mut connection = database.connect();
        migrate(&mut connection, &MIGRATIONS[..11]).unwrap();
        let old_history = history(&connection);
        connection
            .execute_batch(
                "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
             VALUES('fixture','BUILD','LOCAL','OBSERVE','COMPLETED',0);",
            )
            .unwrap();
        connection.execute(
            "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,rss_bytes,attribution_confidence)
             VALUES(?1,'fixture','process',300,'A')", [&ts],
        ).unwrap();

        let old_roots = if existing {
            // This is the exact persistent schema created lazily by the prior
            // rollup path. Its pending sum represents an interrupted sweep.
            connection.execute_batch(
                "CREATE TABLE resource_rollup_pending_sweeps(ts TEXT PRIMARY KEY NOT NULL, rss INTEGER NOT NULL);
                 CREATE INDEX idx_resource_samples_rollup_next ON resource_samples(job_id,process_identity,timestamp_ns);",
            ).unwrap();
            connection
                .execute(
                    "INSERT INTO resource_rollup_pending_sweeps(ts,rss) VALUES(?1,200)",
                    [&ts],
                )
                .unwrap();
            Some(roots(&connection))
        } else {
            None
        };

        migrate(&mut connection, MIGRATIONS).unwrap();
        assert_eq!(&history(&connection)[..11], old_history.as_slice());
        assert_eq!(history(&connection).len(), 12);
        assert_eq!(
            assess_schema(&connection).unwrap().applied_version,
            Some(12)
        );
        let new_roots = roots(&connection);
        assert_eq!(new_roots.len(), 2);
        if let Some(old_roots) = old_roots {
            assert_eq!(new_roots, old_roots, "existing objects were replaced");
            assert_eq!(
                connection
                    .query_row(
                        "SELECT rss FROM resource_rollup_pending_sweeps WHERE ts=?1",
                        [&ts],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap(),
                200
            );
        }
        migrate(&mut connection, MIGRATIONS).unwrap();
        assert_eq!(roots(&connection), new_roots);
        let report = crate::rollup::roll_up(&mut connection, NOW).unwrap();
        assert_eq!(report.raw_rolled_up, 1);
        let expected = if existing { 500 } else { 300 };
        let peaks: Vec<i64> = connection
            .prepare("SELECT peak_rss_bytes FROM resource_rollups ORDER BY granularity")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(peaks, vec![expected; 3]);
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM resource_rollup_pending_sweeps",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(
            crate::rollup::roll_up(&mut connection, NOW)
                .unwrap()
                .raw_rolled_up,
            0
        );
        let peaks_after: Vec<i64> = connection
            .prepare("SELECT peak_rss_bytes FROM resource_rollups ORDER BY granularity")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            peaks_after, peaks,
            "repeated rollup changed a completed sweep"
        );
    }
}

fn roots(connection: &Connection) -> Vec<(String, i64)> {
    connection
        .prepare(
            "SELECT name,rootpage FROM sqlite_schema WHERE name IN
         ('resource_rollup_pending_sweeps','idx_resource_samples_rollup_next') ORDER BY name",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
