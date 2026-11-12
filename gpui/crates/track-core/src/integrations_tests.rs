use crate::integrations::*;
use crate::runtime::write_runtime;
use std::os::unix::fs::PermissionsExt;

#[test]
fn runtime_written_atomically_0600() {
    let dir = tempfile::tempdir().unwrap();
    let cli = std::path::Path::new("/Applications/Track.app/Contents/MacOS/track-cli");
    let path = write_runtime(dir.path(), cli).unwrap();
    assert_eq!(path, dir.path().join("runtime.json"));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["pid"], std::process::id());
    assert_eq!(v["cliPath"], cli.to_str().unwrap());
    assert_eq!(v["dbPath"], dir.path().join("track.db").to_str().unwrap());
    assert!(v["startedAt"].as_str().unwrap().ends_with('Z'));
    assert!(v.get("apiBase").is_none());
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    // Rewriting leaves no temporary files behind.
    write_runtime(dir.path(), cli).unwrap();
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from("runtime.json")]);
}

#[test]
fn installs_dev_skill_under_track_dev() {
    let home = tempfile::tempdir().unwrap();
    let paths = install_integrations_in(home.path(), IntegrationKind::Both, true).unwrap();
    let skill_dir = home.path().join(".codex/skills/track-dev");
    let command = home.path().join(".claude/commands/track-dev.md");
    assert_eq!(paths, vec![skill_dir.clone(), command.clone()]);
    let skill = std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
    assert!(skill.lines().any(|l| l == "name: track-dev"));
    assert!(skill.contains("TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw \"$HOME/.track-gpui/runtime.json\")"));
    assert!(skill.contains("TRACK_DATA_DIR=\"$HOME/.track-gpui\" \"$TRACK_CLI\" prepare"));
    assert!(!skill.contains("~/.track/runtime.json"));
    assert!(!skill.contains("network_error"), "HTTP-era errors are rewritten");
    let yaml = std::fs::read_to_string(skill_dir.join("agents/openai.yaml")).unwrap();
    assert!(yaml.contains("$track-dev を使って"));
    let cmd = std::fs::read_to_string(&command).unwrap();
    assert!(cmd.contains("# /track-dev"));
    assert!(cmd.contains("<!-- track:claude-command:v1 -->"));
    assert!(cmd.contains("TRACK_DATA_DIR=\"$HOME/.track-gpui\" \"$TRACK_CLI\" create"));
    // Installing again replaces our own files.
    install_integrations_in(home.path(), IntegrationKind::Both, true).unwrap();
}

#[test]
fn production_text_keeps_track_names() {
    let home = tempfile::tempdir().unwrap();
    install_integrations_in(home.path(), IntegrationKind::Codex, false).unwrap();
    let skill = std::fs::read_to_string(home.path().join(".codex/skills/track/SKILL.md")).unwrap();
    assert!(skill.lines().any(|l| l == "name: track"));
    assert!(skill.contains("TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw ~/.track/runtime.json)\n   \"$TRACK_CLI\" prepare"));
    assert!(skill.contains("Trackアプリを起動しておく必要はない"));
    for stale in ["network_error", "unexpected_response", "通信エラー時", "APIから", "ローカル通信"] {
        assert!(!skill.contains(stale), "{stale} is rewritten");
        assert!(!adapt(include_str!("../../../../integrations/claude/track.md"), false).contains(stale), "{stale} is rewritten in the command");
    }
    assert!(!home.path().join(".claude/commands/track.md").exists(), "only the chosen kind is installed");
}

#[test]
fn never_touches_real_track_skill_in_dev() {
    let home = tempfile::tempdir().unwrap();
    let real_skill = home.path().join(".codex/skills/track/SKILL.md");
    let real_cmd = home.path().join(".claude/commands/track.md");
    std::fs::create_dir_all(real_skill.parent().unwrap()).unwrap();
    std::fs::create_dir_all(real_cmd.parent().unwrap()).unwrap();
    std::fs::write(&real_skill, "---\nname: track\n---\n本物").unwrap();
    std::fs::write(&real_cmd, "<!-- track:claude-command:v1 -->\n本物").unwrap();
    install_integrations_in(home.path(), IntegrationKind::Both, true).unwrap();
    assert_eq!(std::fs::read_to_string(&real_skill).unwrap(), "---\nname: track\n---\n本物");
    assert_eq!(std::fs::read_to_string(&real_cmd).unwrap(), "<!-- track:claude-command:v1 -->\n本物");
}

#[test]
fn refuses_to_overwrite_unrelated_files() {
    let home = tempfile::tempdir().unwrap();
    let other = home.path().join(".codex/skills/track-dev/SKILL.md");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    std::fs::write(&other, "---\nname: something-else\n---\n").unwrap();
    let err = install_integrations_in(home.path(), IntegrationKind::Both, true).unwrap_err();
    assert!(err.to_string().contains("同名のCodexスキルが既にあります"), "{err}");
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "---\nname: something-else\n---\n");
    assert!(!home.path().join(".claude/commands/track-dev.md").exists(), "nothing is written when validation fails");

    let home = tempfile::tempdir().unwrap();
    let cmd = home.path().join(".claude/commands/track-dev.md");
    std::fs::create_dir_all(cmd.parent().unwrap()).unwrap();
    std::fs::write(&cmd, "# my own command").unwrap();
    let err = install_integrations_in(home.path(), IntegrationKind::Claude, true).unwrap_err();
    assert!(err.to_string().contains("同名のClaude Codeコマンドが既にあります"), "{err}");

    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".codex/skills/track-dev");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "x").unwrap();
    assert!(install_integrations_in(home.path(), IntegrationKind::Codex, true).unwrap_err().to_string().contains("空ではないCodexスキルフォルダ"));
}
