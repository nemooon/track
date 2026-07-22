use std::{
    io::{BufRead, BufReader, Read},
    net::{SocketAddr, TcpStream},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tauri::{Manager, RunEvent};

const SERVER_URL: &str = "http://127.0.0.1:8787";
const SERVER_ADDR: &str = "127.0.0.1:8787";

#[derive(Default)]
struct SidecarState(Mutex<Option<Child>>);

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

fn server_is_listening(timeout: Duration) -> bool {
    let address: SocketAddr = SERVER_ADDR.parse().expect("valid Track server address");
    TcpStream::connect_timeout(&address, timeout).is_ok()
}

fn wait_for_server(child: &mut Child) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if server_is_listening(Duration::from_millis(100)) {
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Err(format!("Bun sidecarが起動中に終了しました: {status}"));
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("Bun sidecarの起動が15秒以内に完了しませんでした".into())
}

fn start_sidecar(app: &tauri::App) -> Result<Child, Box<dyn std::error::Error>> {
    if server_is_listening(Duration::from_millis(200)) {
        return Err(format!("{SERVER_ADDR} は既に別のプロセスが使用しています").into());
    }

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

    log::info!("Bun sidecarを起動: {}", sidecar_path.display());
    let mut child = Command::new(&sidecar_path)
        .env("TRACK_RESOURCE_DIR", &resource_dir)
        .env("TRACK_PORT", "8787")
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

    if let Err(error) = wait_for_server(&mut child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }

    log::info!("Bun sidecarの起動を確認");
    Ok(child)
}

fn show_main_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or("mainウィンドウが見つかりません")?;
    window.navigate(tauri::Url::parse(SERVER_URL)?)?;
    window.show()?;
    window.set_focus()?;
    Ok(())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            app.manage(SidecarState::default());

            if !cfg!(debug_assertions) {
                let child = start_sidecar(app)?;
                app.state::<SidecarState>()
                    .0
                    .lock()
                    .expect("sidecar state lock")
                    .replace(child);
            }

            show_main_window(app)?;
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
