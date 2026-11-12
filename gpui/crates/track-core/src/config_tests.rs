use crate::config::*;
use std::path::Path;

fn write(dir: &Path, v: serde_json::Value) {
    std::fs::write(dir.join("config.json"), v.to_string()).unwrap();
}

fn read(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap()).unwrap()
}

#[test]
fn load_config_normalizes_like_tauri() {
    let d = tempfile::tempdir().unwrap();
    let c = load_config(d.path());
    assert_eq!(c, Config::defaults(d.path()));
    assert_eq!((c.backup_interval_hours, c.backup_keep, c.ai_provider.as_str()), (24, 30, "apple-intelligence"));
    assert_eq!(c.export_dir, d.path().join("exports"));
    write(d.path(), serde_json::json!({
        "exportDir": "  ", "backupKeep": 0, "backupIntervalHours": -1, "aiProvider": "x",
        "aiCodexModel": " gpt ", "aiCommandArgs": [" -a ", "", 3, "b"]
    }));
    let c = load_config(d.path());
    assert_eq!(c.export_dir, d.path().join("exports"));
    assert_eq!((c.backup_keep, c.backup_interval_hours, c.ai_provider.as_str()), (30, 24, "apple-intelligence"));
    assert_eq!(c.ai_codex_model, "gpt");
    assert_eq!(c.ai_command_args, vec!["-a".to_string(), "b".to_string()]);
    write(d.path(), serde_json::json!({"backupIntervalHours": 0, "backupKeep": 5, "aiProvider": "codex", "exportDir": "/x/y"}));
    let c = load_config(d.path());
    assert_eq!((c.backup_interval_hours, c.backup_keep, c.ai_provider.as_str()), (0, 5, "codex"));
    assert_eq!(c.export_dir, Path::new("/x/y"));
    assert_eq!(c.app_config().ai_provider, "codex");
}

#[test]
fn save_config_keeps_unknown_keys() {
    let d = tempfile::tempdir().unwrap();
    write(d.path(), serde_json::json!({"futureKey": 1, "aiProvider": "codex"}));
    let c = save_config_in(d.path(), ConfigPatch { backup_keep: Some(7), ..Default::default() }, d.path()).unwrap();
    assert_eq!(c.backup_keep, 7);
    let v = read(d.path());
    assert_eq!(v["futureKey"], 1);
    assert_eq!(v["backupKeep"], 7);
    assert_eq!(v["aiProvider"], "codex");
    // Every known key is written, like the server's saveConfig.
    for k in ["exportDir", "backupIntervalHours", "aiCodexExecutable", "aiCodexModel", "aiCommandExecutable", "aiCommandArgs"] {
        assert!(v.get(k).is_some(), "{k}");
    }
    assert!(std::fs::read_to_string(d.path().join("config.json")).unwrap().ends_with("}\n"));
}

#[test]
fn dev_export_dir_outside_root_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(root.path(), serde_json::json!({"backupKeep": 3}));
    let before = std::fs::read_to_string(root.path().join("config.json")).unwrap();
    let err = save_config_in(root.path(), ConfigPatch { export_dir: Some(outside.path().join("b").to_string_lossy().into()), ..Default::default() }, root.path()).unwrap_err();
    assert_eq!(err.code(), "dev_guard");
    assert_eq!(std::fs::read_to_string(root.path().join("config.json")).unwrap(), before);
    assert!(!outside.path().join("b").exists());
    let inside = root.path().join("exports2");
    let c = save_config_in(root.path(), ConfigPatch { export_dir: Some(format!(" {} ", inside.display())), ..Default::default() }, root.path()).unwrap();
    assert_eq!(c.export_dir, inside);
    assert!(inside.is_dir(), "created like the server");
}

#[test]
fn relative_export_dir_is_rejected() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(save_config_in(d.path(), ConfigPatch { export_dir: Some("rel/dir".into()), ..Default::default() }, d.path()).unwrap_err().code(), "not_absolute");
    assert_eq!(save_config_in(d.path(), ConfigPatch { export_dir: Some(d.path().join("../x").to_string_lossy().into()), ..Default::default() }, d.path()).unwrap_err().code(), "dev_guard");
}

#[test]
fn interval_and_keep_ranges() {
    let d = tempfile::tempdir().unwrap();
    let bad = |p: ConfigPatch| save_config_in(d.path(), p, d.path()).unwrap_err().code().to_string();
    assert_eq!(bad(ConfigPatch { backup_interval_hours: Some(721), ..Default::default() }), "invalid_input");
    assert_eq!(bad(ConfigPatch { backup_keep: Some(0), ..Default::default() }), "invalid_input");
    assert_eq!(bad(ConfigPatch { backup_keep: Some(1001), ..Default::default() }), "invalid_input");
    assert_eq!(bad(ConfigPatch { ai_provider: Some("gpt".into()), ..Default::default() }), "invalid_input");
    assert_eq!(bad(ConfigPatch { ai_command_args: Some(vec!["a".into(); 33]), ..Default::default() }), "invalid_input");
    assert_eq!(bad(ConfigPatch { ai_command_args: Some(vec![" ".into()]), ..Default::default() }), "invalid_input");
    let c = save_config_in(d.path(), ConfigPatch { backup_interval_hours: Some(0), ai_command_args: Some(vec![" -x ".into()]), ..Default::default() }, d.path()).unwrap();
    assert_eq!((c.backup_interval_hours, c.ai_command_args), (0, vec!["-x".to_string()]));
}

#[test]
fn suggestions_hidden_in_dev() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("Library/Mobile Documents/com~apple~CloudDocs")).unwrap();
    assert!(export_dir_suggestions(home.path(), true).is_empty());
    let s = export_dir_suggestions(home.path(), false);
    assert_eq!(s, vec![("iCloud Drive".to_string(), home.path().join("Library/Mobile Documents/com~apple~CloudDocs/Track"))]);
    assert!(export_dir_suggestions(tempfile::tempdir().unwrap().path(), false).is_empty());
}
