use std::{
    io::{BufRead, BufReader, Read},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tauri::{Manager, RunEvent, Url};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const DEV_SERVER_URL: &str = "http://127.0.0.1:8787";

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
        Url::parse(DEV_SERVER_URL)?
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
    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }));
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
