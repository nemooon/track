//! `runtime.json`: tells AI skills where the CLI is. Unlike src/server/runtime.ts it is not removed
//! on exit, because the CLI works without the app running.
use crate::error::Result;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Writes `{pid, cliPath, dbPath, startedAt}` to `<data_dir>/runtime.json` (0600, atomic rename).
pub fn write_runtime(data_dir: &Path, cli_path: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(data_dir)?;
    let body = serde_json::json!({
        "pid": std::process::id(),
        "cliPath": cli_path,
        "dbPath": data_dir.join("track.db"),
        "startedAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    });
    let path = data_dir.join("runtime.json");
    let tmp = data_dir.join(format!(".runtime.json.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
    f.write_all(serde_json::to_string_pretty(&body).unwrap().as_bytes())?;
    f.sync_all()?;
    drop(f);
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(path)
}
