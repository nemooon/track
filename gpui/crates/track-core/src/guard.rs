//! Development guard: while the GPUI version is a prototype it may only touch the
//! copy DB under `$HOME/.track-gpui` (see gpui/scripts/prepare-db.sh). Remove at adoption.
use crate::error::{CoreError, Result};
use std::path::{Component, Path, PathBuf};

/// `TRACK_DEV_GUARD_ROOT` overrides the allowed directory (tests use a tempdir).
pub fn allowed_root() -> PathBuf {
    std::env::var_os("TRACK_DEV_GUARD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".track-gpui"))
}

pub fn dev_guard(data_dir: &Path) -> Result<()> {
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

/// `TRACK_DATA_DIR`, else `~/.track` (same as the Bun server and CLI).
pub fn data_dir_from_env() -> PathBuf {
    std::env::var_os("TRACK_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".track"))
}
