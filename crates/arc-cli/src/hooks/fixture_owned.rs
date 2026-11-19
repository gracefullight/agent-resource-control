//! Test-only child supervision; an unreaped root reserves its process-group ID.
use arc_core::process_identity::ProcessIdentity;
use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

const WATCHDOG: Duration = Duration::from_secs(120);

pub(crate) struct OwnedChild {
    child: Option<Child>,
    marker: Option<marker::Marker>,
    label: String,
    own_group: bool,
    descendants: Vec<ProcessIdentity>,
}

impl OwnedChild {
    pub(crate) fn spawn(command: &mut Command, label: &str) -> Self {
        command.process_group(0);
        Self::start(command, label, false)
    }
    pub(crate) fn spawn_nested(command: &mut Command, label: &str) -> Self {
        // Own a distinct group without replacing the outer worker's inherited
        // marker or changing the child's explicitly controlled environment.
        command.process_group(0);
        Self::start(command, label, true)
    }
    fn start(command: &mut Command, label: &str, nested: bool) -> Self {
        let mut marker = Some(
            if nested {
                marker::Marker::attach_nested(command)
            } else {
                marker::Marker::attach(command)
            }
            .expect("create owned fixture marker"),
        );
        let child = command.spawn().expect("spawn owned fixture child");
        if let Some(marker) = &mut marker {
            marker.close_parent();
        }
        Self {
            child: Some(child),
            marker,
            label: label.into(),
            own_group: true,
            descendants: Vec::new(),
        }
    }
    pub(crate) fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("fixture child already reaped")
    }
    fn root_exited(&self) -> io::Result<bool> {
        let child = self.child.as_ref().expect("fixture child already reaped");
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        #[cfg(target_os = "macos")]
        let pid = info.si_pid;
        #[cfg(target_os = "linux")]
        let pid = unsafe { info.si_pid() };
        Ok(pid != 0
            && matches!(
                info.si_code,
                libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
            ))
    }
    fn observe_marker(&mut self) -> io::Result<()> {
        let root = self.child.as_ref().unwrap().id();
        if let Some(marker) = &self.marker {
            for identity in marker.identities()? {
                if identity.pid != root && !self.descendants.contains(&identity) {
                    self.descendants.push(identity);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn observe(&mut self) -> io::Result<bool> {
        let root = self.child.as_ref().unwrap().id();
        if self.root_exited()? {
            self.observe_marker()?;
        }
        let members = group_members(root)?;
        for pid in members.iter().copied().filter(|pid| *pid != root) {
            if let Ok(identity) = ProcessIdentity::capture(pid) {
                if !self.descendants.contains(&identity) {
                    self.descendants.push(identity);
                }
            }
        }
        let mut present = members.iter().any(|pid| *pid != root);
        for identity in &self.descendants {
            match ProcessIdentity::capture(identity.pid) {
                Ok(current) if current == *identity => present = true,
                Ok(_) => (),
                Err(error) => {
                    // Unknown identity is not evidence of disappearance.
                    if unsafe { libc::kill(identity.pid as i32, 0) } == 0 {
                        present = true;
                    } else if io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(present)
    }
    pub(crate) fn wait(&mut self) -> ExitStatus {
        self.wait_with_clock(Instant::now)
    }
    fn wait_with_clock(&mut self, now: impl Fn() -> Instant) -> ExitStatus {
        let deadline = now() + WATCHDOG;
        loop {
            if self.own_group {
                let exited = self
                    .root_exited()
                    .unwrap_or_else(|e| panic!("{}: cannot observe root: {e}", self.label));
                let descendants = self
                    .observe()
                    .unwrap_or_else(|e| panic!("{}: cannot observe descendants: {e}", self.label));
                if exited && !descendants {
                    let status = self
                        .child_mut()
                        .try_wait()
                        .expect("reap fixture child")
                        .expect("root exit observed");
                    self.child = None;
                    return status;
                }
            } else if let Some(status) = self
                .child_mut()
                .try_wait()
                .expect("observe nested fixture child")
            {
                self.child = None;
                return status;
            }
            assert!(
                now() < deadline,
                "{}: fixture child or descendant did not exit",
                self.label
            );
            thread::yield_now();
        }
    }
    /// Allow the wrapper to forward SIGTERM, then clean any surviving members.
    pub(crate) fn terminate_gracefully(&mut self) {
        if self.child.is_none() {
            return;
        }
        if !self.own_group {
            let child = self.child_mut();
            if matches!(child.try_wait(), Ok(None)) {
                unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
            }
            return;
        }
        match self.root_exited() {
            Ok(false) => {
                unsafe { libc::kill(self.child.as_ref().unwrap().id() as i32, libc::SIGTERM) };
            }
            Ok(true) => (),
            Err(error) => {
                eprintln!("{}: cannot observe fixture root: {error}", self.label);
                return;
            }
        }
        let deadline = Instant::now() + WATCHDOG;
        while matches!(self.root_exited(), Ok(false)) && Instant::now() < deadline {
            thread::yield_now();
        }
        if let Err(error) = self.cleanup() {
            eprintln!("{}: {error}", self.label);
        }
    }
    fn cleanup(&mut self) -> io::Result<()> {
        self.cleanup_with(Self::observe_marker)
    }
    fn cleanup_with(
        &mut self,
        observe_marker: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.child.is_none() {
            return Ok(());
        }
        let pid = self.child.as_ref().unwrap().id();
        let mut errors = Vec::new();
        if self.own_group {
            // waitid validates the unreaped child before signalling its reserved group.
            let reserved = match self.root_exited() {
                Ok(_) => true,
                Err(error) => {
                    errors.push(format!("cannot verify fixture root: {error}"));
                    false
                }
            };
            if let Err(error) = observe_marker(self) {
                errors.push(format!("cannot observe fixture marker: {error}"));
            }
            if let Err(error) = self.observe() {
                errors.push(format!("cannot observe fixture descendants: {error}"));
            }
            if reserved && unsafe { libc::kill(-(pid as i32), libc::SIGKILL) } != 0 {
                let error = io::Error::last_os_error();
                // Darwin may report EPERM for a group whose only remaining
                // member is its exited, unreaped leader. Verify that state
                // before treating the signal as unnecessary.
                let already_gone = matches!(self.root_exited(), Ok(true))
                    && group_members(pid)
                        .is_ok_and(|members| members.iter().all(|member| *member == pid));
                if error.raw_os_error() != Some(libc::ESRCH) && !already_gone {
                    errors.push(format!("cannot terminate fixture group: {error}"));
                }
            } else if !reserved {
                // The Child handle remains ours even if group observation failed.
                if let Err(error) = self.child_mut().kill() {
                    if error.raw_os_error() != Some(libc::ESRCH) {
                        errors.push(format!("cannot terminate fixture root: {error}"));
                    }
                }
            }
            for identity in &self.descendants {
                match ProcessIdentity::capture(identity.pid) {
                    Ok(current) if current == *identity => {
                        if unsafe { libc::kill(identity.pid as i32, libc::SIGKILL) } != 0 {
                            let error = io::Error::last_os_error();
                            if error.raw_os_error() != Some(libc::ESRCH) {
                                errors
                                    .push(format!("cannot terminate fixture descendant: {error}"));
                            }
                        }
                    }
                    Ok(_) => (),
                    Err(_)
                        if unsafe { libc::kill(identity.pid as i32, 0) } != 0
                            && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) => {}
                    Err(error) => errors.push(format!("cannot verify fixture descendant: {error}")),
                }
            }
            let deadline = Instant::now() + WATCHDOG;
            loop {
                match self.observe() {
                    Ok(false) => break,
                    Ok(true) => (),
                    Err(error) => {
                        errors.push(format!("cannot verify fixture cleanup: {error}"));
                        break;
                    }
                }
                if Instant::now() >= deadline {
                    errors.push("fixture descendants survived cleanup".into());
                    break;
                }
                thread::yield_now();
            }
        } else {
            let child = self.child_mut();
            if matches!(child.try_wait(), Ok(None)) {
                let _ = child.kill();
            }
        }
        let deadline = Instant::now() + WATCHDOG;
        loop {
            match self.child_mut().try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                    break;
                }
                Ok(None) => (),
                Err(error) => {
                    errors.push(format!("cannot reap fixture root: {error}"));
                    break;
                }
            }
            if Instant::now() >= deadline {
                errors.push("fixture root survived cleanup".into());
                break;
            }
            thread::yield_now();
        }
        if let Some(marker) = &mut self.marker {
            if errors.is_empty() {
                marker.remove_on_drop();
            } else {
                marker.retain_on_drop();
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
    }
}

mod marker {
    include!("../../../arc-daemon/tests/support/owned_marker.rs");

    impl Marker {
        // Attach a second private descriptor while leaving the outer marker
        // environment untouched. The outer supervisor retains its own proof.
        pub(super) fn attach_nested(command: &mut Command) -> io::Result<Self> {
            let mut unused = Command::new("/dev/null");
            let marker = Self::attach(&mut unused)?;
            let fd = marker.file.as_ref().unwrap().as_raw_fd();
            unsafe {
                command.pre_exec(move || {
                    if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            Ok(marker)
        }
    }
}

#[cfg(test)]
#[path = "fixture_owned_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "fixture_owned_installer_tests.rs"]
mod installer_watchdog_tests;

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("{}: fixture cleanup failed: {error}", self.label);
        }
    }
}

#[cfg(target_os = "macos")]
fn group_members(group: u32) -> io::Result<Vec<u32>> {
    let mut ids = vec![0i32; 128];
    loop {
        unsafe {
            *libc::__error() = 0;
        }
        let bytes = unsafe {
            libc::proc_listpids(
                2,
                group,
                ids.as_mut_ptr().cast(),
                std::mem::size_of_val(ids.as_slice()) as i32,
            )
        };
        if bytes < 0 || (bytes == 0 && io::Error::last_os_error().raw_os_error() != Some(0)) {
            return Err(io::Error::last_os_error());
        }
        let count = bytes as usize / std::mem::size_of::<i32>();
        if count < ids.len() {
            return Ok(ids[..count]
                .iter()
                .copied()
                .filter(|pid| *pid > 0 && unsafe { libc::getpgid(*pid) } == group as i32)
                .map(|pid| pid as u32)
                .collect());
        }
        if ids.len() >= 16_384 {
            return Err(io::Error::other("fixture process group limit"));
        }
        ids.resize(ids.len() * 2, 0);
    }
}
#[cfg(target_os = "linux")]
fn group_members(group: u32) -> io::Result<Vec<u32>> {
    let mut members = Vec::new();
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<i32>().ok())
        {
            if unsafe { libc::getpgid(pid) } == group as i32 {
                members.push(pid as u32);
            }
        }
    }
    Ok(members)
}
