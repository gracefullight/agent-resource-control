#!/usr/bin/env bash
set -euo pipefail
# Separate setup, never called by the gate. Exact release + upstream Cargo.lock.
mise install
rustup component add llvm-tools-preview --toolchain 1.95.0
cargo +1.95.0 install cargo-llvm-cov --version 0.6.21 --locked
cargo +1.95.0 fetch --locked
