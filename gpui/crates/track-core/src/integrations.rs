//! Installs the Codex skill and the Claude Code command (same places and overwrite rules as
//! `src-tauri/src/lib.rs`), with the text adapted to the direct-DB CLI. The development build
//! installs them as `track-dev` so the real `track` skill and command are never touched.
use crate::error::{CoreError, Result};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const SKILL: &str = include_str!("../../../../integrations/codex/track/SKILL.md");
const OPENAI_YAML: &str = include_str!("../../../../integrations/codex/track/agents/openai.yaml");
const CLAUDE_COMMAND: &str = include_str!("../../../../integrations/claude/track.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationKind {
    Codex,
    Claude,
    Both,
}

fn fail(message: String) -> CoreError {
    CoreError::code_msg("install_failed", message)
}

/// Rewrites the HTTP-era text for the CLI that reads the DB directly, and for `dev` the names and paths.
pub fn adapt(text: &str, dev: bool) -> String {
    let mut t = text
        .replace(
            "1. 読み取り専用の準備コマンドを実行する。ローカル通信がサンドボックスで遮断された場合は通信権限付きで再試行する。",
            "1. 読み取り専用の準備コマンドを実行する。CLIはTrackのデータベースを直接読み書きするため、Trackアプリを起動しておく必要はない。サンドボックスでデータベースや会話ログへのアクセスが拒否された場合は権限付きで再試行する。",
        )
        .replace("通信エラー時は同じ要求ID", "登録に失敗した場合は同じ要求ID")
        .replace("重複がAPIから報告されたら", "重複がCLIから報告されたら")
        .replace(
            "- `cliPath`がない: Trackを起動する。項目自体がなければTrackを更新する。",
            "- `runtime.json`や`cliPath`がない: Trackアプリを一度起動する（CLIの場所が記録される）。項目自体がなければTrackを更新する。",
        )
        .replace(
            "- `network_error`: 通信権限付きで一度再試行し、なお失敗したらTrackの起動と接続先を確認する。",
            "- `dev_guard` / `database_error`: データベースの場所（`TRACK_DATA_DIR`）とアクセス権限を確認する。",
        )
        .replace("- `unexpected_response`: APIとCLIのバージョンを確認し、Trackを更新する。", "- `schema_too_new`: CLIがデータベースより古い。Trackを更新する。");
    if dev {
        t = t
            .replace("~/.track/runtime.json", "\"$HOME/.track-gpui/runtime.json\"")
            .replace("\"$TRACK_CLI\" ", "TRACK_DATA_DIR=\"$HOME/.track-gpui\" \"$TRACK_CLI\" ")
            .replace("name: track\n", "name: track-dev\n")
            .replace("$track", "$track-dev")
            .replace("# /track ", "# /track-dev ")
            .replace("display_name: \"Track 工数記録\"", "display_name: \"Track 工数記録（開発版）\"")
            .replace("description: Trackに", "description: 【開発版】Trackに");
    }
    t
}

fn skill_name(dev: bool) -> &'static str {
    if dev { "track-dev" } else { "track" }
}

fn is_track_skill(path: &Path, name: &str) -> bool {
    std::fs::read_to_string(path).map(|c| c.lines().any(|l| l.trim() == format!("name: {name}"))).unwrap_or(false)
}

fn is_track_command(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|c| {
            c.lines().any(|l| l.trim() == "<!-- track:claude-command:v1 -->")
                || (c.contains("# /track")
                    && (c.contains("個人用工数管理アプリ")
                        || (c.contains("Track (個人の工数管理アプリ)") && c.contains("~/.track/runtime.json") && c.contains("prepare --source claude"))))
        })
        .unwrap_or(false)
}

fn validate_codex(path: &Path, name: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Ok(()),
        Ok(m) if m.is_dir() => {
            let skill = path.join("SKILL.md");
            if skill.exists() {
                if !is_track_skill(&skill, name) {
                    return Err(fail(format!("同名のCodexスキルが既にあります: {}", path.display())));
                }
            } else if std::fs::read_dir(path)?.next().is_some() {
                return Err(fail(format!("空ではないCodexスキルフォルダが既にあります: {}", path.display())));
            }
            Ok(())
        }
        Ok(_) => Err(fail(format!("Codexスキルのインストール先にファイルがあります: {}", path.display()))),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn validate_claude(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Ok(()),
        Ok(m) if m.is_file() => {
            if is_track_command(path) { Ok(()) } else { Err(fail(format!("同名のClaude Codeコマンドが既にあります: {}", path.display()))) }
        }
        Ok(_) => Err(fail(format!("Claude Codeコマンドのインストール先にフォルダがあります: {}", path.display()))),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn replace_symlink(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn write_atomic(destination: &Path, text: &str) -> Result<()> {
    let parent = destination.parent().ok_or_else(|| fail("インストール先が不正です".into()))?;
    std::fs::create_dir_all(parent)?;
    let name = destination.file_name().and_then(|n| n.to_str()).ok_or_else(|| fail("ファイル名が不正です".into()))?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let tmp = parent.join(format!(".{name}.{}-{stamp}.tmp", std::process::id()));
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o644).open(&tmp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    drop(f);
    if let Err(e) = std::fs::rename(&tmp, destination) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Installs into `home`. Every destination is validated before anything is written.
/// Returns the installed skill folder and/or command file.
pub fn install_integrations_in(home: &Path, kind: IntegrationKind, dev: bool) -> Result<Vec<PathBuf>> {
    let name = skill_name(dev);
    let codex = matches!(kind, IntegrationKind::Codex | IntegrationKind::Both).then(|| home.join(".codex/skills").join(name));
    let claude = matches!(kind, IntegrationKind::Claude | IntegrationKind::Both).then(|| home.join(".claude/commands").join(format!("{name}.md")));
    if let Some(p) = &codex {
        validate_codex(p, name)?;
    }
    if let Some(p) = &claude {
        validate_claude(p)?;
    }
    // Only the production install cleans up the old `~/.agents/skills/track` link.
    let legacy = home.join(".agents/skills/track");
    let remove_legacy = !dev && codex.is_some() && std::fs::symlink_metadata(&legacy).is_ok_and(|m| m.file_type().is_symlink() && is_track_skill(&legacy.join("SKILL.md"), "track"));
    let mut installed = Vec::new();
    if let Some(p) = codex {
        replace_symlink(&p)?;
        write_atomic(&p.join("SKILL.md"), &adapt(SKILL, dev))?;
        write_atomic(&p.join("agents/openai.yaml"), &adapt(OPENAI_YAML, dev))?;
        installed.push(p);
    }
    if let Some(p) = claude {
        replace_symlink(&p)?;
        write_atomic(&p, &adapt(CLAUDE_COMMAND, dev))?;
        installed.push(p);
    }
    if remove_legacy {
        std::fs::remove_file(&legacy)?;
    }
    Ok(installed)
}

/// Installs into the user's home directory.
pub fn install_integrations(kind: IntegrationKind, dev: bool) -> Result<Vec<PathBuf>> {
    let home = dirs::home_dir().ok_or_else(|| fail("ホームフォルダが見つかりません。".into()))?;
    install_integrations_in(&home, kind, dev)
}
