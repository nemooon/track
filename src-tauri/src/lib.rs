use std::{
    io::{BufRead, BufReader, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
#[cfg(desktop)]
use tauri::Emitter;
use tauri::{Manager, RunEvent, Url};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[cfg(target_os = "macos")]
mod ai_integration {
    use std::{
        env, fs,
        fs::OpenOptions,
        io::{self, ErrorKind, Write},
        os::unix::fs::OpenOptionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use tauri::Manager;

    #[derive(Debug)]
    pub struct InstallResult {
        pub codex_path: PathBuf,
        pub claude_path: PathBuf,
    }

    fn integration_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
        let bundled = app
            .path()
            .resource_dir()
            .map_err(|error| format!("アプリのリソースを取得できません: {error}"))?
            .join("integrations");
        if bundled.is_dir() {
            return Ok(bundled);
        }

        let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("開発用リソースの場所を取得できません")?
            .join("integrations");
        if development.is_dir() {
            return Ok(development);
        }
        Err("AI連携ファイルが見つかりません。アプリを再インストールしてください。".into())
    }

    fn is_track_skill(path: &Path) -> bool {
        fs::read_to_string(path)
            .map(|content| content.lines().any(|line| line.trim() == "name: track"))
            .unwrap_or(false)
    }

    fn is_track_command(path: &Path) -> bool {
        fs::read_to_string(path)
            .map(|content| {
                content.lines().any(|line| line.trim() == "<!-- track:claude-command:v1 -->")
                    || (content.contains("# /track")
                        && (content.contains("個人用工数管理アプリ")
                            || (content.contains("Track (個人の工数管理アプリ)")
                                && content.contains("~/.track/runtime.json")
                                && content.contains("prepare --source claude"))))
            })
            .unwrap_or(false)
    }

    fn directory_is_empty(path: &Path) -> io::Result<bool> {
        Ok(fs::read_dir(path)?.next().is_none())
    }

    fn validate_codex_destination(path: &Path) -> Result<(), String> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {}
            Ok(metadata) if metadata.is_dir() => {
                let skill = path.join("SKILL.md");
                if skill.exists() {
                    if !is_track_skill(&skill) {
                        return Err(format!(
                            "同名のCodexスキルが既にあります: {}",
                            path.display()
                        ));
                    }
                } else if !directory_is_empty(path).map_err(|error| error.to_string())? {
                    return Err(format!(
                        "空ではないCodexスキルフォルダが既にあります: {}",
                        path.display()
                    ));
                }
            }
            Ok(_) => {
                return Err(format!(
                    "Codexスキルのインストール先にファイルがあります: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        Ok(())
    }

    fn validate_claude_destination(path: &Path) -> Result<(), String> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {}
            Ok(metadata) if metadata.is_file() => {
                if !is_track_command(path) {
                    return Err(format!(
                        "同名のClaude Codeコマンドが既にあります: {}",
                        path.display()
                    ));
                }
            }
            Ok(_) => {
                return Err(format!(
                    "Claude Codeコマンドのインストール先にフォルダがあります: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        Ok(())
    }

    fn replace_symlink(path: &Path) -> io::Result<()> {
        if fs::symlink_metadata(path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn copy_atomic(source: &Path, destination: &Path) -> io::Result<()> {
        let parent = destination
            .parent()
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "インストール先が不正です"))?;
        fs::create_dir_all(parent)?;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let filename = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "ファイル名が不正です"))?;
        let temporary = parent.join(format!(".{filename}.{}-{stamp}.tmp", std::process::id()));
        let bytes = fs::read(source)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);

        if let Err(error) = fs::rename(&temporary, destination) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        Ok(())
    }

    fn install_from(root: &Path, home: &Path) -> Result<InstallResult, String> {
        let codex_source = root.join("codex/track");
        let claude_source = root.join("claude/track.md");
        for source in [
            codex_source.join("SKILL.md"),
            codex_source.join("agents/openai.yaml"),
            claude_source.clone(),
        ] {
            if !source.is_file() {
                return Err(format!("AI連携ファイルがありません: {}", source.display()));
            }
        }

        let codex_path = home.join(".codex/skills/track");
        let claude_path = home.join(".claude/commands/track.md");
        let legacy_codex_path = home.join(".agents/skills/track");
        validate_codex_destination(&codex_path)?;
        validate_claude_destination(&claude_path)?;
        let remove_legacy_codex_link = fs::symlink_metadata(&legacy_codex_path)
            .map(|metadata| {
                metadata.file_type().is_symlink()
                    && is_track_skill(&legacy_codex_path.join("SKILL.md"))
            })
            .unwrap_or(false);

        replace_symlink(&codex_path).map_err(|error| error.to_string())?;
        replace_symlink(&claude_path).map_err(|error| error.to_string())?;
        fs::create_dir_all(codex_path.join("agents")).map_err(|error| error.to_string())?;
        copy_atomic(&codex_source.join("SKILL.md"), &codex_path.join("SKILL.md"))
            .map_err(|error| error.to_string())?;
        copy_atomic(
            &codex_source.join("agents/openai.yaml"),
            &codex_path.join("agents/openai.yaml"),
        )
        .map_err(|error| error.to_string())?;
        copy_atomic(&claude_source, &claude_path).map_err(|error| error.to_string())?;
        if remove_legacy_codex_link {
            fs::remove_file(&legacy_codex_path).map_err(|error| error.to_string())?;
        }

        Ok(InstallResult {
            codex_path,
            claude_path,
        })
    }

    pub fn install(app: &tauri::AppHandle) -> Result<InstallResult, String> {
        let root = integration_root(app)?;
        let home = env::var_os("HOME").ok_or("ホームディレクトリを取得できません")?;
        install_from(&root, Path::new(&home))
    }

    #[cfg(test)]
    mod tests {
        use super::install_from;
        use std::{
            fs,
            os::unix::fs::symlink,
            path::{Path, PathBuf},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn test_dir(name: &str) -> PathBuf {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory =
                std::env::temp_dir().join(format!("track-ai-integration-{name}-{stamp}"));
            fs::create_dir_all(&directory).unwrap();
            directory
        }

        fn create_sources(root: &Path, skill: &str, command: &str) {
            fs::create_dir_all(root.join("codex/track/agents")).unwrap();
            fs::create_dir_all(root.join("claude")).unwrap();
            fs::write(root.join("codex/track/SKILL.md"), skill).unwrap();
            fs::write(
                root.join("codex/track/agents/openai.yaml"),
                "interface:\n  display_name: \"Track\"\n",
            )
            .unwrap();
            fs::write(root.join("claude/track.md"), command).unwrap();
        }

        #[test]
        fn installs_and_updates_both_integrations() {
            let directory = test_dir("install");
            let root = directory.join("resources/integrations");
            let home = directory.join("home");
            create_sources(
                &root,
                "---\nname: track\n---\nfirst",
                "# /track\n個人用工数管理アプリ first",
            );
            install_from(&root, &home).unwrap();

            create_sources(
                &root,
                "---\nname: track\n---\nsecond",
                "# /track\n個人用工数管理アプリ second",
            );
            install_from(&root, &home).unwrap();

            assert!(
                fs::read_to_string(home.join(".codex/skills/track/SKILL.md"))
                    .unwrap()
                    .contains("second")
            );
            assert!(fs::read_to_string(home.join(".claude/commands/track.md"))
                .unwrap()
                .contains("second"));
            fs::remove_dir_all(directory).unwrap();
        }

        #[test]
        fn updates_distributed_claude_command_and_recognizes_marker() {
            let directory = test_dir("distributed-command");
            let root = directory.join("resources/integrations");
            let home = directory.join("home");
            create_sources(
                &root,
                "---\nname: track\n---\nnew",
                include_str!("../../integrations/claude/track.md"),
            );
            fs::create_dir_all(home.join(".claude/commands")).unwrap();
            let command = home.join(".claude/commands/track.md");
            fs::write(
                &command,
                "---\ndescription: Track (個人の工数管理アプリ) に、現セッションの作業を記録する\n---\n# /track — 今のセッションをTrackに記録\n~/.track/runtime.json\nprepare --source claude",
            ).unwrap();
            install_from(&root, &home).unwrap();
            assert_eq!(fs::read_to_string(&command).unwrap(), include_str!("../../integrations/claude/track.md"));
            install_from(&root, &home).unwrap();
            fs::write(&command, "<!-- track:claude-command:v1 -->\n説明文を変更したTrack連携").unwrap();
            install_from(&root, &home).unwrap();
            fs::remove_dir_all(directory).unwrap();
        }

        #[test]
        fn migrates_existing_track_symlinks_without_deleting_targets() {
            let directory = test_dir("symlink");
            let root = directory.join("resources/integrations");
            let home = directory.join("home");
            let legacy = directory.join("legacy");
            create_sources(
                &root,
                "---\nname: track\n---\nnew",
                "# /track\n個人用工数管理アプリ new",
            );
            fs::create_dir_all(legacy.join("skill")).unwrap();
            fs::write(
                legacy.join("skill/SKILL.md"),
                "---\nname: track\n---\nlegacy",
            )
            .unwrap();
            fs::create_dir_all(home.join(".codex/skills")).unwrap();
            symlink(legacy.join("skill"), home.join(".codex/skills/track")).unwrap();
            fs::create_dir_all(home.join(".agents/skills")).unwrap();
            symlink(legacy.join("skill"), home.join(".agents/skills/track")).unwrap();
            fs::create_dir_all(home.join(".claude/commands")).unwrap();
            fs::write(
                legacy.join("track.md"),
                "この内容はインストーラーの判定対象にしない",
            )
            .unwrap();
            symlink(
                legacy.join("track.md"),
                home.join(".claude/commands/track.md"),
            )
            .unwrap();

            install_from(&root, &home).unwrap();

            assert!(!fs::symlink_metadata(home.join(".codex/skills/track"))
                .unwrap()
                .file_type()
                .is_symlink());
            assert!(legacy.join("skill/SKILL.md").is_file());
            assert!(legacy.join("track.md").is_file());
            assert!(!home.join(".agents/skills/track").exists());
            fs::remove_dir_all(directory).unwrap();
        }

        #[test]
        fn refuses_unrelated_files() {
            let directory = test_dir("collision");
            let root = directory.join("resources/integrations");
            let home = directory.join("home");
            create_sources(
                &root,
                "---\nname: track\n---\nnew",
                "# /track\n個人用工数管理アプリ new",
            );
            fs::create_dir_all(home.join(".codex/skills/track")).unwrap();
            fs::write(
                home.join(".codex/skills/track/SKILL.md"),
                "---\nname: unrelated\n---",
            )
            .unwrap();

            let error = install_from(&root, &home).unwrap_err();
            assert!(error.contains("同名のCodexスキル"));

            fs::remove_dir_all(home.join(".codex/skills/track")).unwrap();
            fs::create_dir_all(home.join(".claude/commands")).unwrap();
            fs::write(home.join(".claude/commands/track.md"), "unrelated command").unwrap();

            let error = install_from(&root, &home).unwrap_err();
            assert!(error.contains("同名のClaude Codeコマンド"));
            fs::remove_dir_all(directory).unwrap();
        }
    }
}

const HOMEBREW_TRACK_CASK: &str = "nemooon/tap/track";
const XATTR_EXECUTABLE: &str = "/usr/bin/xattr";

fn homebrew_executable() -> Option<PathBuf> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

fn command_error(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if detail.is_empty() {
        format!("終了コード: {}", output.status)
    } else {
        detail.to_string()
    }
}

fn parse_brew_cask_version(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output)
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
}

fn installed_track_version(brew: &std::path::Path) -> Result<Option<String>, String> {
    let output = Command::new(brew)
        .args(["list", "--cask", "--versions", "track"])
        .output()
        .map_err(|error| format!("Homebrewを実行できません: {error}"))?;
    Ok(output
        .status
        .success()
        .then(|| parse_brew_cask_version(&output.stdout))
        .flatten())
}

fn remove_quarantine(xattr: &std::path::Path, app_path: &std::path::Path) -> Result<(), String> {
    let output = Command::new(xattr)
        .args(["-dr", "com.apple.quarantine"])
        .arg(app_path)
        .output()
        .map_err(|error| {
            format!("アップデートは完了しましたが、隔離属性を削除できませんでした: {error}")
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "アップデートは完了しましたが、隔離属性を削除できませんでした。\n\n{}\n\nターミナルから次のコマンドを実行してください:\n\nxattr -dr com.apple.quarantine \"{}\"",
            command_error(&output),
            app_path.display()
        ))
    }
}

fn run_homebrew_update(
    brew: &std::path::Path,
    xattr: &std::path::Path,
    app_path: &std::path::Path,
    expected_version: &str,
) -> Result<(), String> {
    if installed_track_version(brew)?.is_none() {
        return Err(
            "TrackはHomebrewでインストールされていません。ターミナルからアップデートしてください。"
                .into(),
        );
    }

    let output = Command::new(brew)
        .args([
            "upgrade",
            "--cask",
            "--no-quit",
            "--no-ask",
            HOMEBREW_TRACK_CASK,
        ])
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .output()
        .map_err(|error| format!("Homebrewを実行できません: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "アップデートに失敗しました。\n\n{}",
            command_error(&output)
        ));
    }

    match installed_track_version(brew)? {
        Some(version) if version == expected_version => {}
        Some(version) => {
            return Err(format!(
                "Homebrewにはまだバージョン{expected_version}が反映されていません（現在: {version}）。少し待ってからもう一度お試しください。"
            ));
        }
        None => {
            return Err(
                "アップデート後のバージョンを確認できませんでした。ターミナルから状態を確認してください。"
                    .into(),
            );
        }
    }

    remove_quarantine(xattr, app_path)
}

fn current_app_bundle() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Trackのインストール先を確認できません: {error}"))?;
    executable
        .ancestors()
        .find(|path| path.extension().and_then(|extension| extension.to_str()) == Some("app"))
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| "Track.appのインストール先を確認できません。".to_string())
}

#[tauri::command]
async fn install_update(expected_version: String) -> Result<(), String> {
    let app_path = current_app_bundle()?;
    tauri::async_runtime::spawn_blocking(move || {
        let brew = homebrew_executable()
            .ok_or("Homebrewが見つかりません。ターミナルからアップデートしてください。")?;
        run_homebrew_update(
            &brew,
            std::path::Path::new(XATTR_EXECUTABLE),
            &app_path,
            &expected_version,
        )
    })
    .await
    .map_err(|error| format!("アップデート処理を完了できません: {error}"))?
}

#[tauri::command]
fn restart_track(app: tauri::AppHandle) {
    app.restart();
}

#[cfg(test)]
mod update_tests {
    use super::{parse_brew_cask_version, run_homebrew_update};
    #[cfg(unix)]
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn parses_homebrew_cask_version() {
        assert_eq!(
            parse_brew_cask_version(b"track 0.4.0\n"),
            Some("0.4.0".into())
        );
        assert_eq!(parse_brew_cask_version(b""), None);
    }

    #[cfg(unix)]
    fn fake_brew(name: &str, body: &str) -> (PathBuf, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "track-update-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let brew = directory.join("brew");
        fs::write(&brew, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&brew).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&brew, permissions).unwrap();
        (brew, directory)
    }

    #[cfg(unix)]
    fn fake_xattr(directory: &std::path::Path, body: &str) -> PathBuf {
        let xattr = directory.join("xattr");
        fs::write(&xattr, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&xattr).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&xattr, permissions).unwrap();
        xattr
    }

    #[cfg(unix)]
    #[test]
    fn runs_expected_homebrew_upgrade_and_verifies_version() {
        let (brew, directory) = fake_brew(
            "success",
            r#"
directory=$(dirname "$0")
if [ "$1" = "list" ]; then
  if [ -f "$directory/updated" ]; then
    echo "track 0.4.0"
  else
    echo "track 0.3.0"
  fi
elif [ "$1" = "upgrade" ]; then
  printf '%s\n' "$*" > "$directory/arguments"
  touch "$directory/updated"
else
  exit 1
fi
"#,
        );
        let app_path = directory.join("Track.app");
        let xattr = fake_xattr(
            &directory,
            r#"
directory=$(dirname "$0")
printf '%s\n' "$*" > "$directory/xattr-arguments"
"#,
        );

        run_homebrew_update(&brew, &xattr, &app_path, "0.4.0").unwrap();
        assert_eq!(
            fs::read_to_string(directory.join("arguments"))
                .unwrap()
                .trim(),
            "upgrade --cask --no-quit --no-ask nemooon/tap/track"
        );
        assert_eq!(
            fs::read_to_string(directory.join("xattr-arguments"))
                .unwrap()
                .trim(),
            format!("-dr com.apple.quarantine {}", app_path.to_string_lossy())
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn reports_when_homebrew_tap_has_not_caught_up() {
        let (brew, directory) = fake_brew(
            "tap-lag",
            r#"
if [ "$1" = "list" ]; then
  echo "track 0.3.0"
elif [ "$1" = "upgrade" ]; then
  exit 0
else
  exit 1
fi
"#,
        );

        let error = run_homebrew_update(
            &brew,
            &directory.join("xattr-must-not-run"),
            &directory.join("Track.app"),
            "0.4.0",
        )
        .unwrap_err();
        assert!(error.contains("まだバージョン0.4.0が反映されていません"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn reports_when_quarantine_removal_fails_after_update() {
        let (brew, directory) = fake_brew(
            "xattr-failure",
            r#"
directory=$(dirname "$0")
if [ "$1" = "list" ]; then
  if [ -f "$directory/updated" ]; then
    echo "track 0.4.0"
  else
    echo "track 0.3.0"
  fi
elif [ "$1" = "upgrade" ]; then
  touch "$directory/updated"
else
  exit 1
fi
"#,
        );
        let xattr = fake_xattr(
            &directory,
            r#"
echo "Operation not permitted" >&2
exit 1
"#,
        );
        let app_path = directory.join("Track.app");

        let error = run_homebrew_update(&brew, &xattr, &app_path, "0.4.0").unwrap_err();
        assert!(error.contains("アップデートは完了しました"));
        assert!(error.contains("Operation not permitted"));
        assert!(error.contains("xattr -dr com.apple.quarantine"));
        fs::remove_dir_all(directory).unwrap();
    }
}

#[tauri::command]
fn show_ai_integration_installer(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    prompt_install_ai_integration(&app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

const DEV_FRONTEND_URL: &str = "http://127.0.0.1:5173";
#[cfg(desktop)]
const SETTINGS_MENU_ID: &str = "open-settings";
#[cfg(desktop)]
const ABOUT_MENU_ID: &str = "open-about";
#[cfg(desktop)]
const CHECK_FOR_UPDATES_MENU_ID: &str = "check-for-updates";
#[cfg(desktop)]
const DATA_TRANSFER_MENU_ID: &str = "open-data-transfer";
#[cfg(target_os = "macos")]
const INSTALL_AI_INTEGRATION_MENU_ID: &str = "install-ai-integration";
#[cfg(desktop)]
const CALENDAR_MENU_ID: &str = "open-calendar";
#[cfg(desktop)]
const REPORTS_MENU_ID: &str = "open-reports";
#[cfg(desktop)]
const NOTES_MENU_ID: &str = "open-notes";
#[cfg(desktop)]
const PREVIOUS_PERIOD_MENU_ID: &str = "previous-period";
#[cfg(desktop)]
const NEXT_PERIOD_MENU_ID: &str = "next-period";
#[cfg(desktop)]
const TODAY_MENU_ID: &str = "go-to-today";
#[cfg(desktop)]
const ZOOM_IN_MENU_ID: &str = "calendar-zoom-in";
#[cfg(desktop)]
const ZOOM_OUT_MENU_ID: &str = "calendar-zoom-out";

#[derive(Default)]
struct SidecarState(Mutex<Option<Child>>);

struct StartedSidecar {
    child: Child,
    url: Url,
}

fn pipe_sidecar_output(reader: impl Read + Send + 'static, stderr: bool) {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if stderr {
                log::warn!(target: "track_server", "{line}");
            } else {
                log::info!(target: "track_server", "{line}");
            }
        }
    });
}

fn available_loopback_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn server_is_listening(port: u16, timeout: Duration) -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&address, timeout).is_ok()
}

fn wait_for_server(child: &mut Child, port: u16) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if server_is_listening(port, Duration::from_millis(100)) {
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Err(format!("Bun sidecarが起動中に終了しました: {status}"));
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("Bun sidecarの起動が15秒以内に完了しませんでした".into())
}

fn start_sidecar(app: &tauri::App) -> Result<StartedSidecar, Box<dyn std::error::Error>> {
    let port = available_loopback_port()?;
    let executable_dir = std::env::current_exe()?
        .parent()
        .ok_or("Track実行ファイルのディレクトリを取得できません")?
        .to_path_buf();
    let sidecar_name = if cfg!(windows) {
        "track-server.exe"
    } else {
        "track-server"
    };
    let sidecar_path = executable_dir.join(sidecar_name);
    let cli_name = if cfg!(windows) {
        "track-cli.exe"
    } else {
        "track-cli"
    };
    let cli_path = executable_dir.join(cli_name);
    if !cli_path.is_file() {
        return Err(format!("同梱CLIが見つかりません: {}", cli_path.display()).into());
    }
    let resource_dir = app.path().resource_dir()?;

    log::info!(
        "Bun sidecarを起動: {} (port: {port})",
        sidecar_path.display()
    );
    let mut child = Command::new(&sidecar_path)
        .env("TRACK_RESOURCE_DIR", &resource_dir)
        .env("TRACK_PORT", port.to_string())
        .env("TRACK_CLI_PATH", &cli_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(stdout) = child.stdout.take() {
        pipe_sidecar_output(stdout, false);
    }
    if let Some(stderr) = child.stderr.take() {
        pipe_sidecar_output(stderr, true);
    }

    if let Err(error) = wait_for_server(&mut child, port) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }

    let url = Url::parse(&format!("http://127.0.0.1:{port}"))?;
    log::info!("Bun sidecarの起動を確認: {url}");
    Ok(StartedSidecar { child, url })
}

fn show_main_window(app: &tauri::App, server_url: &Url) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or("mainウィンドウが見つかりません")?;
    window.navigate(server_url.clone())?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

fn focus_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

#[cfg(desktop)]
fn open_settings_overlay(app: &tauri::AppHandle) {
    focus_main_window(app);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.emit("track-open-settings", ()) {
        log::error!("設定オーバーレイを開けません: {error}");
    }
}

#[cfg(desktop)]
fn open_about_dialog(app: &tauri::AppHandle) {
    focus_main_window(app);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.emit("track-open-about", ()) {
        log::error!("Trackについてを開けません: {error}");
    }
}

#[cfg(desktop)]
fn open_data_transfer_dialog(app: &tauri::AppHandle) {
    focus_main_window(app);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.emit("track-open-data-transfer", ()) {
        log::error!("データ移行ダイアログを開けません: {error}");
    }
}

#[cfg(target_os = "macos")]
fn prompt_install_ai_integration(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    app.dialog()
        .message(
            "TrackのAI連携をインストールします。\n\nCodexスキル:\n~/.codex/skills/track\n\nClaude Codeコマンド:\n~/.claude/commands/track.md\n\n既存のTrack連携は最新版へ更新します。",
        )
        .title("AI連携をインストール")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "インストール".into(),
            "キャンセル".into(),
        ))
        .show(move |confirmed| {
            if !confirmed {
                return;
            }
            let (message, kind) = match ai_integration::install(&app_handle) {
                Ok(result) => (
                    format!(
                        "AI連携をインストールしました。\n\nCodex:\n{}\n\nClaude Code:\n{}\n\n新しいセッションから$trackまたは/trackを利用できます。",
                        result.codex_path.display(),
                        result.claude_path.display(),
                    ),
                    MessageDialogKind::Info,
                ),
                Err(error) => (
                    format!("AI連携をインストールできませんでした。\n\n{error}"),
                    MessageDialogKind::Error,
                ),
            };
            app_handle
                .dialog()
                .message(message)
                .title("Track AI連携")
                .kind(kind)
                .show(|_| {});
        });
}

#[cfg(desktop)]
fn open_app_view(app: &tauri::AppHandle, path: &str) {
    focus_main_window(app);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.emit("track-open-view", path) {
        log::error!("画面を切り替えられません: {error}");
    }
}

#[cfg(desktop)]
fn navigate_date(app: &tauri::AppHandle, action: &str) {
    focus_main_window(app);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.emit("track-date-navigation", action) {
        log::error!("表示期間を移動できません: {error}");
    }
}

#[cfg(desktop)]
fn zoom_calendar(app: &tauri::AppHandle, direction: &str) {
    focus_main_window(app);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.emit("track-calendar-zoom", direction) {
        log::error!("カレンダーを拡大縮小できません: {error}");
    }
}

fn stop_sidecar(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<SidecarState>() else {
        return;
    };
    let Some(mut child) = state.0.lock().expect("sidecar state lock").take() else {
        return;
    };

    log::info!("Bun sidecarを終了");
    let _ = child.kill();
    let _ = child.wait();
}

fn setup_app(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(SidecarState::default());

    let server_url = if cfg!(debug_assertions) {
        Url::parse(DEV_FRONTEND_URL)?
    } else {
        let StartedSidecar { child, url } = start_sidecar(app)?;
        app.state::<SidecarState>()
            .0
            .lock()
            .expect("sidecar state lock")
            .replace(child);
        url
    };

    show_main_window(app, &server_url)
}

fn show_startup_error(app: &tauri::App, error: &dyn std::error::Error) {
    log::error!("Trackの起動に失敗: {error}");
    app.dialog()
        .message(format!(
            "Trackを起動できませんでした。\n\n{error}\n\nアプリを終了して、もう一度お試しください。"
        ))
        .title("Trackの起動に失敗しました")
        .kind(MessageDialogKind::Error)
        .blocking_show();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                focus_main_window(app);
            }))
            .menu(|app| {
                #[cfg(target_os = "macos")]
                {
                    use tauri::menu::{
                        MenuBuilder, MenuItem, SubmenuBuilder, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID,
                    };

                    let app_name = app
                        .config()
                        .product_name
                        .clone()
                        .unwrap_or_else(|| app.package_info().name.clone());
                    let about = MenuItem::with_id(
                        app,
                        ABOUT_MENU_ID,
                        format!("{app_name}について"),
                        true,
                        None::<&str>,
                    )?;
                    let check_for_updates = MenuItem::with_id(
                        app,
                        CHECK_FOR_UPDATES_MENU_ID,
                        "アップデートを確認…",
                        true,
                        None::<&str>,
                    )?;
                    let settings = MenuItem::with_id(
                        app,
                        SETTINGS_MENU_ID,
                        "設定…",
                        true,
                        Some("CmdOrCtrl+,"),
                    )?;
                    let data_transfer = MenuItem::with_id(
                        app,
                        DATA_TRANSFER_MENU_ID,
                        "データの移行…",
                        true,
                        None::<&str>,
                    )?;
                    let install_ai_integration = MenuItem::with_id(
                        app,
                        INSTALL_AI_INTEGRATION_MENU_ID,
                        "AI連携をインストール…",
                        true,
                        None::<&str>,
                    )?;
                    let calendar = MenuItem::with_id(
                        app,
                        CALENDAR_MENU_ID,
                        "カレンダー",
                        true,
                        Some("CmdOrCtrl+1"),
                    )?;
                    let reports = MenuItem::with_id(
                        app,
                        REPORTS_MENU_ID,
                        "レポート",
                        true,
                        Some("CmdOrCtrl+2"),
                    )?;
                    let notes = MenuItem::with_id(
                        app,
                        NOTES_MENU_ID,
                        "メモ",
                        true,
                        Some("CmdOrCtrl+3"),
                    )?;
                    let previous_period = MenuItem::with_id(
                        app,
                        PREVIOUS_PERIOD_MENU_ID,
                        "前の期間",
                        true,
                        Some("CmdOrCtrl+["),
                    )?;
                    let next_period = MenuItem::with_id(
                        app,
                        NEXT_PERIOD_MENU_ID,
                        "次の期間",
                        true,
                        Some("CmdOrCtrl+]"),
                    )?;
                    let today =
                        MenuItem::with_id(app, TODAY_MENU_ID, "今日", true, Some("CmdOrCtrl+T"))?;
                    let zoom_in = MenuItem::with_id(
                        app,
                        ZOOM_IN_MENU_ID,
                        "カレンダーを拡大",
                        true,
                        Some("CmdOrCtrl++"),
                    )?;
                    let zoom_out = MenuItem::with_id(
                        app,
                        ZOOM_OUT_MENU_ID,
                        "カレンダーを縮小",
                        true,
                        Some("CmdOrCtrl+-"),
                    )?;

                    let app_menu = SubmenuBuilder::new(app, &app_name)
                        .item(&about)
                        .item(&check_for_updates)
                        .separator()
                        .item(&settings)
                        .item(&install_ai_integration)
                        .separator()
                        .services_with_text("サービス")
                        .separator()
                        .hide_with_text(format!("{app_name}を隠す"))
                        .hide_others_with_text("ほかを隠す")
                        .show_all_with_text("すべてを表示")
                        .separator()
                        .quit_with_text(format!("{app_name}を終了"))
                        .build()?;
                    let file_menu = SubmenuBuilder::new(app, "ファイル")
                        .item(&data_transfer)
                        .separator()
                        .close_window_with_text("ウインドウを閉じる")
                        .build()?;
                    let edit_menu = SubmenuBuilder::new(app, "編集")
                        .undo_with_text("取り消す")
                        .redo_with_text("やり直す")
                        .separator()
                        .cut_with_text("カット")
                        .copy_with_text("コピー")
                        .paste_with_text("ペースト")
                        .select_all_with_text("すべてを選択")
                        .build()?;
                    let view_menu = SubmenuBuilder::new(app, "表示")
                        .item(&calendar)
                        .item(&reports)
                        .item(&notes)
                        .separator()
                        .item(&previous_period)
                        .item(&next_period)
                        .item(&today)
                        .separator()
                        .item(&zoom_in)
                        .item(&zoom_out)
                        .separator()
                        .fullscreen_with_text("フルスクリーンにする")
                        .build()?;
                    let window_menu = SubmenuBuilder::with_id(app, WINDOW_SUBMENU_ID, "ウインドウ")
                        .minimize_with_text("しまう")
                        .maximize_with_text("ズーム")
                        .separator()
                        .close_window_with_text("ウインドウを閉じる")
                        .separator()
                        .bring_all_to_front_with_text("すべてを手前に移動")
                        .build()?;
                    let help_menu =
                        SubmenuBuilder::with_id(app, HELP_SUBMENU_ID, "ヘルプ").build()?;

                    MenuBuilder::new(app)
                        .items(&[
                            &app_menu,
                            &file_menu,
                            &edit_menu,
                            &view_menu,
                            &window_menu,
                            &help_menu,
                        ])
                        .build()
                }

                #[cfg(not(target_os = "macos"))]
                {
                    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

                    let menu = Menu::default(app)?;
                    let settings = MenuItem::with_id(
                        app,
                        SETTINGS_MENU_ID,
                        "設定…",
                        true,
                        Some("CmdOrCtrl+,"),
                    )?;
                    let data_transfer = MenuItem::with_id(
                        app,
                        DATA_TRANSFER_MENU_ID,
                        "データの移行…",
                        true,
                        None::<&str>,
                    )?;
                    let separator = PredefinedMenuItem::separator(app)?;
                    let items = menu.items()?;
                    if let Some(app_menu) = items.first().and_then(|item| item.as_submenu()) {
                        app_menu.insert_items(&[&settings, &data_transfer, &separator], 2)?;
                    }
                    Ok(menu)
                }
            })
            .on_menu_event(|app, event| match event.id().as_ref() {
                ABOUT_MENU_ID => open_about_dialog(app),
                CHECK_FOR_UPDATES_MENU_ID => {
                    focus_main_window(app);
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(error) = window.emit("track-check-for-updates", ()) {
                            log::error!("アップデート確認を開始できません: {error}");
                        }
                    }
                }
                #[cfg(target_os = "macos")]
                INSTALL_AI_INTEGRATION_MENU_ID => prompt_install_ai_integration(app),
                SETTINGS_MENU_ID => open_settings_overlay(app),
                DATA_TRANSFER_MENU_ID => open_data_transfer_dialog(app),
                CALENDAR_MENU_ID => open_app_view(app, "/calendar"),
                REPORTS_MENU_ID => open_app_view(app, "/reports"),
                NOTES_MENU_ID => open_app_view(app, "/notes"),
                PREVIOUS_PERIOD_MENU_ID => navigate_date(app, "previous"),
                NEXT_PERIOD_MENU_ID => navigate_date(app, "next"),
                TODAY_MENU_ID => navigate_date(app, "today"),
                ZOOM_IN_MENU_ID => zoom_calendar(app, "in"),
                ZOOM_OUT_MENU_ID => zoom_calendar(app, "out"),
                _ => {}
            });
    }

    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            install_update,
            restart_track,
            show_ai_integration_installer
        ])
        .setup(|app| {
            if let Err(error) = setup_app(app) {
                show_startup_error(app, error.as_ref());
                stop_sidecar(app.handle());
                app.handle().exit(1);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
            stop_sidecar(app_handle);
        }
    });
}
