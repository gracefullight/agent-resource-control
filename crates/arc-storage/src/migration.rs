use rusqlite::{params, Connection, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

pub const MIGRATION_ID: &str = "0001_phase0";
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        id: MIGRATION_ID,
        sql: include_str!("../migrations/0001_phase0.sql"),
    },
    Migration {
        version: 2,
        id: "0002_managed_exec",
        sql: include_str!("../migrations/0002_managed_exec.sql"),
    },
    Migration {
        version: 3,
        id: "0003_traffic",
        sql: include_str!("../migrations/0003_traffic.sql"),
    },
    Migration {
        version: 4,
        id: "0004_job_meter",
        sql: include_str!("../migrations/0004_job_meter.sql"),
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub id: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaStatus {
    Compatible,
    UpgradeRequired,
    MissingHistory,
    ChecksumMismatch,
    InvalidHistory,
    UnsupportedNewer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaCompatibility {
    pub status: SchemaStatus,
    pub applied_version: Option<u32>,
    pub supported_version: u32,
    pub history_verified: bool,
    pub schema_migrations: i64,
}

#[derive(Debug)]
pub enum MigrationError {
    InvalidRegistry(&'static str),
    Incompatible(SchemaCompatibility),
    Sql(rusqlite::Error),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegistry(reason) => write!(f, "invalid compiled migration registry: {reason}"),
            Self::Sql(error) => write!(f, "SQLite migration operation failed: {error}"),
            Self::Incompatible(schema) => match schema.status {
                SchemaStatus::UnsupportedNewer => write!(f,
                    "unsupported newer schema version {} (supported {}); use a binary supporting that version; downgrade execution is not supported",
                    schema.applied_version.unwrap_or(0), schema.supported_version),
                SchemaStatus::ChecksumMismatch => f.write_str("migration checksum mismatch; preserve the database and investigate against a trusted copy"),
                SchemaStatus::MissingHistory => f.write_str("missing migration history; preserve the database and investigate against a trusted copy"),
                _ => f.write_str("invalid migration history; preserve the database and investigate against a trusted copy"),
            },
        }
    }
}

impl Error for MigrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sql(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for MigrationError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}

pub(crate) fn checksum(sql: &str) -> String {
    Sha256::digest(sql.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn parse_id(id: &str) -> Option<u32> {
    let (prefix, suffix) = id.split_once('_')?;
    let version: u32 = prefix.parse().ok()?;
    (version > 0
        && version <= i32::MAX as u32
        && prefix == format!("{version:04}")
        && !suffix.is_empty()
        && suffix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'))
    .then_some(version)
}

pub(crate) fn validate_registry(registry: &[Migration]) -> Result<(), MigrationError> {
    if registry.is_empty() {
        return Err(MigrationError::InvalidRegistry("empty registry"));
    }
    for (index, migration) in registry.iter().enumerate() {
        if migration.version as usize != index + 1
            || parse_id(migration.id) != Some(migration.version)
        {
            return Err(MigrationError::InvalidRegistry(
                "versions and IDs must form an ordered sequence from 1",
            ));
        }
        if migration.sql.trim().is_empty() {
            return Err(MigrationError::InvalidRegistry("empty SQL"));
        }
        if registry[..index]
            .iter()
            .any(|prior| prior.id == migration.id)
        {
            return Err(MigrationError::InvalidRegistry("duplicate ID"));
        }
    }
    Ok(())
}

/// Assess one read snapshot without changing schema, history, or version witness.
pub fn assess_schema(connection: &Connection) -> Result<SchemaCompatibility, MigrationError> {
    assess_with_registry(connection, MIGRATIONS)
}

pub(crate) fn assess_with_registry(
    connection: &Connection,
    registry: &[Migration],
) -> Result<SchemaCompatibility, MigrationError> {
    validate_registry(registry)?;
    if connection.is_autocommit() {
        let transaction = connection.unchecked_transaction()?;
        let result = crate::history::assess(&transaction, registry);
        transaction.rollback()?;
        result
    } else {
        crate::history::assess(connection, registry)
    }
}

pub(crate) fn migrate(
    connection: &mut Connection,
    registry: &[Migration],
) -> Result<(), MigrationError> {
    validate_registry(registry)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    #[cfg(test)]
    tests::observe(tests::Stage::Locked, &transaction);
    let result = (|| {
        let assessment = assess_with_registry(&transaction, registry)?;
        if !assessment.history_verified {
            return Err(MigrationError::Incompatible(assessment));
        }
        for migration in &registry[assessment.applied_version.unwrap_or(0) as usize..] {
            transaction.execute_batch(migration.sql)?;
            transaction.execute(
                "INSERT INTO schema_migrations (id, applied_at, checksum) VALUES (?1, unixepoch(), ?2)",
                params![migration.id, checksum(migration.sql)],
            )?;
        }
        transaction.pragma_update(None, "user_version", registry.last().unwrap().version)?;
        #[cfg(test)]
        tests::observe(tests::Stage::BeforeCommit, &transaction);
        Ok(())
    })();
    match result {
        Ok(()) => transaction.commit().map_err(MigrationError::Sql),
        Err(error) => {
            transaction.rollback()?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
