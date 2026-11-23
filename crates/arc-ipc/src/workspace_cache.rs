//! Bounded, transient project-analysis entries for clients without a safe
//! user-owned cache directory. The operation itself negotiates support.
use arc_detector::scripts::cache::{Entry, MAX_ENTRY_BYTES};
use serde::{Deserialize, Serialize};

pub use arc_detector::scripts::cache as cache_types;
pub const OPERATION: &str = "detector.workspace-cache.v1";

#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum CacheRequest {
    Probe,
    Read { key: String },
    Write { key: String, entry: String },
}

impl CacheRequest {
    pub fn valid(&self) -> bool {
        let key_ok = |key: &str| {
            key.len() == 64
                && key
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        match self {
            Self::Probe => true,
            Self::Read { key } => key_ok(key),
            Self::Write { key, entry } => {
                key_ok(key)
                    && entry.len() <= MAX_ENTRY_BYTES
                    && serde_json::from_str::<Entry>(entry)
                        .is_ok_and(|entry| entry.is_well_formed())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cache_types::{CachedFile, FileIdentity, ENTRY_VERSION};

    #[test]
    fn request_round_trips_entry_timestamps_through_wire_value() {
        let request = CacheRequest::Write {
            key: "a".repeat(64),
            entry: serde_json::to_string(&Entry {
                version: ENTRY_VERSION,
                files: vec![CachedFile {
                    path: "package.json".into(),
                    identity: FileIdentity {
                        dev: 1,
                        ino: 2,
                        size: 3,
                        mtime_ns: 1_700_000_000_000_000_000,
                        ctime_ns: 1_700_000_000_000_000_000,
                    },
                    sha256: "b".repeat(64),
                }],
                memberships: vec![],
                absent: vec![],
                results: vec![],
            })
            .unwrap(),
        };
        let value = serde_json::to_value(request).unwrap();
        let parsed: CacheRequest = serde_json::from_value(value).unwrap();
        assert!(parsed.valid());
    }
}
