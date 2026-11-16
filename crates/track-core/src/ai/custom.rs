//! `CustomCommandProvider`: the prompt on stdin, the answer on stdout.
use super::process::{DEFAULT_TIMEOUT, resolve_executable, run_process};
use super::{AiMode, AiProgress, AiProvider, AiStatus, ai_error, build_prompt, clean_response};
use crate::CoreError;
use crate::model::AppConfig;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct CustomCommand {
    /// Empty when the configured command was not found.
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl CustomCommand {
    pub fn from_config(config: &AppConfig, home: &Path) -> Self {
        CustomCommand {
            executable: resolve_executable(&config.ai_command_executable, home).unwrap_or_default(),
            args: config.ai_command_args.clone(),
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl AiProvider for CustomCommand {
    fn id(&self) -> &'static str {
        "custom-command"
    }

    fn status(&self) -> AiStatus {
        let available = !self.executable.as_os_str().is_empty();
        AiStatus {
            provider: self.id(),
            label: "カスタムコマンド".into(),
            available,
            detail: if available {
                format!("標準入力でプロンプトを渡します（{}）。", self.executable.display())
            } else {
                "実行可能なコマンドを指定してください。".into()
            },
        }
    }

    fn generate(&self, mode: AiMode, input: &str, on_progress: &mut dyn FnMut(AiProgress)) -> Result<String, CoreError> {
        if self.executable.as_os_str().is_empty() {
            return Err(ai_error("設定したコマンドが見つからないか、実行できません。"));
        }
        on_progress(AiProgress::status("カスタムコマンドを実行しています…"));
        let out = run_process(&self.executable, &self.args, &build_prompt(mode, input), self.timeout).map_err(ai_error)?;
        clean_response(&out.stdout)
    }
}
