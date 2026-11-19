//! 「アップデートを確認…」: the newest release on GitHub, and updating through Homebrew as the
//! Tauri version did (`brew upgrade --cask nemooon/tap/track`, then clearing the quarantine flag).
//! The network is reached with the system's curl; nothing of the user's data is sent.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/nemooon/track/releases/latest";
pub const HOMEBREW_TRACK_CASK: &str = "nemooon/tap/track";
pub const XATTR: &str = "/usr/bin/xattr";
/// What someone who installed from the zip or DMG runs after replacing Track.app.
pub const QUARANTINE_COMMAND: &str = "xattr -dr com.apple.quarantine /Applications/Track.app";
const CURL: &str = "/usr/bin/curl";

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Without the leading `v`.
    pub version: String,
    /// The release notes (Markdown).
    pub notes: String,
    pub url: String,
}

/// The release in a GitHub "latest release" response, if it names one.
pub fn parse_release(json: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let tag = v.get("tag_name")?.as_str()?;
    Some(Release {
        version: tag.trim_start_matches('v').to_string(),
        notes: v.get("body").and_then(|b| b.as_str()).unwrap_or_default().trim().to_string(),
        url: v.get("html_url").and_then(|u| u.as_str()).unwrap_or_default().to_string(),
    })
}

/// Whether `candidate` is a later version than `current`: numbers compared as numbers, and a
/// prerelease (`0.8.0-alpha.1`) before its release (`0.8.0`).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn split(v: &str) -> (Vec<u64>, Option<Vec<String>>) {
        let v = v.trim_start_matches('v');
        let (core, pre) = match v.split_once('-') {
            Some((c, p)) => (c, Some(p.split('.').map(str::to_string).collect())),
            None => (v, None),
        };
        (core.split('.').map(|p| p.parse().unwrap_or(0)).collect(), pre)
    }
    let ((a, ap), (b, bp)) = (split(candidate), split(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    match (ap, bp) {
        (None, None) => false,
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (Some(x), Some(y)) => {
            for (p, q) in x.iter().zip(y.iter()) {
                let ord = match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(m), Ok(n)) => m.cmp(&n),
                    _ => p.cmp(q),
                };
                if ord != std::cmp::Ordering::Equal {
                    return ord == std::cmp::Ordering::Greater;
                }
            }
            x.len() > y.len()
        }
    }
}

/// Reads the newest release from GitHub.
pub fn fetch_latest() -> Result<Release, String> {
    let output = Command::new(CURL)
        .args(["-fsSL", "--max-time", "15", "-H", "Accept: application/vnd.github+json", LATEST_RELEASE_URL])
        .output()
        .map_err(|e| format!("アップデートを確認できませんでした: {e}"))?;
    if !output.status.success() {
        return Err(format!("アップデートを確認できませんでした。\n\n{}", command_error(&output)));
    }
    parse_release(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| "リリースの情報を読めませんでした。".to_string())
}

pub fn homebrew_executable() -> Option<PathBuf> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"].into_iter().map(PathBuf::from).find(|p| p.is_file())
}

/// Where Homebrew keeps what it installed (Apple silicon, then Intel).
pub const CASKROOMS: [&str; 2] = ["/opt/homebrew/Caskroom", "/usr/local/Caskroom"];

/// Whether Homebrew installed `app`: some `<caskroom>/track/<version>/Track.app` leads to it.
/// Only files are read, so the update window can ask before anything runs.
pub fn installed_by_homebrew(caskrooms: &[&Path], app: &Path) -> bool {
    let Ok(app) = app.canonicalize() else { return false };
    caskrooms.iter().filter_map(|room| std::fs::read_dir(room.join("track")).ok()).flatten().flatten().any(|version| version.path().join("Track.app").canonicalize().is_ok_and(|p| p == app))
}

fn command_error(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() };
    if detail.is_empty() { format!("終了コード: {}", output.status) } else { detail.to_string() }
}

/// The version in `brew list --cask --versions track` output (`track 0.8.0`).
pub fn parse_brew_cask_version(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output).split_whitespace().nth(1).map(str::to_string)
}

fn installed_track_version(brew: &Path) -> Result<Option<String>, String> {
    let output = Command::new(brew)
        .args(["list", "--cask", "--versions", "track"])
        .output()
        .map_err(|e| format!("Homebrewを実行できません: {e}"))?;
    Ok(output.status.success().then(|| parse_brew_cask_version(&output.stdout)).flatten())
}

fn remove_quarantine(xattr: &Path, app: &Path) -> Result<(), String> {
    let output = Command::new(xattr)
        .args(["-dr", "com.apple.quarantine"])
        .arg(app)
        .output()
        .map_err(|e| format!("アップデートは完了しましたが、隔離属性を削除できませんでした: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "アップデートは完了しましたが、隔離属性を削除できませんでした。\n\n{}\n\nターミナルから次のコマンドを実行してください:\n\nxattr -dr com.apple.quarantine \"{}\"",
            command_error(&output),
            app.display()
        ))
    }
}

/// Upgrades the Track cask to `expected`, checks Homebrew now has it, and clears the quarantine
/// flag on `app` (the cask is not signed by a Developer ID).
pub fn run_homebrew_update(brew: &Path, xattr: &Path, app: &Path, expected: &str) -> Result<(), String> {
    if installed_track_version(brew)?.is_none() {
        return Err("TrackはHomebrewでインストールされていません。ターミナルからアップデートしてください。".into());
    }
    let output = Command::new(brew)
        .args(["upgrade", "--cask", "--no-quit", "--no-ask", HOMEBREW_TRACK_CASK])
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .output()
        .map_err(|e| format!("Homebrewを実行できません: {e}"))?;
    if !output.status.success() {
        return Err(format!("アップデートに失敗しました。\n\n{}", command_error(&output)));
    }
    match installed_track_version(brew)? {
        Some(v) if v == expected => {}
        Some(v) => return Err(format!("Homebrewにはまだバージョン{expected}が反映されていません（現在: {v}）。少し待ってからもう一度お試しください。")),
        None => return Err("アップデート後のバージョンを確認できませんでした。ターミナルから状態を確認してください。".into()),
    }
    remove_quarantine(xattr, app)
}

/// The `.app` this program runs from, if it runs from one.
pub fn current_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().find(|p| p.extension().and_then(|e| e.to_str()) == Some("app")).map(Path::to_path_buf)
}
