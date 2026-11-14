//! GPUI-only settings (`gpui-config.json` in the data directory): backups, AI and appearance.
//! The values and rules are those of the Tauri app's `config.json` (src/server/config.ts), which
//! is read once to seed this file and never written.
use crate::error::{CoreError, Result};
use crate::model::AppConfig;
use serde_json::{Map, Value};
use std::path::{Component, Path, PathBuf};

pub const PROVIDERS: [&str; 3] = ["apple-intelligence", "codex", "custom-command"];

/// This app's own settings file.
pub const FILE: &str = "gpui-config.json";
/// The Tauri app's file, read only to seed `FILE`.
pub const LEGACY_FILE: &str = "config.json";

/// Light / dark choice; `System` follows macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn key(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "system" => Some(Appearance::System),
            "light" => Some(Appearance::Light),
            "dark" => Some(Appearance::Dark),
            _ => None,
        }
    }
}

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
    pub appearance: Appearance,
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
            appearance: Appearance::System,
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
        set("appearance", Value::String(self.appearance.key().into()));
    }
}

fn path_of(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE)
}

fn read_object_at(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

fn read_object(data_dir: &Path) -> Map<String, Value> {
    read_object_at(&path_of(data_dir))
}

fn write_object(data_dir: &Path, obj: Map<String, Value>) -> Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let tmp = data_dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(&Value::Object(obj)).unwrap() + "\n")?;
    std::fs::rename(&tmp, path_of(data_dir))?;
    Ok(())
}

/// Seeds `gpui-config.json` from the Tauri app's `config.json` the first time (or with the
/// defaults when there is none). Once the file exists — even unreadable — it is left alone.
/// A backup folder outside `dev_root` is not carried over (the real backups live there).
pub fn migrate_in(data_dir: &Path, dev_root: &Path) -> Result<()> {
    if path_of(data_dir).exists() {
        return Ok(());
    }
    let mut c = normalize(data_dir, read_object_at(&data_dir.join(LEGACY_FILE)));
    if !within(&c.export_dir, dev_root) {
        c.export_dir = Config::defaults(data_dir).export_dir;
    }
    let mut obj = Map::new();
    c.write_into(&mut obj);
    write_object(data_dir, obj)
}

/// Reads `gpui-config.json`, normalised like `loadConfig` (defaults when missing). Never writes:
/// seeding is `migrate_in`, run by the app only once the development guard has passed.
pub fn load_config(data_dir: &Path) -> Config {
    normalize(data_dir, read_object(data_dir))
}

/// Same normalisation as `loadConfig` (without the `TRACK_EXPORT_DIR` override).
fn normalize(data_dir: &Path, v: Map<String, Value>) -> Config {
    let base = Config::defaults(data_dir);
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
        appearance: v.get("appearance").and_then(Value::as_str).and_then(Appearance::from_key).unwrap_or_default(),
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
    pub appearance: Option<Appearance>,
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
    // A file that exists but cannot be read is the user's to fix: saving over it would drop
    // every value it held.
    if let Ok(text) = std::fs::read_to_string(path_of(data_dir))
        && !serde_json::from_str::<Value>(&text).is_ok_and(|v| v.is_object())
    {
        return Err(CoreError::with_details(
            "config_unreadable",
            format!("{FILE} を読めないため保存できません。ファイルを直すか削除してください。"),
            serde_json::json!({ "path": path_of(data_dir) }),
        ));
    }
    migrate_in(data_dir, dev_root)?;
    let mut next = normalize(data_dir, read_object(data_dir));
    if let Some(a) = patch.appearance {
        next.appearance = a;
    }
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
    write_object(data_dir, obj)?;
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
