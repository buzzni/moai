use std::path::{Path, PathBuf};

pub fn dir() -> std::io::Result<PathBuf> {
    let path = if let Some(p) = std::env::var_os("MOAI_CTL_DIR") {
        PathBuf::from(p)
    } else if let Some(p) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(p).join("moai")
    } else {
        std::env::temp_dir().join(format!("moai-{}", uid()))
    };
    ensure(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
#[cfg(not(unix))]
fn uid() -> u32 {
    0
}

fn ensure(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700).recursive(true);
        builder.create(path)?;
        let meta = std::fs::symlink_metadata(path)?;
        if !meta.is_dir() || meta.uid() != uid() || meta.mode() & 0o077 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{} must be a directory owned by this user with mode 0700", path.display()),
            ));
        }
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(path)?;
    Ok(())
}

pub fn socket(dir: &Path) -> std::io::Result<PathBuf> {
    let path = dir.join(format!("{}.sock", std::process::id()));
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let limit = if cfg!(target_os = "macos") { 104 } else { 108 };
        if path.as_os_str().as_bytes().len() >= limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("control socket path is too long: {}", path.display()),
            ));
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(unix)]
    fn a_dir_open_to_others_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("moai-ctl-perms-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(super::ensure(&path).is_err());
        std::fs::remove_dir(&path).unwrap();
    }
}
