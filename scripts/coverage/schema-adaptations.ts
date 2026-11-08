import { tokens } from "./source";

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
export function schemaReference(path: string, immutableBaseline: string): string {
  let source = immutableBaseline;
  for (const edit of recipes[path] ?? []) source = editItem(source, edit);
  if (path === "crates/arc-storage/src/migration/tests/acceptance.rs") source += fixedLegacy;
  return source;
}
