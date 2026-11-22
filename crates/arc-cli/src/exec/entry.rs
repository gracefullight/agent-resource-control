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

/// The hook route accepts supported literal argv and its silent gh path probe;
/// execution preserves the caller's environment without a lineage variable.
/// `request` links this stage's job record to its request and `event` names
/// the vendor hook event this run delivers; neither reaches a child.
pub(crate) fn run_hook(
    argv: Vec<OsString>,
    request: Option<String>,
    event: Option<String>,
) -> ExitCode {
    let (Ok(request), Ok(event)) = (
        super::stages::parse_request(request),
        super::event_key::parse(event),
    ) else {
        return super::invalid();
    };
    if let [option, selected] = argv.as_slice() {
        if option == "--arc-check-gh-path" {
            return if crate::hooks::matches_gh_shell_path(selected) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
    }
    if !crate::hooks::literal_argv(&argv) {
        return super::invalid();
    }
    super::event_key::with_event(event, || {
        super::stages::with_request(request, || {
            super::run_with_environment(argv, true, &Instant::now)
        })
    })
}
