use crate::update::{Release, is_newer, parse_brew_cask_version, parse_release, run_homebrew_update};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[test]
fn newer_versions_compare_by_numbers_and_prereleases() {
    assert!(is_newer("0.8.0", "0.7.4"));
    assert!(is_newer("0.10.0", "0.9.9"), "numbers, not text");
    assert!(!is_newer("0.7.4", "0.8.0-alpha.1"), "the Tauri release is older than the GPUI alpha");
    assert!(is_newer("0.8.0", "0.8.0-alpha.1"), "a release beats its prerelease");
    assert!(is_newer("0.8.0-beta.1", "0.8.0-alpha.1"));
    assert!(is_newer("0.8.0-alpha.2", "0.8.0-alpha.1"));
    assert!(!is_newer("0.8.0-alpha.1", "0.8.0-alpha.1"));
    assert!(is_newer("v0.9.0", "0.8.0"), "a leading v is ignored");
}

#[test]
fn reads_the_latest_release() {
    let json = r###"{"tag_name":"v0.9.0","body":"## 変更\n- 速くなった","html_url":"https://github.com/nemooon/track/releases/tag/v0.9.0"}"###;
    assert_eq!(
        parse_release(json),
        Some(Release { version: "0.9.0".into(), notes: "## 変更\n- 速くなった".into(), url: "https://github.com/nemooon/track/releases/tag/v0.9.0".into() })
    );
    assert_eq!(parse_release(r#"{"message":"Not Found"}"#), None);
}

#[test]
fn parses_homebrew_cask_version() {
    assert_eq!(parse_brew_cask_version(b"track 0.4.0\n"), Some("0.4.0".into()));
    assert_eq!(parse_brew_cask_version(b""), None);
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

#[test]
fn runs_the_homebrew_upgrade_and_checks_the_version() {
    let dir = tempfile::tempdir().unwrap();
    let brew = script(
        dir.path(),
        "brew",
        r#"
d=$(dirname "$0")
if [ "$1" = "list" ]; then
  if [ -f "$d/updated" ]; then echo "track 0.9.0"; else echo "track 0.8.0"; fi
elif [ "$1" = "upgrade" ]; then
  printf '%s\n' "$*" > "$d/arguments"
  touch "$d/updated"
else
  exit 1
fi
"#,
    );
    let xattr = script(dir.path(), "xattr", r#"printf '%s\n' "$*" > "$(dirname "$0")/xattr-arguments""#);
    let app = dir.path().join("Track.app");
    run_homebrew_update(&brew, &xattr, &app, "0.9.0").unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("arguments")).unwrap().trim(), "upgrade --cask --no-quit --no-ask nemooon/tap/track");
    assert_eq!(fs::read_to_string(dir.path().join("xattr-arguments")).unwrap().trim(), format!("-dr com.apple.quarantine {}", app.display()));
}

#[test]
fn refuses_when_track_is_not_from_homebrew() {
    let dir = tempfile::tempdir().unwrap();
    let brew = script(dir.path(), "brew", "exit 1");
    let error = run_homebrew_update(&brew, &dir.path().join("xattr"), &dir.path().join("Track.app"), "0.9.0").unwrap_err();
    assert!(error.contains("Homebrewでインストールされていません"), "{error}");
}

#[test]
fn reports_when_the_tap_has_not_caught_up() {
    let dir = tempfile::tempdir().unwrap();
    let brew = script(dir.path(), "brew", r#"if [ "$1" = "list" ]; then echo "track 0.8.0"; fi"#);
    let error = run_homebrew_update(&brew, &dir.path().join("xattr-must-not-run"), &dir.path().join("Track.app"), "0.9.0").unwrap_err();
    assert!(error.contains("まだバージョン0.9.0が反映されていません"), "{error}");
}
