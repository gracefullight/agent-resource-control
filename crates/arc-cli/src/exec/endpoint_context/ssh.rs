//! Restricted OpenSSH configuration. Unsupported directives fail closed.
use super::{
    files::{fact, Reads},
    interfaces,
};
use arc_detector::{EndpointContext, EndpointKind};
use std::{
    ffi::{CStr, OsStr},
    net::{IpAddr, SocketAddr},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
};

pub(super) fn capture(
    args: &[String],
    context: &mut EndpointContext,
    reads: &mut Reads,
) -> Option<()> {
    let (mut i, mut config, mut port) = (0, None, None);
    while let Some(arg) = args.get(i) {
        if arg == "--" {
            i += 1;
            break;
        }
        if !arg.starts_with('-') {
            break;
        }
        let (slot, value) = if arg == "-F" || arg == "-p" {
            i += 1;
            (arg.as_str(), args.get(i)?.as_str())
        } else if let Some(v) = arg.strip_prefix("-F") {
            ("-F", v)
        } else if let Some(v) = arg.strip_prefix("-p") {
            ("-p", v)
        } else {
            return None;
        };
        if value.is_empty() {
            return None;
        }
        if slot == "-F" {
            if config.replace(value).is_some() {
                return None;
            }
        } else if port
            .replace(value.parse::<u16>().ok().filter(|p| *p != 0)?)
            .is_some()
        {
            return None;
        }
        i += 1;
    }
    let destination = args.get(i)?;
    if destination.is_empty() || args.get(i + 1).is_none() {
        return None;
    }
    let endpoint = endpoint(destination, config.map(Path::new), port, reads)?;
    interfaces::capture(context)?;
    fact(context, EndpointKind::SshDestination, destination, endpoint)
}

/// Bind a Git-selected SSH host using the same default OpenSSH inputs.
pub(super) fn bind_git(host: &str, context: &mut EndpointContext, reads: &mut Reads) -> Option<()> {
    let endpoint = endpoint(host, None, None, reads)?;
    interfaces::capture(context)?;
    fact(context, EndpointKind::SshDestination, host, endpoint)
}

fn endpoint(
    destination: &str,
    config: Option<&Path>,
    port: Option<u16>,
    reads: &mut Reads,
) -> Option<String> {
    let (paths, required) = if let Some(config) = config {
        // -F replaces both the account-home and system-wide files.
        (vec![config.to_path_buf()], true)
    } else {
        let home = account_home()?;
        // Avoid consulting another account's configuration when HOME has been
        // deliberately isolated. OpenSSH itself uses the account's home.
        if std::env::var_os("HOME").is_some_and(|value| value.as_os_str() != home.as_os_str()) {
            return None;
        }
        (
            vec![
                home.join(".ssh/config"),
                PathBuf::from("/etc/ssh/ssh_config"),
            ],
            false,
        )
    };
    resolve_files(destination, &paths, required, port, reads)
}

fn account_home() -> Option<PathBuf> {
    // OpenSSH uses getuid/getpwuid, not HOME, for its default config path.
    let mut account: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = ptr::null_mut();
    let mut buffer = vec![0u8; 16 * 1024];
    let error = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            &mut account,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if error != 0 || result.is_null() || account.pw_dir.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(account.pw_dir) }.to_bytes();
    if bytes.len() > arc_detector::MAX_CONTEXT_VALUE_BYTES {
        return None;
    }
    let home = PathBuf::from(OsStr::from_bytes(bytes));
    home.is_absolute().then_some(home)
}

fn resolve_files(
    destination: &str,
    paths: &[PathBuf],
    required: bool,
    cli_port: Option<u16>,
    reads: &mut Reads,
) -> Option<String> {
    let mut settings = Settings::default();
    for path in paths {
        let Some(bytes) = reads.read(path)? else {
            if required {
                return None;
            } else {
                continue;
            }
        };
        parse(&bytes, destination, &mut settings)?;
    }
    let host = destination.rsplit('@').next()?;
    let hostname = settings.hostname.as_deref().unwrap_or(host);
    let ip: IpAddr = hostname.parse().ok()?;
    Some(format!(
        "ssh://{}",
        SocketAddr::new(ip, cli_port.or(settings.port).unwrap_or(22))
    ))
}

#[derive(Default)]
struct Settings {
    hostname: Option<String>,
    port: Option<u16>,
}
fn parse(bytes: &[u8], destination: &str, settings: &mut Settings) -> Option<()> {
    let input = std::str::from_utf8(bytes).ok()?;
    let host = destination.rsplit('@').next()?;
    let mut active = true;
    for line in input.lines() {
        let line = line.split('#').next()?.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains(['\'', '"', '\\', '%']) {
            return None;
        }
        let mut parts = line
            .split(|c: char| c.is_ascii_whitespace() || c == '=')
            .filter(|s| !s.is_empty());
        let key = parts.next()?.to_ascii_lowercase();
        let values: Vec<_> = parts.collect();
        if key == "host" {
            if values.is_empty()
                || values
                    .iter()
                    .any(|v| v.starts_with('!') || (v.contains(['*', '?']) && *v != "*"))
            {
                return None;
            }
            active = values
                .iter()
                .any(|v| *v == "*" || v.eq_ignore_ascii_case(host));
            continue;
        }
        if matches!(key.as_str(), "include" | "match") {
            return None;
        }
        if !active {
            continue;
        }
        if values.len() != 1 {
            return None;
        }
        let value = values[0];
        match key.as_str() {
            "hostname" => {
                if settings.hostname.is_none() {
                    settings.hostname = Some(value.to_owned());
                }
            }
            "port" => {
                if settings.port.is_none() {
                    settings.port = Some(value.parse::<u16>().ok().filter(|p| *p != 0)?);
                }
            }
            "proxycommand" | "proxyjump" if value.eq_ignore_ascii_case("none") => {}
            "canonicalizehostname" if value.eq_ignore_ascii_case("no") => {}
            "user"
            | "identityfile"
            | "identitiesonly"
            | "serveraliveinterval"
            | "serveralivecountmax"
            | "stricthostkeychecking"
            | "userknownhostsfile"
            | "loglevel" => {}
            _ => return None,
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn host_first_value_and_proxy_rejection() {
        let data = b"Host worker\n HostName 192.0.2.24\n Port 2222\n ProxyJump none\nHost *\n HostName 192.0.2.25\n Port 22\n";
        let mut settings = Settings::default();
        parse(data, "user@worker", &mut settings).unwrap();
        assert_eq!(
            (settings.hostname.as_deref(), settings.port),
            (Some("192.0.2.24"), Some(2222))
        );
        for invalid in [
            b"Host worker\n HostName 192.0.2.24\n ProxyJump jump\n".as_slice(),
            b"Include other\nHost *\n HostName 192.0.2.24\n",
            b"Match exec test\n HostName 192.0.2.24\n",
        ] {
            assert!(parse(invalid, "worker", &mut Settings::default()).is_none());
        }
    }

    #[test]
    fn user_file_precedes_system_and_command_port_precedes_both() {
        let root = std::env::temp_dir().join(format!("arc-ssh-config-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let user = root.join("user");
        let system = root.join("system");
        fs::write(&user, "Host worker\n HostName 192.0.2.24\n Port 2222\n").unwrap();
        fs::write(&system, "Host *\n HostName 192.0.2.25\n Port 22\n").unwrap();
        let paths = [user, system];
        assert_eq!(
            resolve_files("worker", &paths, false, None, &mut Reads::new()).as_deref(),
            Some("ssh://192.0.2.24:2222")
        );
        assert_eq!(
            resolve_files("worker", &paths, false, Some(2022), &mut Reads::new()).as_deref(),
            Some("ssh://192.0.2.24:2022")
        );
    }

    #[test]
    fn resolved_git_host_is_compatible_with_the_shared_detector() {
        let root = std::env::temp_dir().join(format!("arc-git-ssh-config-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let user = root.join("user");
        let system = root.join("system");
        fs::write(&user, "Host github.com\n HostName 192.0.2.24\n").unwrap();
        fs::write(&system, "Host *\n Port 22\n").unwrap();
        let endpoint = resolve_files(
            "github.com",
            &[user, system],
            false,
            None,
            &mut Reads::new(),
        )
        .unwrap();
        let mut context = EndpointContext {
            captured_at_ms: 10_000,
            ..Default::default()
        };
        fact(
            &mut context,
            EndpointKind::GitRemote,
            "origin",
            "ssh://git@github.com/org/repo.git".into(),
        )
        .unwrap();
        fact(
            &mut context,
            EndpointKind::SshDestination,
            "github.com",
            endpoint,
        )
        .unwrap();
        fact(
            &mut context,
            EndpointKind::SshDestination,
            "@local-addresses",
            "192.0.2.5".into(),
        )
        .unwrap();
        let argv = ["git", "push", "origin", "main"].map(std::ffi::OsString::from);
        let result = arc_detector::classify_argv_with_context(&argv, &context, 10_000);
        assert_eq!(result.classification, arc_detector::JobClass::RemoteCi);
    }
}
