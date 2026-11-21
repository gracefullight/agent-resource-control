//! Instrumentation values deliberately retained across isolated fixture execs.
#![allow(dead_code)] // Each integration binary uses the subset it needs.
use std::{collections::BTreeMap, process::Command};

pub fn forward(command: &mut Command) {
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
}

pub fn include_expected(expected: &mut BTreeMap<String, String>) {
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
        expected.insert("LLVM_PROFILE_FILE".into(), profile);
    }
}

pub fn normalize_actual(
    actual: &mut BTreeMap<String, String>,
    expected: &BTreeMap<String, String>,
) {
    if expected.contains_key("LLVM_PROFILE_FILE") {
        // compiler-rt may add this process-internal initialization marker.
        actual.remove("__LLVM_PROFILE_RT_INIT_ONCE");
    }
}
