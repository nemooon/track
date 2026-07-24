use std::{
    io::{BufRead, BufReader, Read},
    net::{SocketAddr, TcpListener, TcpStream},
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
mod weekly_report {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        os::unix::fs::OpenOptionsExt,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tauri::Manager;

    fn helper_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
        let resource = app
            .path()
            .resource_dir()
            .map_err(|error| format!("アプリのリソースを取得できません: {error}"))?
            .join("TrackAIHelper.app");
        if resource.exists() {
            return Ok(resource);
        }

        let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("TrackAIHelper.app");
        if development.exists() {
            return Ok(development);
        }
        Err("Apple Intelligenceヘルパーが見つかりません。アプリを再ビルドしてください。".into())
    }

    fn write_private(path: &Path, content: &[u8]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| format!("週報生成の一時ファイルを作成できません: {error}"))?;
        file.write_all(content)
            .map_err(|error| format!("週報生成の一時ファイルへ書き込めません: {error}"))
    }

    fn macos_major_version() -> Result<u32, String> {
        let output = Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .map_err(|error| format!("macOSのバージョンを確認できません: {error}"))?;
        let version = String::from_utf8_lossy(&output.stdout);
        version
            .trim()
            .split('.')
            .next()
            .and_then(|part| part.parse().ok())
            .ok_or_else(|| format!("macOSのバージョンを判定できません: {version}"))
    }

    pub fn generate(app: &tauri::AppHandle, prompt: &str) -> Result<String, String> {
        if prompt.trim().is_empty() {
            return Err("週報生成の入力が空です。".into());
        }
        if prompt.len() > 200_000 {
            return Err("週報生成の入力が大きすぎます。絞り込みを指定してください。".into());
        }
        if macos_major_version()? < 26 {
            return Err("週報のAI生成にはmacOS 26以降が必要です。".into());
        }

        let helper = helper_path(app)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "track-weekly-report-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&temp_dir)
            .map_err(|error| format!("週報生成の一時フォルダを作成できません: {error}"))?;

        let result = (|| {
            let input = temp_dir.join("prompt.txt");
            let stdout = temp_dir.join("response.txt");
            let stderr = temp_dir.join("error.txt");
            write_private(&input, prompt.as_bytes())?;
            write_private(&stdout, b"")?;
            write_private(&stderr, b"")?;

            let status = Command::new("/usr/bin/open")
                .arg("-n")
                .arg("-W")
                .arg("--stdout")
                .arg(&stdout)
                .arg("--stderr")
                .arg(&stderr)
                .arg(&helper)
                .arg("--args")
                .arg(&input)
                .status()
                .map_err(|error| format!("Apple Intelligenceを起動できません: {error}"))?;

            let response = fs::read_to_string(&stdout)
                .map_err(|error| format!("生成した週報を読み込めません: {error}"))?;
            let helper_error = fs::read_to_string(&stderr).unwrap_or_default();
            if !status.success() || response.trim().is_empty() {
                let detail = helper_error.trim();
                return Err(if detail.is_empty() {
                    "Apple Intelligenceから応答がありませんでした。".into()
                } else {
                    detail.into()
                });
            }
            Ok(response.trim().to_string())
        })();

        if let Err(error) = fs::remove_dir_all(&temp_dir) {
            log::warn!("週報生成の一時ファイルを削除できません: {error}");
        }
        result
    }
}

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
            .map(|content| content.contains("# /track") && content.contains("個人用工数管理アプリ"))
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

#[tauri::command]
async fn generate_weekly_report(app: tauri::AppHandle, prompt: String) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            weekly_report::generate(&app, &prompt)
        })
        .await
        .map_err(|error| format!("週報生成処理を完了できません: {error}"))?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, prompt);
        Err("週報のAI生成はmacOSでのみ利用できます。".into())
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
#[cfg(target_os = "macos")]
const INSTALL_AI_INTEGRATION_MENU_ID: &str = "install-ai-integration";
#[cfg(desktop)]
const CALENDAR_MENU_ID: &str = "open-calendar";
#[cfg(desktop)]
const REPORTS_MENU_ID: &str = "open-reports";
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
                    let settings = MenuItem::with_id(
                        app,
                        SETTINGS_MENU_ID,
                        "設定…",
                        true,
                        Some("CmdOrCtrl+,"),
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
                    let separator = PredefinedMenuItem::separator(app)?;
                    let items = menu.items()?;
                    if let Some(app_menu) = items.first().and_then(|item| item.as_submenu()) {
                        app_menu.insert_items(&[&settings, &separator], 2)?;
                    }
                    Ok(menu)
                }
            })
            .on_menu_event(|app, event| match event.id().as_ref() {
                ABOUT_MENU_ID => open_about_dialog(app),
                #[cfg(target_os = "macos")]
                INSTALL_AI_INTEGRATION_MENU_ID => prompt_install_ai_integration(app),
                SETTINGS_MENU_ID => open_settings_overlay(app),
                CALENDAR_MENU_ID => open_app_view(app, "/calendar"),
                REPORTS_MENU_ID => open_app_view(app, "/reports"),
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
            generate_weekly_report,
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
