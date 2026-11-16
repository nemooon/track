//! Child-process helpers shared by the providers (src/server/ai/process.ts).
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MAX_STDOUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 1024 * 1024;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3 * 60);

pub fn is_executable(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn expand_home(value: &str, home: &Path) -> PathBuf {
    if value == "~" {
        home.to_path_buf()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(value)
    }
}

pub fn search_directories(home: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var("PATH").unwrap_or_default().split(':').filter(|s| !s.is_empty()).map(PathBuf::from).collect();
    for d in [home.join(".local/bin"), home.join(".bun/bin"), home.join(".npm-global/bin")] {
        dirs.push(d);
    }
    for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        dirs.push(PathBuf::from(d));
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

fn codex_version(candidate: &Path) -> Option<[u64; 3]> {
    let out = run_process(candidate, &["--version".to_string()], "", Duration::from_secs(1)).ok()?;
    let rest = out.stdout.split("codex-cli").nth(1)?.trim_start();
    let mut parts = rest.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse::<u64>().ok());
    Some([parts.next()??, parts.next()??, parts.next()??])
}

/// Like `newestCodexExecutable`: the first executable wins unless a later one reports a newer version.
pub fn newest_codex(candidates: &[PathBuf]) -> Option<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut selected: Option<(PathBuf, [u64; 3])> = None;
    for c in candidates {
        if !is_executable(c) || !seen.insert(std::fs::canonicalize(c).unwrap_or(c.clone())) {
            continue;
        }
        let v = codex_version(c).unwrap_or([0, 0, 0]);
        match &selected {
            Some((_, best)) if v <= *best => {}
            _ => selected = Some((c.clone(), v)),
        }
    }
    selected.map(|(p, _)| p)
}

/// Like `resolveExecutable`: absolute paths must be executable, bare names are searched on PATH and
/// the usual install locations, and `codex` prefers the newest CLI including the desktop apps' copies.
pub fn resolve_executable(requested: &str, home: &Path) -> Option<PathBuf> {
    let value = requested.trim();
    if value.is_empty() {
        return None;
    }
    let path = expand_home(value, home);
    if path.is_absolute() {
        return is_executable(&path).then_some(path);
    }
    if value.contains('/') || value.contains('\\') {
        return None;
    }
    let dirs = search_directories(home);
    if value == "codex" {
        let mut installed: Vec<PathBuf> = dirs
            .iter()
            .filter(|d| !d.components().any(|c| c.as_os_str() == "node_modules"))
            .map(|d| d.join(value))
            .find(|p| is_executable(p))
            .into_iter()
            .collect();
        for p in [
            "/Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex",
            "/Applications/Codex.app/Contents/Resources/codex-cli/bin/codex",
            "/Applications/ChatGPT.app/Contents/Resources/codex",
            "/Applications/Codex.app/Contents/Resources/codex",
        ] {
            installed.push(PathBuf::from(p));
        }
        return newest_codex(&installed);
    }
    dirs.iter().map(|d| d.join(value)).find(|p| is_executable(p))
}

pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
}

/// Runs `exe` with `input` on stdin. Errors carry the user-facing message (stderr when the command fails).
pub fn run_process(exe: &Path, args: &[String], input: &str, timeout: Duration) -> Result<ProcessOutput, String> {
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("コマンドを起動できません: {e}"))?;
    let mut stdin = child.stdin.take().unwrap();
    let input = input.as_bytes().to_vec();
    // EPIPE (the command does not read stdin) is not an error.
    std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let overflow = Arc::new(AtomicBool::new(false));
    let read = |mut r: Box<dyn Read + Send>, cap: usize, flag: Option<Arc<AtomicBool>>| {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let mut buf = [0u8; 8192];
            while let Ok(n) = r.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if out.len() + n > cap {
                    match &flag {
                        Some(f) => {
                            f.store(true, Ordering::SeqCst);
                            break;
                        }
                        None => continue,
                    }
                }
                out.extend_from_slice(&buf[..n]);
            }
            out
        })
    };
    let out_t = read(Box::new(child.stdout.take().unwrap()), MAX_STDOUT_BYTES, Some(overflow.clone()));
    let err_t = read(Box::new(child.stderr.take().unwrap()), MAX_STDERR_BYTES, None);
    let deadline = Instant::now() + timeout;
    let status = loop {
        if overflow.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("コマンドの出力が大きすぎます。".into());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("コマンドが{}秒以内に完了しませんでした。", timeout.as_secs_f64().round() as u64));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = String::from_utf8_lossy(&out_t.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).into_owned();
    if overflow.load(Ordering::SeqCst) {
        return Err("コマンドの出力が大きすぎます。".into());
    }
    if status.success() {
        return Ok(ProcessOutput { stdout, stderr });
    }
    let detail = stderr.trim();
    if !detail.is_empty() {
        return Err(detail.to_string());
    }
    use std::os::unix::process::ExitStatusExt;
    Err(match status.signal() {
        Some(sig) => format!("コマンドがシグナル {sig}で終了しました。"),
        None => format!("コマンドが終了コード {}で終了しました。", status.code().unwrap_or(1)),
    })
}

/// A private temporary directory removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> std::io::Result<Self> {
        let dir = std::env::temp_dir().join(format!("{prefix}{}", uuid::Uuid::new_v4().simple()));
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(TempDir(dir))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
