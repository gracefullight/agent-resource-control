//! Exercise the worker against shared modules before production registration.
// The temporary integration target does not consume every shared runtime API.
#![allow(dead_code)]

#[path = "../src/admission.rs"]
mod admission;
#[path = "../src/connection.rs"]
mod connection;
#[path = "../src/job_meter_dispatch.rs"]
mod job_meter_dispatch;
#[path = "../src/transport.rs"]
mod transport;
#[path = "../src/transport_test_support.rs"]
mod transport_test_support;

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
