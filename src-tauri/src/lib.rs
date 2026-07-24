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
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const DEV_FRONTEND_URL: &str = "http://127.0.0.1:5173";
#[cfg(desktop)]
const SETTINGS_MENU_ID: &str = "open-settings";
#[cfg(desktop)]
const ABOUT_MENU_ID: &str = "open-about";
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
    let resource_dir = app.path().resource_dir()?;

    log::info!(
        "Bun sidecarを起動: {} (port: {port})",
        sidecar_path.display()
    );
    let mut child = Command::new(&sidecar_path)
        .env("TRACK_RESOURCE_DIR", &resource_dir)
        .env("TRACK_PORT", port.to_string())
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
