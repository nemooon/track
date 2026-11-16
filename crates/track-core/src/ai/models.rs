//! Codex model suggestions for the settings screen (src/server/ai/models.ts): ask the CLI's
//! `app-server` over JSON-RPC, else read Codex's local cache, else a built-in list.
use super::process::resolve_executable;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_OUTPUT: usize = 2 * 1024 * 1024;
const FALLBACK: [(&str, &str); 4] = [("gpt-6.1-sol", "GPT-6.1 Sol"), ("gpt-6-astra", "GPT-6 Astra"), ("gpt-6-sol", "GPT-6 Sol"), ("gpt-6-luna", "GPT-6 Luna")];

#[derive(Debug, Clone, PartialEq)]
pub struct CodexModelCatalog {
    /// (id, label)
    pub models: Vec<(String, String)>,
    /// "codex", "cache" or "fallback"
    pub source: &'static str,
    pub detail: String,
}

/// `loadCodexModels`. `codex_home` is `$CODEX_HOME` or `~/.codex`.
pub fn codex_models(requested_executable: &str, home: &Path, codex_home: &Path) -> CodexModelCatalog {
    let requested = if requested_executable.trim().is_empty() { "codex" } else { requested_executable };
    if let Some(exe) = resolve_executable(requested, home)
        && let Ok(models) = discover(&exe, TIMEOUT)
    {
        return CodexModelCatalog { models, source: "codex", detail: "Codexから取得したモデル候補です。表示中は5分ごとに自動更新します。".into() };
    }
    if let Some(models) = from_cache(codex_home) {
        return CodexModelCatalog {
            models,
            source: "cache",
            detail: "Codexへ接続できないため、前回のモデル候補を表示しています。最新の一覧とは異なる場合があります。".into(),
        };
    }
    CodexModelCatalog {
        models: FALLBACK.iter().map(|(i, l)| (i.to_string(), l.to_string())).collect(),
        source: "fallback",
        detail: "モデル一覧を取得できないため、2026年10月5日時点の標準候補を表示しています。Codexの実行ファイルとログイン状態を確認してください。".into(),
    }
}

/// `$CODEX_HOME` or `~/.codex`.
pub fn default_codex_home(home: &Path) -> std::path::PathBuf {
    std::env::var_os("CODEX_HOME").map(Into::into).unwrap_or_else(|| home.join(".codex"))
}

fn from_cache(codex_home: &Path) -> Option<Vec<(String, String)>> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(codex_home.join("models_cache.json")).ok()?).ok()?;
    let models: Vec<(String, String)> = v["models"]
        .as_array()?
        .iter()
        .filter(|m| m["visibility"] == "list")
        .filter_map(|m| Some((m["slug"].as_str().filter(|s| !s.is_empty())?.to_string(), m["display_name"].as_str().filter(|s| !s.is_empty())?.to_string())))
        .collect();
    (!models.is_empty()).then_some(models)
}

/// Reads the authenticated client's picker without starting a thread or turn.
fn discover(exe: &Path, timeout: Duration) -> Result<Vec<(String, String)>, String> {
    let mut child = Command::new(exe)
        .arg("app-server")
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let result = talk(&mut child, timeout);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn talk(child: &mut std::process::Child, timeout: Duration) -> Result<Vec<(String, String)>, String> {
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        let mut total = 0;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            total += line.len() + 1;
            if total > MAX_OUTPUT || tx.send(Some(line)).is_err() {
                break;
            }
        }
        let _ = tx.send(None);
    });
    let mut send = |v: Value| writeln!(stdin, "{v}").map_err(|e| e.to_string());
    send(json!({ "id": 0, "method": "initialize", "params": { "clientInfo": { "name": "track", "title": "Track", "version": "1.0.0" } } }))?;
    let deadline = Instant::now() + timeout;
    let mut request_id = 1;
    let mut cursors = std::collections::HashSet::new();
    let mut models: Vec<(String, String)> = Vec::new();
    loop {
        let line = match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Some(l)) => l,
            Ok(None) => return Err("Codexがモデル一覧を返さずに終了しました。".into()),
            Err(_) => return Err("モデル一覧の取得がタイムアウトしました。".into()),
        };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let id = msg["id"].as_i64();
        if id != Some(0) && id != Some(request_id) {
            continue;
        }
        if !msg["error"].is_null() {
            return Err("Codexがモデル一覧の取得を拒否しました。".into());
        }
        let list = |cursor: Option<&str>, id: i64| {
            let mut params = json!({ "limit": 100, "includeHidden": false });
            if let Some(c) = cursor {
                params["cursor"] = c.into();
            }
            json!({ "id": id, "method": "model/list", "params": params })
        };
        if id == Some(0) {
            send(json!({ "method": "initialized", "params": {} }))?;
            send(list(None, request_id))?;
            continue;
        }
        let data = msg["result"]["data"].as_array().ok_or("モデル一覧の形式が不正です。")?;
        for m in data {
            let (Some(model), Some(name)) = (m["model"].as_str(), m["displayName"].as_str()) else {
                return Err("モデル一覧の形式が不正です。".into());
            };
            if m["hidden"] != true && !models.iter().any(|(i, _)| i == model) {
                models.push((model.into(), name.into()));
            }
        }
        match msg["result"]["nextCursor"].as_str() {
            Some(c) => {
                if !cursors.insert(c.to_string()) || cursors.len() > 20 {
                    return Err("モデル一覧のページを取得できませんでした。".into());
                }
                request_id += 1;
                send(list(Some(c), request_id))?;
            }
            None if models.is_empty() => return Err("利用できるモデルがありませんでした。".into()),
            None => return Ok(models),
        }
    }
}
