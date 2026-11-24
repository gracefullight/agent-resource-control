//! Detect a settings symlink replaced after the installer resolves it.
use std::{
    fs::{self, Metadata},
    io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

const CHANGED: &str =
    "Codex settings symlink changed during the update; review the current settings and retry.";

pub(super) struct Guard {
    path: PathBuf,
    link: Option<(Metadata, PathBuf)>,
}

impl Guard {
    pub(super) fn capture(path: &Path) -> Result<Self, &'static str> {
        let link = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Some((metadata, fs::read_link(path).map_err(|_| CHANGED)?))
            }
            Ok(_) => None,
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => return Err("Cannot inspect Codex settings."),
        };
        Ok(Self {
            path: path.to_path_buf(),
            link,
        })
    }

    pub(super) fn check(&self) -> Result<(), &'static str> {
        let current = fs::symlink_metadata(&self.path);
        match (&self.link, current) {
            (Some((old, target)), Ok(now))
                if now.file_type().is_symlink()
                    && (old.dev(), old.ino(), old.ctime(), old.ctime_nsec())
                        == (now.dev(), now.ino(), now.ctime(), now.ctime_nsec())
                    && fs::read_link(&self.path).as_ref().ok() == Some(target) =>
            {
                Ok(())
            }
            (None, Ok(now)) if !now.file_type().is_symlink() => Ok(()),
            (None, Err(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            _ => Err(CHANGED),
        }
    }
}
