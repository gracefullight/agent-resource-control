import { tokens } from "../../coverage/source";

// Operator-authorized schema-2 expectations. Apply only to immutable baseline
// content, never to candidate source. Each replacement names its path, function
// and exact old text; all remaining tokens, assertions and reachability are pinned.
type Edit = [item: string, before: string, after: string, occurrences?: number];
const countQuery = '"SELECT COUNT(*) FROM schema_migrations", [], |row| row\n                .get::<_, i64>(0))\n            .unwrap(),\n        1';
const doctorVersion = "Schema compatible. Version 1. Migration history verified.";
const qualifiedId = "UPDATE schema_migrations SET id='0001_other' WHERE id='0001_phase0'";
const firstRow = "SELECT id, applied_at, checksum FROM schema_migrations WHERE id='0001_phase0'";
const secondRow = `    let second: (String, i64, String) = connection
        .query_row(
            "SELECT id, applied_at, checksum FROM schema_migrations WHERE id='0002_managed_exec'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(second.0, "0002_managed_exec");
    assert!(second.1 > 0);
    assert_eq!(
        second.2,
        Sha256::digest(include_bytes!("../migrations/0002_managed_exec.sql"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
`;
const secondReopen = `    let recorded_second: (String, i64, String) = reopened
        .query_row(
            "SELECT id, applied_at, checksum FROM schema_migrations WHERE id='0002_managed_exec'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(recorded_second, second);
`;
const fixedLegacy = `
#[test]
fn fixed_schema1_registry_stamps_legacy_witness_without_upgrading() {
    let database = Database::new();
    let mut connection = database.connect();
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    connection
        .execute(
            "INSERT INTO vendors VALUES ('kept', 'original', NULL, 17)",
            [],
        )
        .unwrap();
    connection.pragma_update(None, "user_version", 0).unwrap();
    let legacy = snapshot(&connection);
    let report = assess_with_registry(&connection, &MIGRATIONS[..1]).unwrap();
    assert_eq!(report.status, SchemaStatus::Compatible);
    assert_eq!(report.applied_version, Some(1));
    assert_eq!(report.supported_version, 1);
    assert_eq!(report.schema_migrations, 1);
    assert!(report.history_verified);
    assert_eq!(snapshot(&connection), legacy);
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    let stamped = snapshot(&connection);
    assert_eq!(stamped.tables, legacy.tables);
    assert_eq!(stamped.schema, legacy.schema);
    assert_eq!(stamped.witness, 1);
    connection.close().unwrap();
    let mut connection = database.connect();
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    assert_eq!(snapshot(&connection), stamped);
}
`;
const nfr = "nfr010_wal_migration_survives_close_and_reopen_without_cleanup";
const schema = "live_schema_failures_use_bounded_worker_reach_cli_and_preserve_records_and_history";
const startup = "missing_invalid_and_newer_schema_startup_reject_without_data_or_history_changes";
const legacyUpgrade = "production_legacy_history_is_read_only_then_stamped_atomically";
// Schema 3 rebuilds permits and adds Traffic tables. Preserve the old rows and
// foreign keys with the exact replacement assertions, not a blanket exemption.
const trafficPermitAssertions = `for object in &legacy.schema {
        if object.1 != "permits" {
            assert!(stamped.schema.contains(object));
        }
    }
    let upgraded_permit_columns = permit_columns(&connection);
    assert_eq!(
        permit_foreign_keys(&connection),
        original_permit_foreign_keys
    );
    assert_eq!(
        upgraded_permit_columns.len(),
        original_permit_columns.len() + 2
    );
    assert_eq!(
        upgraded_permit_columns[..original_permit_columns.len()],
        original_permit_columns
    );
    assert_eq!(
        &upgraded_permit_columns[original_permit_columns.len()..],
        &[
            (
                "cpu_millis_est".to_owned(),
                "INTEGER".to_owned(),
                0,
                None,
                0
            ),
            ("generation".to_owned(), "INTEGER".to_owned(), 0, None, 0),
        ]
    );
    assert_eq!(
        permit_unique_columns(&connection),
        original_permit_unique_columns
    );
    connection
        .execute_batch(
            "SAVEPOINT permit_uniqueness;
         INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
           VALUES ('fixture-job-a','BUILD','HOST_LOCAL','NONE','NEW',1),
                  ('fixture-job-b','BUILD','HOST_LOCAL','NONE','NEW',1);
         INSERT INTO permits(id,job_id,state,reserved_at)
           VALUES ('fixture-permit','fixture-job-a','ACTIVE',1);",
        )
        .unwrap();
    assert!(connection.execute(
        "INSERT INTO permits(id,job_id,state,reserved_at) VALUES ('fixture-permit','fixture-job-b','ACTIVE',1)", [],
    ).is_err());
    assert!(connection.execute(
        "INSERT INTO permits(id,job_id,state,reserved_at) VALUES ('other-permit','fixture-job-a','ACTIVE',1)", [],
    ).is_err());
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM permits", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    connection
        .execute_batch("ROLLBACK TO permit_uniqueness; RELEASE permit_uniqueness;")
        .unwrap();`;
const trafficPermitHelpers = `
fn permit_unique_columns(connection: &Connection) -> Vec<Vec<String>> {
    let names: Vec<String> = connection
        .prepare("SELECT name FROM pragma_index_list('permits') WHERE \\"unique\\" = 1")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut columns: Vec<Vec<String>> = names
        .iter()
        .map(|name| {
            connection
                .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
                .unwrap()
                .query_map([name], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        })
        .collect();
    columns.sort();
    columns
}

fn permit_columns(connection: &Connection) -> Vec<(String, String, i64, Option<String>, i64)> {
    connection
        .prepare("PRAGMA table_info(permits)")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

type PermitForeignKey = (String, String, String, String, String, String);
fn permit_foreign_keys(connection: &Connection) -> Vec<PermitForeignKey> {
    connection
        .prepare("PRAGMA foreign_key_list(permits)")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

`;
const recipes: Record<string, Edit[]> = {
  "crates/arc-storage/src/migration/tests/acceptance.rs": [
    [legacyUpgrade, "let mut connection = crate::open_and_migrate(&database.0).unwrap();", `let mut connection = database.connect();
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    connection
        .execute(
            "INSERT INTO vendors VALUES ('kept', 'original', NULL, 17)",
            [],
        )
        .unwrap();
    let original_history = history(&connection);`],
    [legacyUpgrade, "assert_eq!(report.status, SchemaStatus::Compatible);", `assert_eq!(report.status, SchemaStatus::UpgradeRequired);
    assert_eq!(report.applied_version, Some(1));
    assert_eq!(report.supported_version, 2);
    assert_eq!(report.schema_migrations, 1);`],
    [legacyUpgrade, `assert_eq!(stamped.tables, legacy.tables);
    assert_eq!(stamped.schema, legacy.schema);`, `assert_eq!(stamped.tables.len(), legacy.tables.len() + 1);
    for (name, rows) in &legacy.tables {
        let updated = &stamped.tables.iter().find(|(n, _)| n == name).unwrap().1;
        if name == "schema_migrations" {
            assert_eq!(updated.len(), 2);
            assert_eq!(updated.first(), rows.first());
        } else {
            assert_eq!(updated, rows);
        }
    }
    assert_eq!(stamped.schema.len(), legacy.schema.len() + 3);
    for object in &legacy.schema {
        assert!(stamped.schema.contains(object));
    }
    assert_eq!(history(&connection)[0], original_history[0]);`],
    [legacyUpgrade, "assert_eq!(stamped.witness, 1);", "assert_eq!(stamped.witness, 2);"],
  ],
  "crates/arc-storage/tests/nfr010.rs": [
    [nfr, '        "jobs",', '        "jobs",\n        "job_lifecycle",'],
    [nfr, "assert_eq!(count, 1);", "assert_eq!(count, 2);"],
    [nfr, '"SELECT id, applied_at, checksum FROM schema_migrations"', JSON.stringify(firstRow), 2],
    [nfr, countQuery, countQuery.slice(0, -1) + "2"],
    [nfr, '    assert_eq!(checksum.len(), 64);', secondRow + '    assert_eq!(checksum.len(), 64);'],
    [nfr, '    assert_eq!(recorded, (id, applied_at, checksum));', '    assert_eq!(recorded, (id, applied_at, checksum));\n' + secondReopen],
  ],
  "crates/arc-daemon/src/db_worker/tests.rs": [
    ["reopen", '"SELECT count(*) FROM schema_migrations", [], |row| row\n                    .get::<_, i64>(0))\n                .unwrap(),\n            1', '"SELECT count(*) FROM schema_migrations", [], |row| row\n                    .get::<_, i64>(0))\n                .unwrap(),\n            2'],
    ["queued_deadline_is_absolute_and_expired_job_is_not_started", "assert_eq!(recover(&client).await.schema_migrations, 1);", "assert_eq!(recover(&client).await.schema_migrations, 2);"],
  ],
  "crates/arc-daemon/src/server_tests.rs": [
    ["a_socketpair_serves_hello_and_survives_an_incompatible_version", 'assert_eq!(result["schema_migrations"], 1);', 'assert_eq!(result["schema_migrations"], 2);'],
  ],
  "crates/arc-daemon/src/database_failure_tests.rs": [
    ["typed_worker_errors_reach_protocol_and_actual_cli_then_recover", doctorVersion, doctorVersion.replace("Version 1", "Version 2")],
  ],
  "crates/arc-daemon/tests/support/runtime.rs": [
    ["assert_cli", doctorVersion, doctorVersion.replace("Version 1", "Version 2")],
  ],
  "crates/arc-daemon/tests/resident/lifecycle.rs": [
    ["resident_contract_holds_until_sigterm", doctorVersion, doctorVersion.replace("Version 1", "Version 2")],
    ["resident_contract_holds_until_sigterm", countQuery, countQuery.slice(0, -1) + "2"],
  ],
  "crates/arc-daemon/tests/resident/schema.rs": [
    [schema, 'SchemaStatus::ChecksumMismatch, "migration checksum mismatch", 1)', 'SchemaStatus::ChecksumMismatch, "migration checksum mismatch", 2)'],
    [schema, "UPDATE schema_migrations SET id='0001_other'", qualifiedId],
    [schema, 'SchemaStatus::InvalidHistory, "invalid migration history", 1)', 'SchemaStatus::InvalidHistory, "invalid migration history", 2)'],
    [schema, "('0002_future',42,'unverified-future'); PRAGMA user_version=2", "('0003_future',42,'unverified-future'); PRAGMA user_version=3"],
    [schema, 'SchemaStatus::UnsupportedNewer, "unsupported newer schema", 2)', 'SchemaStatus::UnsupportedNewer, "unsupported newer schema", 3)'],
    [schema, "assert_eq!(schema.supported_version, 1);", "assert_eq!(schema.supported_version, 2);"],
    [schema, "SchemaStatus::UnsupportedNewer { Some(2) }", "SchemaStatus::UnsupportedNewer { Some(3) }"],
    ["baseline_client_adapter_accepts_actual_new_daemon_production_report", 'assert_eq!(result["schema_migrations"].as_i64(), Some(1));', 'assert_eq!(result["schema_migrations"].as_i64(), Some(2));'],
  ],
  "crates/arc-daemon/tests/startup.rs": [
    [startup, "UPDATE schema_migrations SET id='0001_other'", qualifiedId],
    [startup, "('0002_future',42,'future-checksum'); PRAGMA user_version=2", "('0003_future',42,'future-checksum'); PRAGMA user_version=3"],
    [startup, "unsupported newer schema version 2 (supported 1)", "unsupported newer schema version 3 (supported 2)"],
  ],
};
const trafficRecipes: Record<string, Edit[]> = {
  "crates/arc-daemon/src/database_failure_tests.rs": [
    ["typed_worker_errors_reach_protocol_and_actual_cli_then_recover", doctorVersion.replace("Version 1", "Version 2"), doctorVersion.replace("Version 1", "Version 3")],
  ],
  "crates/arc-daemon/tests/support/runtime.rs": [
    ["assert_cli", doctorVersion.replace("Version 1", "Version 2"), doctorVersion.replace("Version 1", "Version 3")],
  ],
  "crates/arc-daemon/src/db_worker/tests.rs": [
    ["reopen", '"SELECT count(*) FROM schema_migrations", [], |row| row\n                    .get::<_, i64>(0))\n                .unwrap(),\n            2', '"SELECT count(*) FROM schema_migrations", [], |row| row\n                    .get::<_, i64>(0))\n                .unwrap(),\n            3'],
    ["queued_deadline_is_absolute_and_expired_job_is_not_started", "assert_eq!(recover(&client).await.schema_migrations, 2);", "assert_eq!(recover(&client).await.schema_migrations, 3);"],
  ],
  "crates/arc-daemon/src/server_tests.rs": [
    ["a_socketpair_serves_hello_and_survives_an_incompatible_version", 'assert_eq!(result["schema_migrations"], 2);', 'assert_eq!(result["schema_migrations"], 3);'],
  ],
  "crates/arc-daemon/tests/resident/lifecycle.rs": [
    ["resident_contract_holds_until_sigterm", doctorVersion.replace("Version 1", "Version 2"), doctorVersion.replace("Version 1", "Version 3")],
    ["resident_contract_holds_until_sigterm", countQuery.slice(0, -1) + "2", countQuery.slice(0, -1) + "3"],
  ],
  "crates/arc-daemon/tests/resident/schema.rs": [
    [schema, 'SchemaStatus::ChecksumMismatch, "migration checksum mismatch", 2)', 'SchemaStatus::ChecksumMismatch, "migration checksum mismatch", 3)'],
    [schema, 'SchemaStatus::InvalidHistory, "invalid migration history", 2)', 'SchemaStatus::InvalidHistory, "invalid migration history", 3)'],
    [schema, "('0003_future',42,'unverified-future'); PRAGMA user_version=3", "('0004_future',42,'unverified-future'); PRAGMA user_version=4"],
    [schema, 'SchemaStatus::UnsupportedNewer, "unsupported newer schema", 3)', 'SchemaStatus::UnsupportedNewer, "unsupported newer schema", 4)'],
    [schema, "assert_eq!(schema.supported_version, 2);", "assert_eq!(schema.supported_version, 3);"],
    [schema, "SchemaStatus::UnsupportedNewer { Some(3) }", "SchemaStatus::UnsupportedNewer { Some(4) }"],
    ["baseline_client_adapter_accepts_actual_new_daemon_production_report", 'assert_eq!(result["schema_migrations"].as_i64(), Some(2));', 'assert_eq!(result["schema_migrations"].as_i64(), Some(3));'],
  ],
  "crates/arc-daemon/tests/startup.rs": [
    [startup, "('0003_future',42,'future-checksum'); PRAGMA user_version=3", "('0004_future',42,'future-checksum'); PRAGMA user_version=4"],
    [startup, "unsupported newer schema version 3 (supported 2)", "unsupported newer schema version 4 (supported 3)"],
  ],
  "crates/arc-storage/src/migration/tests/acceptance.rs": [
    [legacyUpgrade, "let original_history = history(&connection);", `let original_history = history(&connection);
    let original_permit_columns = permit_columns(&connection);
    let original_permit_foreign_keys = permit_foreign_keys(&connection);
    let original_permit_unique_columns = permit_unique_columns(&connection);
    assert_eq!(
        original_permit_unique_columns,
        vec![vec!["id"], vec!["job_id"]]
    );`],
    [legacyUpgrade, "assert_eq!(report.supported_version, 2);", "assert_eq!(report.supported_version, 3);"],
    [legacyUpgrade, "assert_eq!(stamped.tables.len(), legacy.tables.len() + 1);", "assert_eq!(stamped.tables.len(), legacy.tables.len() + 4);"],
    [legacyUpgrade, "assert_eq!(updated.len(), 2);", "assert_eq!(updated.len(), 3);"],
    [legacyUpgrade, "assert_eq!(stamped.schema.len(), legacy.schema.len() + 3);", "assert_eq!(stamped.schema.len(), legacy.schema.len() + 11);"],
    [legacyUpgrade, "for object in &legacy.schema {\n        assert!(stamped.schema.contains(object));\n    }", trafficPermitAssertions],
    [legacyUpgrade, "assert_eq!(stamped.witness, 2);", "assert_eq!(stamped.witness, 3);"],
  ],
  "crates/arc-storage/tests/nfr010.rs": [
    [nfr, '        "job_lifecycle",', '        "job_lifecycle",\n        "traffic_entries",\n        "traffic_tombstones",'],
    [nfr, "assert_eq!(count, 2);", "assert_eq!(count, 3);"],
    [nfr, countQuery.slice(0, -1) + "2", countQuery.slice(0, -1) + "3"],
  ],
  "crates/arc-storage/tests/managed_exec_migration.rs": [
    ["assert_current", "assert_eq!(assessment.applied_version, Some(2));", "assert_eq!(assessment.applied_version, Some(3));"],
    ["assert_current", "assert_eq!(assessment.supported_version, 2);", "assert_eq!(assessment.supported_version, 3);"],
    ["assert_current", "assert_eq!(assessment.schema_migrations, 2);", "assert_eq!(assessment.schema_migrations, 3);"],
    ["assert_current", "assert_eq!(MIGRATIONS.len(), 2);", "assert_eq!(MIGRATIONS.len(), 3);"],
    ["assert_current", 'assert_eq!(MIGRATIONS[1].id, "0002_managed_exec");', 'assert_eq!(MIGRATIONS[1].id, "0002_managed_exec");\n    assert_eq!(MIGRATIONS[2].id, "0003_traffic");'],
    ["assert_current", "assert_eq!(entries.len(), 2);", "assert_eq!(entries.len(), 3);"],
    ["assert_current", "assert_eq!(snapshot(c).witness, 2);", `assert_eq!(
        entries[1].2,
        "464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814"
    );
    assert_eq!(snapshot(c).witness, 3);`],
    ["current_registry_default_open_preserves_wal_inventory_history_and_reopen", '            "schema_migrations",', '            "schema_migrations",\n            "sqlite_sequence",\n            "traffic_entries",\n            "traffic_tombstones",'],
    ["legacy_v1_without_witness_is_read_only_then_upgrades_without_rewriting_history", "assert_eq!(assessment.supported_version, 2);", "assert_eq!(assessment.supported_version, 3);"],
    ["current_schema_rejects_invalid_history_and_witness_without_mutation", '("PRAGMA user_version=3", SchemaStatus::UnsupportedNewer),', '("PRAGMA user_version=2", SchemaStatus::InvalidHistory),\n        ("PRAGMA user_version=4", SchemaStatus::UnsupportedNewer),'],
    ["current_schema_rejects_invalid_history_and_witness_without_mutation", "('0003_future', 123, 'untrusted'); PRAGMA user_version=3", "('0004_future', 123, 'untrusted'); PRAGMA user_version=4"],
    ["current_schema_rejects_invalid_history_and_witness_without_mutation", "assert_eq!(assessment.supported_version, 2);", "assert_eq!(assessment.supported_version, 3);"],
    ["current_schema_rejects_invalid_history_and_witness_without_mutation", "schema version 3 (supported 2)", "schema version 4 (supported 3)"],
  ],
};
export const schemaAdaptationPaths = Object.keys(recipes);

function editItem(source: string, [name, before, after, occurrences = 1]: Edit): string {
  const ts = tokens(source);
  const matches = ts.flatMap((t, i) => t.text === "fn" && ts[i + 1]?.text === name ? [i] : []);
  if (matches.length !== 1) throw Error(`SCHEMA_REFERENCE: unique function required: ${name}`);
  const start = matches[0];
  let open = start;
  while (open < ts.length && ts[open].text !== "{") open++;
  let depth = 1, end = open + 1;
  for (; end < ts.length && depth; end++) {
    if (ts[end].text === "{") depth++;
    if (ts[end].text === "}") depth--;
  }
  if (depth) throw Error(`SCHEMA_REFERENCE: incomplete function: ${name}`);
  const from = ts[start].start, to = ts[end - 1].end, item = source.slice(from, to);
  if (item.split(before).length - 1 !== occurrences)
    throw Error(`SCHEMA_REFERENCE: exact baseline occurrence required: ${name}: ${before}`);
  return source.slice(0, from) + item.split(before).join(after) + source.slice(to);
}
export function trafficSchemaReference(path: string, schema2Baseline: string): string {
  let source = schema2Baseline;
  for (const edit of trafficRecipes[path] ?? []) source = editItem(source, edit);
  if (path === "crates/arc-storage/src/migration/tests/acceptance.rs") {
    const next = "\n#[test]\nfn sql_failure_rolls_back_schema_application_data_history_and_witness()";
    if (source.split(next).length !== 2) throw Error("SCHEMA_REFERENCE: missing migration helper insertion point");
    source = source.replace(next, trafficPermitHelpers + next);
  }
  return source;
}
export function schemaReference(path: string, immutableBaseline: string): string {
  let source = immutableBaseline;
  for (const edit of recipes[path] ?? []) source = editItem(source, edit);
  source = trafficSchemaReference(path, source);
  if (path === "crates/arc-storage/src/migration/tests/acceptance.rs") source += fixedLegacy;
  return source;
}
