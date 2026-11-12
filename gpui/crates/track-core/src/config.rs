//! `config.json` in the data directory: backups and AI settings (src/server/config.ts and
//! src/server/routes/config.ts). The Tauri app reads the same file, so unknown keys are kept.
use crate::error::{CoreError, Result};
use crate::model::AppConfig;
use serde_json::{Map, Value};
use std::path::{Component, Path, PathBuf};

pub const PROVIDERS: [&str; 3] = ["apple-intelligence", "codex", "custom-command"];

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Backup folder (iCloud Drive is fine: snapshots are single, finished files).
    pub export_dir: PathBuf,
    /// Hours between automatic backups; 0 disables them.
    pub backup_interval_hours: i64,
    /// Automatic backups to keep.
    pub backup_keep: i64,
    pub ai_provider: String,
    pub ai_codex_executable: String,
    pub ai_codex_model: String,
    pub ai_command_executable: String,
    pub ai_command_args: Vec<String>,
}

impl Config {
    pub fn defaults(data_dir: &Path) -> Self {
        Config {
            export_dir: data_dir.join("exports"),
            backup_interval_hours: 24,
            backup_keep: 30,
            ai_provider: "apple-intelligence".into(),
            ai_codex_executable: String::new(),
            ai_codex_model: String::new(),
            ai_command_executable: String::new(),
            ai_command_args: vec![],
        }
    }

    pub fn app_config(&self) -> AppConfig {
        AppConfig {
            ai_provider: self.ai_provider.clone(),
            ai_codex_executable: self.ai_codex_executable.clone(),
            ai_codex_model: self.ai_codex_model.clone(),
            ai_command_executable: self.ai_command_executable.clone(),
            ai_command_args: self.ai_command_args.clone(),
        }
    }

    fn write_into(&self, obj: &mut Map<String, Value>) {
        let mut set = |k: &str, v: Value| {
            obj.insert(k.into(), v);
        };
        set("exportDir", Value::String(self.export_dir.to_string_lossy().into_owned()));
        set("backupIntervalHours", self.backup_interval_hours.into());
        set("backupKeep", self.backup_keep.into());
        set("aiProvider", Value::String(self.ai_provider.clone()));
        set("aiCodexExecutable", Value::String(self.ai_codex_executable.clone()));
        set("aiCodexModel", Value::String(self.ai_codex_model.clone()));
        set("aiCommandExecutable", Value::String(self.ai_command_executable.clone()));
        set("aiCommandArgs", self.ai_command_args.clone().into());
    }
}

fn path_of(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

fn read_object(data_dir: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path_of(data_dir)).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

/// Same normalisation as `loadConfig` (without the `TRACK_EXPORT_DIR` override).
pub fn load_config(data_dir: &Path) -> Config {
    let base = Config::defaults(data_dir);
    let v = read_object(data_dir);
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(|s| s.trim().to_string());
    let n = |k: &str| v.get(k).and_then(Value::as_f64);
    Config {
        export_dir: v.get("exportDir").and_then(Value::as_str).filter(|d| !d.trim().is_empty()).map(PathBuf::from).unwrap_or(base.export_dir),
        backup_interval_hours: n("backupIntervalHours").filter(|h| *h >= 0.).map(|h| h as i64).unwrap_or(base.backup_interval_hours),
        backup_keep: n("backupKeep").filter(|k| *k >= 1.).map(|k| k as i64).unwrap_or(base.backup_keep),
        ai_provider: s("aiProvider").filter(|p| PROVIDERS.contains(&p.as_str())).unwrap_or(base.ai_provider),
        ai_codex_executable: s("aiCodexExecutable").unwrap_or_default(),
        ai_codex_model: s("aiCodexModel").unwrap_or_default(),
        ai_command_executable: s("aiCommandExecutable").unwrap_or_default(),
        ai_command_args: v
            .get("aiCommandArgs")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default(),
    }
}

/// `PATCH /api/config`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigPatch {
    pub export_dir: Option<String>,
    pub backup_interval_hours: Option<i64>,
    pub backup_keep: Option<i64>,
    pub ai_provider: Option<String>,
    pub ai_codex_executable: Option<String>,
    pub ai_codex_model: Option<String>,
    pub ai_command_executable: Option<String>,
    pub ai_command_args: Option<Vec<String>>,
}

fn invalid(message: &str) -> CoreError {
    CoreError::code_msg("invalid_input", message)
}

fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `path` lies inside `root` (absolute, no `.`/`..` tricks).
pub fn within(path: &Path, root: &Path) -> bool {
    path.is_absolute() && !path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) && path.starts_with(root)
}

pub(crate) fn dev_guard_dir(path: &Path, root: &Path) -> Result<()> {
    if within(path, root) {
        Ok(())
    } else {
        Err(CoreError::code_msg(
            "dev_guard",
            format!("開発版のバックアップはコピー DB のフォルダ（{}）の中だけに置けます（指定: {}）。", root.display(), path.display()),
        ))
    }
}

/// Saves into the user's data dir; while the GPUI version is in development the backup
/// folder must stay under the copy DB's folder.
pub fn save_config(data_dir: &Path, patch: ConfigPatch) -> Result<Config> {
    save_config_in(data_dir, patch, &crate::guard::allowed_root())
}

/// Validates like `updateSchema`, checks the backup folder is writable, then merges into the
/// existing JSON so keys this version does not know survive.
pub fn save_config_in(data_dir: &Path, patch: ConfigPatch, dev_root: &Path) -> Result<Config> {
    let mut next = load_config(data_dir);
    if let Some(h) = patch.backup_interval_hours {
        if !(0..=24 * 30).contains(&h) {
            return Err(invalid("実行間隔は0〜720時間です。"));
        }
        next.backup_interval_hours = h;
    }
    if let Some(k) = patch.backup_keep {
        if !(1..=1000).contains(&k) {
            return Err(invalid("保存数は1〜1000です。"));
        }
        next.backup_keep = k;
    }
    if let Some(p) = &patch.ai_provider {
        if !PROVIDERS.contains(&p.as_str()) {
            return Err(invalid("プロバイダーを選んでください。"));
        }
        next.ai_provider = p.clone();
    }
    for (value, max, slot) in [
        (&patch.ai_codex_executable, 2_000, &mut next.ai_codex_executable),
        (&patch.ai_codex_model, 200, &mut next.ai_codex_model),
        (&patch.ai_command_executable, 2_000, &mut next.ai_command_executable),
    ] {
        if let Some(v) = value {
            let v = v.trim();
            if len16(v) > max {
                return Err(invalid("入力が長すぎます。"));
            }
            *slot = v.to_string();
        }
    }
    if let Some(args) = &patch.ai_command_args {
        let args: Vec<String> = args.iter().map(|a| a.trim().to_string()).collect();
        if args.len() > 32 || args.iter().any(|a| a.is_empty() || len16(a) > 2_000) {
            return Err(invalid("引数を確認してください。"));
        }
        next.ai_command_args = args;
    }
    if let Some(dir) = &patch.export_dir {
        let dir = dir.trim();
        if dir.is_empty() {
            return Err(invalid("保存先フォルダを指定してください。"));
        }
        let dir = PathBuf::from(dir);
        if !dir.is_absolute() {
            return Err(CoreError::code_msg("not_absolute", "保存先は絶対パスで指定してください。"));
        }
        dev_guard_dir(&dir, dev_root)?;
        // An unwritable folder would make every later backup fail silently: create and check now.
        let writable = std::fs::create_dir_all(&dir).is_ok() && {
            let probe = dir.join(format!(".track-write-test-{}", std::process::id()));
            let ok = std::fs::write(&probe, b"").is_ok();
            let _ = std::fs::remove_file(&probe);
            ok
        };
        if !writable {
            return Err(CoreError::with_details("not_writable", "保存先フォルダに書き込めません。", serde_json::json!({ "path": dir })));
        }
        next.export_dir = dir;
    }
    let mut obj = read_object(data_dir);
    next.write_into(&mut obj);
    std::fs::create_dir_all(data_dir)?;
    let tmp = data_dir.join(format!(".config.json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(&Value::Object(obj)).unwrap() + "\n")?;
    std::fs::rename(&tmp, path_of(data_dir))?;
    Ok(next)
}

/// `GET /api/config/suggestions`: iCloud Drive when it exists. Hidden while the dev guard is on,
/// because the real app's backups live there.
pub fn export_dir_suggestions(home: &Path, dev: bool) -> Vec<(String, PathBuf)> {
    if dev {
        return vec![];
    }
    let icloud = home.join("Library/Mobile Documents/com~apple~CloudDocs");
    if icloud.is_dir() { vec![("iCloud Drive".into(), icloud.join("Track"))] } else { vec![] }
}
