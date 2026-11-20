use std::{ffi::OsString, process::ExitCode, time::Instant};

pub(crate) fn run_with_environment(
    argv: Vec<OsString>,
    preserve_environment: bool,
    now: &dyn Fn() -> Instant,
) -> ExitCode {
    run_with_environment_and_notice(
        argv,
        preserve_environment,
        now,
        super::preparation::write_notice_bounded,
    )
}

pub(crate) fn run_with_environment_and_notice(
    argv: Vec<OsString>,
    preserve_environment: bool,
    now: &dyn Fn() -> Instant,
    writer: fn(&str) -> super::preparation::NoticeWrite,
) -> ExitCode {
    super::preparation::with_notice_writer(writer, || {
        run_with_clocks(argv, preserve_environment, now, &Instant::now)
    })
}

pub(super) fn run_with_clocks(
    argv: Vec<OsString>,
    preserve_environment: bool,
    now: &dyn Fn() -> Instant,
    delivery_now: &dyn Fn() -> Instant,
) -> ExitCode {
    super::run::run_with_classifier(
        argv,
        preserve_environment,
        now,
        delivery_now,
        &super::metadata::classify,
    )
}

pub(crate) fn run(argv: Vec<OsString>) -> ExitCode {
    super::run_with_environment(argv, false, &Instant::now)
}

/// The hook route accepts only the frozen literal argv and preserves the
/// caller's environment rather than adding the legacy lineage variable.
pub(crate) fn run_hook(argv: Vec<OsString>) -> ExitCode {
    if !crate::hooks::literal_argv(&argv) {
        return super::invalid();
    }
    super::run_with_environment(argv, true, &Instant::now)
}
