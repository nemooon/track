//! `AppleIntelligenceProvider`: the on-device model through Track's bundled helper app.
use super::process::{DEFAULT_TIMEOUT, TempDir, run_process};
use super::{AiMode, AiProgress, AiProvider, AiStatus, ai_error, clean_response};
use crate::CoreError;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub struct AppleIntelligence {
    helper: Option<PathBuf>,
}

impl AppleIntelligence {
    pub fn new(home: &Path) -> Self {
        let candidates = [
            PathBuf::from("/Applications/Track.app/Contents/Resources/TrackAIHelper.app"),
            home.join("Applications/Track.app/Contents/Resources/TrackAIHelper.app"),
        ];
        AppleIntelligence { helper: candidates.into_iter().find(|p| p.exists()) }
    }

    pub fn with_helper(helper: Option<PathBuf>) -> Self {
        AppleIntelligence { helper }
    }
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?.write_all(text.as_bytes())
}

impl AiProvider for AppleIntelligence {
    fn id(&self) -> &'static str {
        "apple-intelligence"
    }

    fn status(&self) -> AiStatus {
        let mac = cfg!(target_os = "macos");
        let available = mac && self.helper.is_some();
        AiStatus {
            provider: self.id(),
            label: "Apple Intelligence".into(),
            available,
            detail: if available {
                "端末内モデルを使用します。"
            } else if !mac {
                "macOSでのみ利用できます。"
            } else {
                "Apple Intelligenceヘルパーが見つかりません。"
            }
            .into(),
        }
    }

    fn generate(&self, mode: AiMode, input: &str, on_progress: &mut dyn FnMut(AiProgress)) -> Result<String, CoreError> {
        let Some(helper) = &self.helper else {
            return Err(ai_error("Apple Intelligenceヘルパーが見つかりません。Track.app をインストールしてください。"));
        };
        let dir = TempDir::new("track-ai-")?;
        let (input_path, output_path, error_path) = (dir.0.join("prompt.txt"), dir.0.join("response.txt"), dir.0.join("error.txt"));
        on_progress(AiProgress::status("Apple Intelligenceで内容を確認しています…"));
        write_private(&input_path, input)?;
        write_private(&output_path, "")?;
        write_private(&error_path, "")?;
        let s = |p: &Path| p.to_string_lossy().into_owned();
        let args = ["-n", "-W", "--stdout", &s(&output_path), "--stderr", &s(&error_path), &s(helper), "--args", &s(&input_path), mode.as_str()].map(String::from);
        let helper_error = || std::fs::read_to_string(&error_path).unwrap_or_default().trim().to_string();
        if let Err(e) = run_process(Path::new("/usr/bin/open"), &args, "", DEFAULT_TIMEOUT) {
            let he = helper_error();
            return Err(ai_error(if he.is_empty() { e } else { he }));
        }
        let response = std::fs::read_to_string(&output_path).unwrap_or_default();
        let he = helper_error();
        if response.trim().is_empty() && !he.is_empty() {
            return Err(ai_error(he));
        }
        clean_response(&response)
    }
}
