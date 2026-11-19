//! Preserve the caller's native-tool environment at the single spawn boundary.
use arc_detector::Classification;
use arc_ipc::traffic::LineageContext;
use std::{
    collections::BTreeMap,
    ffi::{CString, OsStr, OsString},
    io,
    os::unix::{ffi::OsStrExt, process::CommandExt},
    process::Command,
    ptr,
};

/// Supported shells set `_` to the executable they invoke. If they set it to
/// this wrapper, restore the PATH spelling they would have used for the tool.
/// Otherwise retain the inherited value unchanged.
pub(super) fn restore_shell_underscore(command: &mut Command, program: &OsStr) {
    let (Some(underscore), Ok(wrapper), Some(path)) = (
        std::env::var_os("_"),
        std::env::current_exe(),
        std::env::var_os("PATH"),
    ) else {
        return;
    };
    if underscore == wrapper.as_os_str() {
        if let Some(program_path) = crate::hooks::resolved_executable(program, &path) {
            command.env("_", program_path);
        }
    }
}

// Preserve zsh's PATH spelling for scripts' $0, including bare names for
// empty or "." entries. Install direct execve after the normal launch,
// signal, descriptor and witness hooks.
pub(super) fn preserve_path_invocation(command: &mut Command, argv: &[OsString]) -> io::Result<()> {
    let Some(path) = std::env::var_os("PATH") else {
        return Ok(());
    };
    let Some((_, invocation_path)) = crate::hooks::resolved_path_entry(&argv[0], &path) else {
        return Ok(());
    };
    let invocation = DirectInvocation::new(command, argv, invocation_path.as_os_str())?;
    // SAFETY: the closure performs only execve and an errno read after fork;
    // all memory and pointer arrays are prepared and owned before spawning.
    unsafe { command.pre_exec(move || invocation.exec()) };
    Ok(())
}

struct DirectInvocation {
    program: CString,
    _arguments: Vec<CString>,
    _environment: Vec<CString>,
    arguments: Vec<*const libc::c_char>,
    environment: Vec<*const libc::c_char>,
}

// The pointers address immutable CString allocations owned by this structure;
// moving it does not move those allocations and no pointer is mutated.
unsafe impl Send for DirectInvocation {}
unsafe impl Sync for DirectInvocation {}

impl DirectInvocation {
    fn new(command: &Command, argv: &[OsString], path: &OsStr) -> io::Result<Self> {
        let invalid = |_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in exec input");
        let program = CString::new(path.as_bytes()).map_err(invalid)?;
        let arguments: Vec<CString> = argv
            .iter()
            .map(|arg| CString::new(arg.as_bytes()).map_err(invalid))
            .collect::<io::Result<_>>()?;
        let mut variables: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
        for (key, value) in command.get_envs() {
            if let Some(value) = value {
                variables.insert(key.to_owned(), value.to_owned());
            } else {
                variables.remove(key);
            }
        }
        let environment: Vec<CString> = variables
            .into_iter()
            .map(|(key, value)| {
                let mut entry = key.as_bytes().to_vec();
                entry.push(b'=');
                entry.extend_from_slice(value.as_bytes());
                CString::new(entry).map_err(invalid)
            })
            .collect::<io::Result<_>>()?;
        let mut argument_pointers: Vec<_> = arguments.iter().map(|value| value.as_ptr()).collect();
        let mut environment_pointers: Vec<_> =
            environment.iter().map(|value| value.as_ptr()).collect();
        argument_pointers.push(ptr::null());
        environment_pointers.push(ptr::null());
        Ok(Self {
            program,
            _arguments: arguments,
            _environment: environment,
            arguments: argument_pointers,
            environment: environment_pointers,
        })
    }

    fn exec(&self) -> io::Result<()> {
        // SAFETY: all strings and null-terminated pointer arrays remain alive.
        unsafe {
            libc::execve(
                self.program.as_ptr(),
                self.arguments.as_ptr(),
                self.environment.as_ptr(),
            )
        };
        Err(io::Error::last_os_error())
    }
}

pub(super) fn configure(
    command: &mut Command,
    classification: Classification,
    context: &LineageContext,
    preserve_environment: bool,
) {
    configure_for_caller(
        command,
        classification,
        context,
        std::env::var_os(super::traffic::CONTEXT_ENV).is_some(),
        preserve_environment,
    );
}

fn configure_for_caller(
    command: &mut Command,
    classification: Classification,
    context: &LineageContext,
    supplied: bool,
    preserve_environment: bool,
) {
    if !preserve_environment && !classification.is_native_tool() && !supplied {
        command.env(super::traffic::CONTEXT_ENV, context.as_str());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_detector::Reason;
    use std::ffi::OsString;

    #[test]
    fn native_environment_all_seven_routes_preserve_inherited_lineage() {
        let key = super::super::traffic::CONTEXT_ENV;
        let context = LineageContext::try_from("a".repeat(64)).unwrap();
        for (argv, reason) in [
            (vec!["go", "build"], Reason::GoBuild),
            (vec!["go", "test"], Reason::GoTest),
            (vec!["go", "vet"], Reason::GoVet),
            (vec!["swift", "build"], Reason::SwiftBuild),
            (vec!["swift", "test"], Reason::SwiftTest),
            (vec!["cmake", "--build", "."], Reason::CmakeBuild),
            (vec!["ctest"], Reason::Ctest),
        ] {
            let tokens: Vec<OsString> = argv.iter().map(OsString::from).collect();
            let classification = arc_detector::classify_argv(&tokens);
            assert_eq!(classification.reason, reason);
            assert!(classification.is_native_tool());
            for supplied in [false, true] {
                let mut command = Command::new(argv[0]);
                configure_for_caller(&mut command, classification, &context, supplied, false);
                assert!(
                    command.get_envs().all(|(name, _)| name != key),
                    "{argv:?}, supplied={supplied}"
                );
            }
        }
    }

    #[test]
    fn native_environment_other_families_keep_legacy_injection_only_when_absent() {
        let context = LineageContext::try_from("a".repeat(64)).unwrap();
        let tokens = [OsString::from("cargo"), OsString::from("test")];
        let classification = arc_detector::classify_argv(&tokens);
        for supplied in [false, true] {
            let mut command = Command::new("cargo");
            configure_for_caller(&mut command, classification, &context, supplied, false);
            let actual = command
                .get_envs()
                .find(|(name, _)| *name == super::super::traffic::CONTEXT_ENV);
            assert_eq!(actual.is_some(), !supplied);
            if let Some((_, Some(value))) = actual {
                assert_eq!(value, context.as_str());
            }
        }
    }

    #[test]
    fn hook_environment_keeps_absent_empty_and_supplied_lineage() {
        let context = LineageContext::try_from("a".repeat(64)).unwrap();
        for words in [&["cargo", "build"][..], &["bun", "run", "build"][..]] {
            let argv: Vec<OsString> = words.iter().map(OsString::from).collect();
            let classification = arc_detector::classify_argv(&argv);
            // Both empty and nonempty values are supplied to the child by the
            // inherited environment. No environment operation is added here.
            for supplied in [false, true] {
                let mut command = Command::new(words[0]);
                configure_for_caller(&mut command, classification, &context, supplied, true);
                assert!(command
                    .get_envs()
                    .all(|(name, _)| name != super::super::traffic::CONTEXT_ENV));
            }
        }
    }
}
