//! Development guard: everyday builds may only touch the copy DB under `$HOME/.track-gpui`
//! (see scripts/prepare-db.sh), so working on the app never touches real data. The packaged
//! app is built with the `real-data` feature (scripts/package.sh), which turns the guard off.
use crate::error::{CoreError, Result};
use std::path::{Component, Path, PathBuf};

/// Whether this build is held to the copy DB (and its backups to the copy's folder).
pub const GUARDED: bool = !cfg!(feature = "real-data");

/// `TRACK_DEV_GUARD_ROOT` overrides the allowed directory (tests use a tempdir).
pub fn allowed_root() -> PathBuf {
    std::env::var_os("TRACK_DEV_GUARD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".track-gpui"))
}

pub fn dev_guard(data_dir: &Path) -> Result<()> {
    if !GUARDED {
        return Ok(());
    }
    let ok = data_dir.is_absolute()
        && !data_dir.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        && data_dir == allowed_root();
    if ok {
        Ok(())
    } else {
        Err(CoreError::code_msg(
            "dev_guard",
            format!(
                "開発版はコピー DB（{}）だけを扱います（指定: {}）。TRACK_DATA_DIR=\"$HOME/.track-gpui\" を指定してください。",
                allowed_root().display(),
                data_dir.display()
            ),
        ))
    }
}

/// `TRACK_DATA_DIR`, else `~/.track`.
pub fn data_dir_from_env() -> PathBuf {
    std::env::var_os("TRACK_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".track"))
}

#[cfg(test)]
mod build_tests {
    use super::*;

    /// Everyday builds (and the tests) stay on the copy DB.
    #[cfg(not(feature = "real-data"))]
    #[test]
    fn everyday_builds_keep_the_guard() {
        const { assert!(GUARDED) };
        assert!(dev_guard(&dirs::home_dir().unwrap().join(".track")).is_err());
    }

    /// The alpha (`--features track-core/real-data`) opens the real folder and backs up anywhere.
    /// Only checks paths: nothing is opened or written.
    #[cfg(feature = "real-data")]
    #[test]
    fn the_alpha_build_opens_the_real_folder() {
        const { assert!(!GUARDED) };
        assert!(dev_guard(&dirs::home_dir().unwrap().join(".track")).is_ok());
        assert!(crate::config::dev_guard_dir(std::path::Path::new("/Users/x/Library/Mobile Documents/com~apple~CloudDocs/Track"), &allowed_root()).is_ok());
    }
}
