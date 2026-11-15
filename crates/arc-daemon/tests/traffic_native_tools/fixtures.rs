//! Small shared preparation seams. Process ownership stays in Case/Run/OwnedChild.
use super::{sha, write};
use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

/// One observer entry, then an unmodified exec into the installed native tool.
/// This belongs only to fixture PATH, never the production caller environment.
pub fn observer(directory: &Path, name: &str, real: &Path) -> Value {
    assert!(matches!(name, "go" | "swift" | "cmake" | "ctest" | "cargo"));
    fs::create_dir_all(directory).unwrap();
    let path = directory.join(name);
    let quoted = format!("'{}'", real.to_str().unwrap().replace('\'', "'\\''"));
    write(
        &path,
        format!(
            r##"#!/bin/sh
printf '%s %s %s\n' {name} "$$" "$PPID" >> "$TBS_PROJECT/launches" || exit 125
if [ -n "${{TBS_PREDECESSOR:-}}" ]; then
  case "$TBS_PREDECESSOR" in *[!0123456789abcdef]*|'') exit 125;; esac
  [ "${{#TBS_PREDECESSOR}}" -eq 32 ] || exit 125
  state=$(/usr/bin/sqlite3 -readonly -cmd '.timeout 2000' -cmd ".parameter set :id \"'$TBS_PREDECESSOR'\"" "$ARC_DB_PATH" 'SELECT state FROM permits WHERE job_id=:id;') || exit 125
  [ "$state" = RELEASED ] || exit 125
  printf '%s\n' "$state" > "$TBS_PROJECT/release-witness" || exit 125
fi
exec {quoted} "$@"
"##
        ),
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    json!({"name":name,"real":real,"observerSha256":sha(&path),"nativeSha256":sha(real)})
}

/// Prove a fixture cache is owned and writable before any native execution.
pub fn writable_cache(directory: &Path) {
    fs::create_dir_all(directory).unwrap();
    let probe = directory.join(".arc-cache-probe");
    fs::write(&probe, b"offline fixture cache").unwrap();
    assert_eq!(fs::read(&probe).unwrap(), b"offline fixture cache");
    fs::remove_file(probe).unwrap();
}
