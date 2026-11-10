//! Standalone attribution probe: compile with pinned rustc, with and without
//! `-C instrument-coverage --cfg=trybuild_no_target`. No ARC code runs here.
use std::{env, os::unix::ffi::OsStrExt};
fn main() {
    let mut entries: Vec<_> = env::vars_os()
        .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
        .collect();
    entries.sort();
    print!("[");
    for (i, (key, value)) in entries.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        print!("[{key:?},{value:?}]");
    }
    println!("]");
}
