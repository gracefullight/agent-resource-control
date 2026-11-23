//! Optional host-visible native-byte fixture for filesystems such as APFS
//! that cannot create these names themselves. The container engine is used
//! only after the native fixture fails. It has no network, pulls no image and
//! touches only a randomly labelled volume and container owned by this test.
use crate::{journey::Sandbox, runtime::child_watchdog, runtime::instrumentation};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
};

const LABEL: &str = "io.arc.detector.native-bytes";
const SETUP: &str = r#"set -eu
mkdir -p /fixture/packages/a/"$(printf '\377\376')" /fixture/.git/objects/info /fixture/.git/objects/pack /fixture/.git/refs/heads /fixture/.git/refs/tags
printf '%s\n' '{"name":"root","private":true,"workspaces":["packages/*"],"scripts":{"ci":"bun build x.ts"}}' > /fixture/package.json
printf '%s\n' '{"name":"a","scripts":{"ci":"bun test"}}' > /fixture/packages/a/package.json
printf '%s\n' 'ref: refs/heads/main' > /fixture/.git/HEAD
: > /fixture/.git/config
chmod -R a+rX /fixture
"#;

pub struct Volume {
    name: String,
    root: PathBuf,
    docker: PathBuf,
    home: PathBuf,
    config: PathBuf,
    armed: bool,
}

impl Volume {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn create(s: &Sandbox) -> Result<Self, String> {
        // Derive OrbStack's host-visible volume directory from the local
        // socket, not the runner's HOME or the account's Docker config. This
        // also works when the runner itself supplies an isolated HOME.
        let socket = fs::canonicalize("/var/run/docker.sock")
            .map_err(|error| format!("the local Docker socket is unavailable: {error}"))?;
        let run = socket
            .parent()
            .ok_or("the local Docker socket has no directory")?;
        let orbstack = run.parent().ok_or("the local Docker socket has no owner")?;
        if socket.file_name().is_none_or(|name| name != "docker.sock")
            || run.file_name().is_none_or(|name| name != "run")
            || orbstack.file_name().is_none_or(|name| name != ".orbstack")
        {
            return Err("the local Docker socket is not an OrbStack socket".into());
        }
        let volumes = orbstack
            .parent()
            .ok_or("the OrbStack socket has no volume parent")?
            .join("OrbStack/docker/volumes");
        if !volumes.is_dir() {
            return Err("the host-visible OrbStack volume directory is unavailable".into());
        }
        let docker = [
            "/usr/local/bin/docker",
            "/opt/homebrew/bin/docker",
            "/usr/bin/docker",
        ]
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .ok_or("a local Docker CLI is unavailable")?;
        let mut nonce = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut nonce))
            .map_err(|error| format!("cannot create an isolated fixture identity: {error}"))?;
        let token: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let name = format!("arc-detector-native-bytes-{token}");
        let config = s.home.join("detector-docker-config");
        fs::create_dir_all(&config)
            .map_err(|error| format!("cannot create fixture Docker config: {error}"))?;
        let mut fixture = Self {
            root: volumes.join(&name),
            name,
            docker,
            home: s.home.clone(),
            config,
            armed: false,
        };
        let images = fixture.docker(&["image", "ls", "--no-trunc", "--quiet"])?;
        let images: Vec<_> = images
            .lines()
            .filter(|id| id.starts_with("sha256:") && id.len() == 71)
            .take(16)
            .collect();
        if images.is_empty() {
            return Err("no locally available Linux image was found; pulling is disabled".into());
        }
        // Arm before the first mutation: even an uncertain create result is
        // reconciled by the unique ownership label on unwind or error.
        fixture.armed = true;
        let label = format!("{LABEL}={}", fixture.name);
        fixture.docker(&["volume", "create", "--label", &label, &fixture.name])?;
        if fixture.owned_volumes()? != [fixture.name.as_str()] {
            return Err("the created volume did not retain its unique ownership label".into());
        }
        let mount = format!("type=volume,source={},target=/fixture", fixture.name);
        let mut failures = Vec::new();
        let mut prepared = false;
        for image in images {
            match fixture.docker(&[
                "run",
                "--rm",
                "--pull=never",
                "--network=none",
                "--name",
                &fixture.name,
                "--label",
                &label,
                "--mount",
                &mount,
                "--entrypoint",
                "/bin/sh",
                image,
                "-c",
                SETUP,
            ]) {
                Ok(_) => {
                    prepared = true;
                    break;
                }
                Err(error) => {
                    failures.push(error);
                    fixture.cleanup_containers()?;
                }
            }
        }
        if !prepared {
            return Err(format!(
                "no inspected local image could prepare the isolated fixture: {}",
                failures.join("; ")
            ));
        }
        // Enumerate first: the host NFS view may need to learn a byte name
        // before a direct lookup or chdir can resolve it.
        let parent = fixture.root.join("packages/a");
        let entries = fs::read_dir(&parent)
            .map_err(|error| format!("the isolated volume is not host-visible: {error}"))?;
        if !entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().as_bytes() == b"\xff\xfe")
        {
            return Err("the host volume view does not expose the native byte name".into());
        }
        Ok(fixture)
    }

    fn docker(&self, args: &[&str]) -> Result<String, String> {
        let mut command = Command::new(&self.docker);
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .arg("--config")
            .arg(&self.config)
            .args(["--host", "unix:///var/run/docker.sock"])
            .args(args);
        instrumentation::forward(&mut command);
        let output = child_watchdog::output(&mut command, "native-byte fixture Docker command")
            .map_err(|error| format!("Docker {}: {error}", args[0]))?;
        if !output.status.success() {
            return Err(format!(
                "Docker {} failed: {}",
                args[0],
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn owned_volumes(&self) -> Result<Vec<String>, String> {
        let filter = format!("label={LABEL}={}", self.name);
        let output =
            self.docker(&["volume", "ls", "--filter", &filter, "--format", "{{.Name}}"])?;
        Ok(output.lines().map(str::to_owned).collect())
    }

    fn owned_containers(&self) -> Result<Vec<String>, String> {
        let filter = format!("label={LABEL}={}", self.name);
        let output = self.docker(&[
            "container",
            "ls",
            "--all",
            "--filter",
            &filter,
            "--format",
            "{{.ID}} {{.Names}}",
        ])?;
        let mut ids = Vec::new();
        for line in output.lines() {
            let (id, name) = line
                .split_once(' ')
                .ok_or("invalid fixture container listing")?;
            if name != self.name {
                return Err("the fixture container ownership label is not unique".into());
            }
            ids.push(id.to_owned());
        }
        Ok(ids)
    }

    fn cleanup_containers(&self) -> Result<(), String> {
        for id in self.owned_containers()? {
            self.docker(&["container", "rm", "--force", "--volumes", &id])?;
        }
        Ok(())
    }

    fn cleanup(&mut self) -> Result<(), String> {
        if !self.armed {
            return Ok(());
        }
        self.cleanup_containers()?;
        for name in self.owned_volumes()? {
            if name != self.name {
                return Err("the fixture volume ownership label is not unique".into());
            }
            self.docker(&["volume", "rm", &name])?;
        }
        self.armed = false;
        Ok(())
    }

    /// Exercise the same guard that runs if a comparison assertion unwinds,
    /// then verify the engine has neither of this fixture's resources.
    pub fn verify_unwind_cleanup(self) -> Result<(), String> {
        let observer = Self {
            name: self.name.clone(),
            root: self.root.clone(),
            docker: self.docker.clone(),
            home: self.home.clone(),
            config: self.config.clone(),
            armed: false,
        };
        let unwound = std::panic::catch_unwind(move || {
            let _owned = self;
            // Unlike panic!, resume_unwind does not invoke the panic hook.
            std::panic::resume_unwind(Box::new(()));
        });
        if unwound.is_ok() {
            return Err("fixture cleanup probe did not unwind".into());
        }
        if !observer.owned_containers()?.is_empty() || !observer.owned_volumes()?.is_empty() {
            return Err("fixture resources remained after unwinding".into());
        }
        Ok(())
    }
}

impl Drop for Volume {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            if std::thread::panicking() {
                eprintln!("native-byte fixture cleanup could not be confirmed: {error}");
            } else {
                panic!("native-byte fixture cleanup could not be confirmed: {error}");
            }
        }
    }
}
