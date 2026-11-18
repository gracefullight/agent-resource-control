//! Give Vite a private writable temp directory when dependencies are linked.
use std::{
    ffi::CString,
    fs, io,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, MetadataExt},
    },
    path::{Path, PathBuf},
    thread,
};

pub(super) struct Overlay {
    path: PathBuf,
    backup: PathBuf,
    overlay_identity: (u64, u64),
    original_identity: (u64, u64),
}

fn identity(path: &Path) -> io::Result<(u64, u64)> {
    let metadata = fs::symlink_metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}

fn swap(first: &Path, second: &Path) -> io::Result<()> {
    let first = CString::new(first.as_os_str().as_bytes())?;
    let second = CString::new(second.as_os_str().as_bytes())?;
    // SAFETY: both path strings are NUL-terminated and remain live for the call.
    if unsafe { libc::renamex_np(first.as_ptr(), second.as_ptr(), libc::RENAME_SWAP) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(super) fn writable_vite_temp(path: &Path) -> Option<Overlay> {
    let metadata = fs::symlink_metadata(path).expect("installed desktop dependencies");
    if !metadata.file_type().is_symlink() {
        assert!(
            metadata.is_dir(),
            "desktop dependencies must be a directory"
        );
        return None;
    }
    Some(prepare(path, (metadata.dev(), metadata.ino())).expect("private Vite temp overlay"))
}

fn prepare(path: &Path, original_identity: (u64, u64)) -> io::Result<Overlay> {
    let parent = path.parent().expect("node_modules parent");
    let nonce = arc_core::process_identity::random_job_id()?;
    let staged = parent.join(format!(".arc-node-modules-{nonce}"));
    fs::create_dir(&staged)?;
    let staged_result = (|| {
        fs::create_dir(staged.join(".vite-temp"))?;
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            if name != ".vite-temp" {
                // After the atomic swap, staged names the original symlink.
                symlink(staged.join(&name), staged.join(name))?;
            }
        }
        identity(&staged)
    })();
    let overlay_identity = match staged_result {
        Ok(identity) => identity,
        Err(error) => {
            let _ = fs::remove_dir_all(&staged);
            return Err(error);
        }
    };
    if let Err(error) = swap(path, &staged) {
        let _ = fs::remove_dir_all(&staged);
        return Err(error);
    }
    Ok(Overlay {
        path: path.to_owned(),
        backup: staged,
        overlay_identity,
        original_identity,
    })
}

impl Drop for Overlay {
    fn drop(&mut self) {
        let restore = (|| {
            if identity(&self.path)? != self.overlay_identity
                || identity(&self.backup)? != self.original_identity
            {
                return Err(io::Error::other("dependency overlay ownership changed"));
            }
            swap(&self.path, &self.backup)?;
            fs::remove_dir_all(&self.backup)
        })();
        if let Err(error) = restore {
            if thread::panicking() {
                eprintln!("cannot restore {}: {error}", self.path.display());
            } else {
                panic!("cannot restore {}: {error}", self.path.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn linked_dependencies_get_private_temp_and_are_restored_after_unwind() {
        let fixture = crate::native::Fixture::with_binary(PathBuf::new());
        let installed = fixture.root.join("installed");
        let project = fixture.root.join("project");
        fs::create_dir_all(installed.join(".vite-temp")).unwrap();
        fs::create_dir(&project).unwrap();
        fs::write(installed.join("dependency"), "installed").unwrap();
        fs::write(installed.join(".vite-temp/original"), "preserved").unwrap();
        let modules = project.join("node_modules");
        symlink(&installed, &modules).unwrap();
        let original = fs::read_link(&modules).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _overlay = writable_vite_temp(&modules).unwrap();
            assert!(fs::symlink_metadata(&modules).unwrap().is_dir());
            assert_eq!(
                fs::read_to_string(modules.join("dependency")).unwrap(),
                "installed"
            );
            fs::write(modules.join(".vite-temp/private"), "private").unwrap();
            assert!(!installed.join(".vite-temp/private").exists());
            panic!("fixture unwind");
        }));
        assert!(result.is_err());
        assert_eq!(fs::read_link(&modules).unwrap(), original);
        assert_eq!(
            fs::read_to_string(installed.join(".vite-temp/original")).unwrap(),
            "preserved"
        );
        assert!(!installed.join(".vite-temp/private").exists());
        assert_eq!(fs::read_dir(project).unwrap().count(), 1);
    }
}
