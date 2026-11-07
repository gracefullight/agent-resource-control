use crate::migration::{
    checksum, parse_id, Migration, MigrationError, SchemaCompatibility, SchemaStatus,
};
use rusqlite::{types::ValueRef, Connection};

const ARC_OBJECTS: &[&str] = &[
    "vendors",
    "agent_instances",
    "projects",
    "agent_sessions",
    "jobs",
    "permits",
    "resource_objects",
    "resource_leases",
    "gc_operations",
    "resource_samples",
    "policy_states",
    "policy_decisions",
    "audit_events",
    "idx_jobs_state_class",
    "idx_jobs_project_time",
    "idx_resource_identity",
    "idx_lease_resource_state",
    "idx_gc_state",
];

pub(crate) fn assess(
    connection: &Connection,
    registry: &[Migration],
) -> Result<SchemaCompatibility, MigrationError> {
    let supported = registry.last().unwrap().version;
    let witness: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let mut objects = connection.prepare("SELECT name, type FROM sqlite_schema")?;
    let names = objects
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let history_exists = names
        .iter()
        .any(|(name, kind)| name.eq_ignore_ascii_case("schema_migrations") && kind == "table");
    let history_named = names
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("schema_migrations"));
    let arc_exists = names.iter().any(|(name, _)| {
        ARC_OBJECTS
            .iter()
            .any(|known| name.eq_ignore_ascii_case(known))
    });
    let mut result = SchemaCompatibility {
        status: SchemaStatus::InvalidHistory,
        applied_version: None,
        supported_version: supported,
        history_verified: false,
        schema_migrations: 0,
    };
    if !history_exists {
        result.status = if history_named {
            SchemaStatus::InvalidHistory
        } else if witness != 0 || arc_exists {
            SchemaStatus::MissingHistory
        } else {
            SchemaStatus::UpgradeRequired
        };
        if result.status == SchemaStatus::UpgradeRequired {
            result.applied_version = Some(0);
            result.history_verified = true;
        }
        return Ok(result);
    }
    result.schema_migrations =
        connection.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })?;
    let mut statement =
        connection.prepare("SELECT id, applied_at, checksum FROM schema_migrations")?;
    let mut rows = statement.query([])?;
    let mut history = Vec::new();
    while let Some(row) = rows.next()? {
        let (ValueRef::Text(id), ValueRef::Integer(_), ValueRef::Text(hash)) =
            (row.get_ref(0)?, row.get_ref(1)?, row.get_ref(2)?)
        else {
            return Ok(result);
        };
        let (Ok(id), Ok(hash)) = (std::str::from_utf8(id), std::str::from_utf8(hash)) else {
            return Ok(result);
        };
        let Some(version) = parse_id(id) else {
            return Ok(result);
        };
        history.push((version, id.to_owned(), hash.to_owned()));
    }
    history.sort_by_key(|entry| entry.0);
    if history.is_empty() {
        result.status = SchemaStatus::MissingHistory;
        return Ok(result);
    }
    for (index, (version, id, hash)) in history.iter().enumerate() {
        if index > 0 && history[index - 1].0 == *version {
            return Ok(result);
        }
        if *version as usize != index + 1 {
            result.status = SchemaStatus::MissingHistory;
            return Ok(result);
        }
        if let Some(migration) = registry.get(index) {
            if id != migration.id {
                return Ok(result);
            }
            if *hash != checksum(migration.sql) {
                result.status = SchemaStatus::ChecksumMismatch;
                return Ok(result);
            }
        }
    }
    let last = history.last().unwrap().0;
    let advertised = i64::from(last).max(witness);
    if advertised > i64::from(supported) {
        result.status = SchemaStatus::UnsupportedNewer;
        result.applied_version = u32::try_from(advertised).ok();
    } else if witness > i64::from(last) {
        result.status = SchemaStatus::MissingHistory;
    } else if witness == i64::from(last)
        || (witness == 0
            && last == 1
            && history[0].1 == crate::MIGRATION_ID
            && history[0].2 == checksum(crate::MIGRATIONS[0].sql))
    {
        result.status = if last == supported {
            SchemaStatus::Compatible
        } else {
            SchemaStatus::UpgradeRequired
        };
        result.applied_version = Some(last);
        result.history_verified = true;
    }
    Ok(result)
}
