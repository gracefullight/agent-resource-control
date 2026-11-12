//! Compile the owned modules before task-4 wires them into the daemon binary.
#![allow(dead_code)]

#[path = "../src/admission.rs"]
mod admission;
#[path = "../src/connection.rs"]
mod connection;
#[path = "../src/limits.rs"]
mod limits;
#[path = "../src/meter.rs"]
mod meter;
#[path = "../src/platform.rs"]
mod platform;
#[path = "../src/runtime_metrics.rs"]
mod runtime_metrics;
#[path = "../src/transport.rs"]
mod transport;
#[path = "../src/transport_test_support.rs"]
mod transport_test_support;

#[path = "../src/transport_tests.rs"]
mod tests;
