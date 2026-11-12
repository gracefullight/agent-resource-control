//! Exercise the worker against shared modules before production registration.
// The temporary integration target does not consume every shared runtime API.
#![allow(dead_code)]

#[path = "../src/db_worker.rs"]
mod db_worker;
#[path = "../src/limits.rs"]
mod limits;
#[path = "../src/meter.rs"]
mod meter;
#[path = "../src/platform.rs"]
mod platform;
#[path = "../src/runtime_metrics.rs"]
mod runtime_metrics;
