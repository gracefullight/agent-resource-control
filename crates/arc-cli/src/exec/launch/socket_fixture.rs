use super::*;

pub(super) struct Socket {
    pub(super) root: PathBuf,
    pub(super) listener: UnixListener,
}
impl Socket {
    pub(super) fn new() -> Self {
        let root = PathBuf::from(format!(
            "/tmp/arc-launch-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self {
            listener: UnixListener::bind(root.join("s")).unwrap(),
            root,
        }
    }
    pub(super) fn path(&self) -> PathBuf {
        self.root.join("s")
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
