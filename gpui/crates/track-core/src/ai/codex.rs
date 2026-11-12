//! `CodexProvider`: runs `codex exec --experimental-json` like @openai/codex-sdk and reads its JSONL events.
use super::process::{TempDir, resolve_executable};
use super::{AiMode, AiProgress, AiProvider, ai_error, build_prompt, clean_response};
use crate::CoreError;
use crate::model::AppConfig;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const TIMEOUT: Duration = Duration::from_secs(10 * 60);

const AGGREGATION_SCHEMA: &str = r#"{"type":"object","properties":{"groups":{"type":"array","items":{"type":"object","properties":{"label":{"type":"string"},"summary":{"type":"string"},"entryIds":{"type":"array","items":{"type":"string"}},"values":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"value":{"type":"string"}},"required":["id","value"],"additionalProperties":false}}},"required":["label","summary","entryIds","values"],"additionalProperties":false}}},"required":["groups"],"additionalProperties":false}"#;

pub struct Codex {
    executable: Option<PathBuf>,
    model: Option<String>,
}

impl Codex {
    pub fn new(executable: Option<PathBuf>, model: Option<String>) -> Self {
        Codex { executable, model }
    }

    pub fn from_config(config: &AppConfig, home: &Path) -> Self {
        let exe = if config.ai_codex_executable.trim().is_empty() { "codex" } else { &config.ai_codex_executable };
        Codex { executable: resolve_executable(exe, home), model: Some(config.ai_codex_model.clone()).filter(|m| !m.trim().is_empty()) }
    }
}

/// The argument list the SDK builds for these thread options.
pub fn exec_args(model: Option<&str>, mode: AiMode, workdir: &Path, schema: Option<&Path>) -> Vec<String> {
    let mut a: Vec<String> = vec!["exec".into(), "--experimental-json".into()];
    if let Some(m) = model {
        a.extend(["--model".into(), m.into()]);
    }
    a.extend(["--sandbox".into(), "read-only".into(), "--cd".into(), workdir.to_string_lossy().into_owned(), "--skip-git-repo-check".into()]);
    if let Some(s) = schema {
        a.extend(["--output-schema".into(), s.to_string_lossy().into_owned()]);
    }
    if mode == AiMode::ReportAggregation {
        a.extend(["--config".into(), "model_reasoning_effort=\"low\"".into()]);
    }
    for c in ["sandbox_workspace_write.network_access=false", "web_search=\"disabled\"", "approval_policy=\"never\""] {
        a.extend(["--config".into(), c.into()]);
    }
    a
}

/// Folds JSONL events into progress callbacks and the final agent message.
#[derive(Default)]
pub struct Fold {
    pub final_response: String,
}

impl Fold {
    /// Err carries the failure message from `turn.failed` / `error`.
    pub fn push(&mut self, line: &str, on_progress: &mut dyn FnMut(AiProgress)) -> Result<(), String> {
        let Ok(ev) = serde_json::from_str::<Value>(line) else { return Ok(()) };
        let item = &ev["item"];
        match ev["type"].as_str().unwrap_or("") {
            "turn.started" => on_progress(AiProgress::status("入力内容を確認しています…")),
            "item.updated" | "item.completed" if item["type"] == "reasoning" => {
                let text = item["text"].as_str().unwrap_or("").trim();
                if !text.is_empty() {
                    on_progress(AiProgress { kind: "reasoning", message: text.into() });
                }
            }
            "item.completed" if item["type"] == "agent_message" => self.final_response = item["text"].as_str().unwrap_or("").into(),
            "turn.failed" => return Err(ev["error"]["message"].as_str().unwrap_or("turn failed").into()),
            "error" => return Err(ev["message"].as_str().unwrap_or("error").into()),
            _ => {}
        }
        Ok(())
    }
}

pub fn friendly_error(message: &str) -> String {
    let lower = message.to_lowercase();
    if lower.contains("not logged in") || lower.contains("login") || lower.contains("unauthorized") || lower.contains("401") {
        "Codexへログインしていません。ターミナルで「codex login」を実行してください。".into()
    } else {
        format!("Codexで生成できませんでした: {message}")
    }
}

enum RunError {
    Timeout,
    Failed(String),
}

impl From<String> for RunError {
    fn from(s: String) -> Self {
        RunError::Failed(s)
    }
}

enum Msg {
    Line(String),
    Eof,
}

impl Codex {
    fn run(&self, exe: &Path, mode: AiMode, input: &str, on_progress: &mut dyn FnMut(AiProgress), timeout: Duration) -> Result<String, RunError> {
        let io = |e: std::io::Error| RunError::Failed(e.to_string());
        let dir = TempDir::new("track-codex-").map_err(io)?;
        let schema = if mode == AiMode::ReportAggregation {
            let p = dir.0.join("output-schema.json");
            std::fs::write(&p, AGGREGATION_SCHEMA).map_err(io)?;
            Some(p)
        } else {
            None
        };
        let workdir = dir.0.join("work");
        std::fs::create_dir(&workdir).map_err(io)?;
        let mut child = Command::new(exe)
            .args(exec_args(self.model.as_deref(), mode, &workdir, schema.as_deref()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(io)?;
        on_progress(AiProgress::status("Codexに処理を依頼しました…"));
        let mut stdin = child.stdin.take().unwrap();
        let prompt = build_prompt(mode, input).into_bytes();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&prompt);
        });
        let mut stderr = child.stderr.take().unwrap();
        let err_t = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            s
        });
        let (tx, rx) = mpsc::channel();
        let stdout = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(Msg::Line(line)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Msg::Eof);
        });
        let deadline = Instant::now() + timeout;
        let mut fold = Fold::default();
        let kill = |child: &mut std::process::Child| {
            let _ = child.kill();
            let _ = child.wait();
        };
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Msg::Line(line)) => {
                    if let Err(e) = fold.push(&line, on_progress) {
                        kill(&mut child);
                        return Err(e.into());
                    }
                }
                Ok(Msg::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    kill(&mut child);
                    return Err(RunError::Timeout);
                }
            }
        }
        let status = child.wait().map_err(io)?;
        let stderr = err_t.join().unwrap_or_default();
        if !status.success() {
            let detail = stderr.trim();
            return Err(RunError::Failed(if detail.is_empty() { format!("Codex Exec exited with {status}") } else { detail.to_string() }));
        }
        Ok(fold.final_response)
    }
}

impl AiProvider for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn generate(&self, mode: AiMode, input: &str, on_progress: &mut dyn FnMut(AiProgress)) -> Result<String, CoreError> {
        let Some(exe) = &self.executable else {
            return Err(ai_error("codex実行ファイルが見つかりません。Codexへログイン後、設定で実行ファイルを指定してください。"));
        };
        match self.run(exe, mode, input, on_progress, TIMEOUT) {
            Ok(text) => clean_response(&text),
            Err(RunError::Timeout) => Err(ai_error(format!(
                "Codexの生成が{}分以内に完了しませんでした。対象期間や絞り込みを狭めて再度お試しください。",
                TIMEOUT.as_secs() / 60
            ))),
            Err(RunError::Failed(e)) => Err(ai_error(friendly_error(&e))),
        }
    }
}
