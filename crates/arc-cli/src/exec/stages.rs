//! Mixed command stages on the execution route (AT-TRF-12, DET-016).
//!
//! A stage's `hook-exec` names the request it belongs to with `--arc-request
//! <id>`, and its job record is linked to that request once the stage is
//! registered. The option is consumed here; the stage child's argv,
//! environment and streams never carry it. No stage is scheduled by arcctl:
//! the shell keeps `&&`, and each stage is one ordinary managed child.
//!
//! `arcctl exec -- bash -c <source>` separates exactly the two compounds of
//! `compound`, and only when this run recorded the source as MIXED: the bash
//! child stays the request record and receives the per-stage source, with
//! this run's job id as the request. Every other argv is launched unchanged.
// Also the hook route's definition, which alone admits the stage argv.
#[allow(clippy::duplicate_mod, dead_code)]
#[path = "../hooks/stages/compound.rs"]
mod compound;

use super::{traffic, transport};
use arc_ipc::{
    jobs::{JobId, ProcessIdentity},
    request::{RequestId, StageRequest, OPERATION},
};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::Path,
    process::Command,
};

thread_local! {
    static REQUEST: RefCell<Option<RequestId>> = const { RefCell::new(None) };
}

/// The `hook-exec --arc-request` value. Absent is the unlinked route; a
/// malformed value was not written by the hook and is rejected.
pub(super) fn parse_request(value: Option<String>) -> Result<Option<RequestId>, ()> {
    value
        .map(|value| RequestId::try_from(value).map_err(|_| ()))
        .transpose()
}

/// Scope one stage run to its request.
pub(super) fn with_request<T>(request: Option<RequestId>, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<RequestId>);
    impl Drop for Reset {
        fn drop(&mut self) {
            REQUEST.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let previous = REQUEST.with(|slot| slot.replace(request));
    let _reset = Reset(previous);
    run()
}

/// Link this registered stage to the scoped request. Metadata only: a lost,
/// refused or unsupported link never delays or prevents the stage.
pub(super) fn link(
    socket: &Path,
    job_id: &JobId,
    observer: &ProcessIdentity,
    tracking: &traffic::Mode,
) {
    let Some(request_id) = REQUEST.with(|slot| slot.borrow().clone()) else {
        return;
    };
    if !tracking.authoritative() {
        return;
    }
    let request = StageRequest::Link {
        request_id,
        job_id: job_id.clone(),
        observer: observer.clone(),
    };
    let _ = transport::call(socket, OPERATION, serde_json::json!(request));
}

/// The native child of one execution request. `request` is the id of the
/// record this run registered. Only the source of a separable compound is
/// replaced; the program, its option and every other argv are unchanged.
pub(super) fn command(
    argv: &[OsString],
    request: &JobId,
    classification: arc_detector::Classification,
) -> Command {
    // Only a MIXED record can be separated; every other run reads nothing.
    let staged = (classification.classification == arc_detector::JobClass::Mixed)
        .then(std::env::current_exe)
        .and_then(Result::ok)
        .and_then(|executable| {
            separated(
                argv,
                request,
                classification,
                &executable,
                std::env::vars_os().map(|(name, _)| name),
            )
        });
    let mut command = Command::new(&argv[0]);
    match staged {
        Some(source) => command.arg(&argv[1]).arg(source),
        None => command.args(&argv[1..]),
    };
    command
}

/// The per-stage source for `bash -c <compound>`, or `None` for pass-through.
/// The classifier has already proven the remote CI endpoint and, for `make
/// test`, the literal recipe: anything else is not recorded MIXED with no
/// uncertainty. Inherited state that changes how the nested bash starts or
/// resolves a stage program keeps the user's source.
fn separated(
    argv: &[OsString],
    request: &JobId,
    classification: arc_detector::Classification,
    executable: &Path,
    mut environment: impl Iterator<Item = OsString>,
) -> Option<String> {
    use arc_detector::{Classification, JobClass, Locality, Reason, Uncertainty};
    let [program, option, source] = argv else {
        return None;
    };
    let mixed = Classification {
        classification: JobClass::Mixed,
        locality: Locality::Mixed,
        reason: Reason::CompoundMixed,
        uncertainty: Uncertainty::None,
    };
    if program != "bash" || option != "-c" || classification != mixed {
        return None;
    }
    if environment.any(|name| alters_bash_startup(&name)) {
        return None;
    }
    compound::rewrite(source.to_str()?, executable, request.as_str())
}

/// `BASH_ENV` is sourced before the command string, an exported function (in
/// either the upstream or the macOS system Bash spelling) can replace a stage
/// program, and exported shell options change tracing.
fn alters_bash_startup(name: &OsStr) -> bool {
    let name = name.as_bytes();
    matches!(name, b"BASH_ENV" | b"SHELLOPTS" | b"BASHOPTS")
        || name.starts_with(b"BASH_FUNC_")
        || name.starts_with(b"__BASH_FUNC<")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_option_is_a_scoped_job_shaped_id_and_never_reaches_the_child() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(parse_request(None), Ok(None));
        let parsed = parse_request(Some(id.into())).unwrap().unwrap();
        assert_eq!(parsed.as_str(), id);
        for malformed in ["", "bun test", "0123456789ABCDEF0123456789ABCDEF", &id[1..]] {
            assert_eq!(
                parse_request(Some(malformed.into())),
                Err(()),
                "{malformed}"
            );
        }
        let scoped = || REQUEST.with(|slot| slot.borrow().clone());
        assert_eq!(scoped(), None);
        with_request(Some(parsed.clone()), || {
            assert_eq!(scoped(), Some(parsed.clone()));
            with_request(None, || assert_eq!(scoped(), None));
            assert_eq!(scoped(), Some(parsed.clone()));
        });
        assert_eq!(scoped(), None);

        // An unseparable compound keeps the user's argv exactly.
        let argv: Vec<OsString> = [
            "bash",
            "-c",
            "bun test $(printf x) && gh workflow run ci.yml",
        ]
        .map(OsString::from)
        .to_vec();
        let classification = arc_detector::classify_argv(&argv);
        let native = command(&argv, &parsed, classification);
        assert_eq!(native.get_program(), "bash");
        assert_eq!(native.get_args().collect::<Vec<_>>(), &argv[1..]);
        assert_eq!(native.get_envs().count(), 0);
    }

    const MIXED: arc_detector::Classification = arc_detector::Classification {
        classification: arc_detector::JobClass::Mixed,
        locality: arc_detector::Locality::Mixed,
        reason: arc_detector::Reason::CompoundMixed,
        uncertainty: arc_detector::Uncertainty::None,
    };

    fn argv(words: [&str; 3]) -> Vec<OsString> {
        words.map(OsString::from).to_vec()
    }

    fn request() -> JobId {
        JobId::try_from("0123456789abcdef0123456789abcdef".to_owned()).unwrap()
    }

    #[test]
    fn bash_source_of_a_mixed_compound_becomes_its_two_stages_for_this_request() {
        let wrapper =
            "'/opt/arc/arcctl' hook-exec --arc-request 0123456789abcdef0123456789abcdef -- ";
        for test in ["bun test", "make test"] {
            let source = format!("{test} && gh workflow run ci.yml");
            let staged = separated(
                &argv(["bash", "-c", &source]),
                &request(),
                MIXED,
                Path::new("/opt/arc/arcctl"),
                ["HOME", "PATH", "BASH", "BASH_FUNCTIONS"]
                    .map(OsString::from)
                    .into_iter(),
            );
            assert_eq!(
                staged,
                Some(format!(
                    "{wrapper}{test} && {wrapper}gh workflow run ci.yml"
                ))
            );
        }
        // The running binary is the wrapper; only the source argument changes.
        let words = argv(["bash", "-c", "bun test && gh workflow run ci.yml"]);
        let launched = command(&words, &request(), MIXED);
        let arguments: Vec<_> = launched.get_args().collect();
        assert_eq!(launched.get_program(), "bash");
        assert_eq!(arguments[0], "-c");
        assert_eq!(launched.get_envs().count(), 0);
        if std::env::vars_os().any(|(name, _)| alters_bash_startup(&name)) {
            assert_eq!(arguments[1], words[2].as_os_str());
        } else {
            let source = arguments[1].to_str().unwrap();
            assert_eq!(source.matches(" hook-exec --arc-request ").count(), 2);
            assert!(source.ends_with(" -- gh workflow run ci.yml"), "{source}");
        }
    }

    #[test]
    fn every_other_argv_record_or_startup_state_keeps_the_users_source() {
        let executable = Path::new("/opt/arc/arcctl");
        let source = "bun test && gh workflow run ci.yml";
        let clean = || ["HOME", "PATH"].map(OsString::from).into_iter();
        let staged = |words: &[OsString], classification| {
            separated(words, &request(), classification, executable, clean())
        };
        assert!(staged(&argv(["bash", "-c", source]), MIXED).is_some());
        for words in [
            argv(["sh", "-c", source]),
            argv(["zsh", "-c", source]),
            argv(["/bin/bash", "-c", source]),
            argv(["bash", "-lc", source]),
            argv([
                "bash",
                "-c",
                "bun test $(printf x) && gh workflow run ci.yml",
            ]),
            argv(["bash", "-c", "bun test && gh workflow run ci.yml > log"]),
            argv(["bash", "-c", "bun test && gh workflow run \"$WORKFLOW\""]),
            argv(["bash", "-c", "bash -c 'bun test && gh workflow run ci.yml'"]),
        ] {
            assert_eq!(staged(&words, MIXED), None, "{words:?}");
        }
        let mut long = argv(["bash", "-c", source]);
        long.push("extra".into());
        assert_eq!(staged(&long, MIXED), None);
        assert_eq!(staged(&long[..2], MIXED), None);

        // Only a record with no uncertainty: an unproven endpoint or a changed
        // Makefile is recorded UNKNOWN/OBSERVE and passes through.
        let observed = arc_detector::classify_argv(&argv(["bash", "-c", "$UNRESOLVED"]));
        assert_eq!(observed.uncertainty, arc_detector::Uncertainty::Observe);
        assert_eq!(staged(&argv(["bash", "-c", source]), observed), None);
        let local = arc_detector::classify_argv(&argv(["bun", "test", "x"])[..2]);
        assert_eq!(staged(&argv(["bash", "-c", source]), local), None);

        for name in [
            "BASH_ENV",
            "SHELLOPTS",
            "BASHOPTS",
            "BASH_FUNC_bun%%",
            "BASH_FUNC_gh()",
            "__BASH_FUNC<make>()",
        ] {
            assert!(alters_bash_startup(OsStr::new(name)), "{name}");
            let environment = ["PATH", name].map(OsString::from).into_iter();
            assert_eq!(
                separated(
                    &argv(["bash", "-c", source]),
                    &request(),
                    MIXED,
                    executable,
                    environment
                ),
                None,
                "{name}"
            );
        }
        for name in ["BASH", "BASH_VERSION", "BASH_ENVIRONMENT", "ENV", "SHELL"] {
            assert!(!alters_bash_startup(OsStr::new(name)), "{name}");
        }
        // A relative wrapper path cannot be named from the shell's directory.
        assert_eq!(
            separated(
                &argv(["bash", "-c", source]),
                &request(),
                MIXED,
                Path::new("arcctl"),
                clean()
            ),
            None
        );
    }
}
