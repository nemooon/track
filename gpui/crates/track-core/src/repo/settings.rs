//! The single `Settings` row and `config.json`.
use crate::error::Result;
use crate::model::{AppConfig, ReportCopyFormat, Settings};
use crate::Db;
use std::path::Path;

/// `DEFAULT_REPORT_COPY_FORMATS` in src/server/routes/settings.ts.
pub fn default_report_copy_formats() -> Vec<ReportCopyFormat> {
    serde_json::from_value(serde_json::json!([{
        "id": "standard-output", "name": "標準", "target": "ai-aggregation", "delimiter": "tab",
        "includeHeader": true, "aiPrompt": "",
        "columns": [
            { "id": "standard-output-1", "kind": "field", "field": "category" },
            { "id": "standard-output-2", "kind": "field", "field": "duration", "durationFormat": "hours-minutes" },
            { "id": "standard-output-3", "kind": "field", "field": "percentage" }
        ]
    }]))
    .unwrap()
}

/// Aggregation formats from the stored JSON; anything unreadable or empty → the default.
pub fn parse_report_copy_formats(raw: &str) -> Vec<ReportCopyFormat> {
    let parsed: Vec<ReportCopyFormat> = serde_json::from_str(raw).unwrap_or_default();
    let agg: Vec<ReportCopyFormat> = parsed.into_iter().filter(|f| f.target == "ai-aggregation").collect();
    if agg.is_empty() { default_report_copy_formats() } else { agg }
}

impl Db {
    pub fn settings(&self) -> Result<Settings> {
        Ok(self.conn().query_row(
            r#"SELECT workStart, workEnd, workDays, weeklyReportTemplate, reportCopyFormats FROM "Settings" LIMIT 1"#,
            [],
            |r| {
                let days: String = r.get(2)?;
                let formats: String = r.get(4)?;
                Ok(Settings {
                    work_start: r.get(0)?,
                    work_end: r.get(1)?,
                    work_days: days.split(',').filter_map(|d| d.trim().parse().ok()).collect(),
                    weekly_report_template: r.get(3)?,
                    report_copy_formats: parse_report_copy_formats(&formats),
                })
            },
        )?)
    }
}

/// `~/.track/config.json` (only the AI fields the app uses). Missing file → defaults.
pub fn app_config(data_dir: &Path) -> AppConfig {
    let raw = std::fs::read_to_string(data_dir.join("config.json")).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    AppConfig {
        ai_provider: Some(s("aiProvider")).filter(|p| !p.is_empty()).unwrap_or_else(|| "apple-intelligence".into()),
        ai_codex_executable: s("aiCodexExecutable"),
        ai_codex_model: s("aiCodexModel"),
        ai_command_executable: s("aiCommandExecutable"),
        ai_command_args: v.get("aiCommandArgs").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
    }
}
