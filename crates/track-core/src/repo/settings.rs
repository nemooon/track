//! The single `Settings` row and `config.json`.
use crate::error::{CoreError, Result};
use crate::model::{ReportCopyColumn, ReportCopyField, ReportCopyFormat, Settings, SettingsPatch};
use crate::Db;

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

fn invalid(message: &str) -> CoreError {
    CoreError::code_msg("invalid_input", message)
}

/// UTF-16 length like zod's `.max()`.
fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `reportCopyFormatSchema` in src/shared/validators.ts; returns the format trimmed like zod.
pub fn validate_report_copy_format(f: &ReportCopyFormat) -> Result<ReportCopyFormat> {
    use ReportCopyField::*;
    let mut f = f.clone();
    f.name = f.name.trim().to_string();
    f.ai_prompt = f.ai_prompt.trim().to_string();
    if f.id.is_empty() || len16(&f.id) > 100 || f.name.is_empty() || len16(&f.name) > 100 {
        return Err(invalid("出力フォーマットの名前を確認してください。"));
    }
    if !matches!(f.target.as_str(), "entries" | "ai-aggregation") || !matches!(f.delimiter.as_str(), "tab" | "comma") {
        return Err(invalid("出力フォーマットの対象・区切りを確認してください。"));
    }
    if len16(&f.ai_prompt) > 10_000 || f.columns.is_empty() || f.columns.len() > 30 {
        return Err(invalid("出力フォーマットの列は1〜30個にしてください。"));
    }
    let entries = f.target == "entries";
    for c in &mut f.columns {
        if c.id().is_empty() || len16(c.id()) > 100 {
            return Err(invalid("列の ID が不正です。"));
        }
        match c {
            ReportCopyColumn::Field { field, label, .. } => {
                let allowed = if entries {
                    matches!(field, Date | Start | End | Client | Project | Title | Note | Tags | Duration | DurationMinutes | Percentage)
                } else {
                    matches!(field, Category | Summary | Duration | DurationMinutes | Percentage | EntryCount)
                };
                if !allowed {
                    return Err(invalid("対象では利用できない項目です"));
                }
                if label.as_deref().is_some_and(|l| len16(l) > 100) {
                    return Err(invalid("列名は100文字以内です。"));
                }
            }
            ReportCopyColumn::Blank { label, .. } => {
                if label.as_deref().is_some_and(|l| len16(l) > 100) {
                    return Err(invalid("列名は100文字以内です。"));
                }
            }
            ReportCopyColumn::Ai { label, prompt, .. } => {
                if entries {
                    return Err(invalid("AI生成項目はカスタム出力でのみ利用できます"));
                }
                *label = label.trim().to_string();
                *prompt = prompt.trim().to_string();
                if label.is_empty() || len16(label) > 100 || prompt.is_empty() || len16(prompt) > 1_000 {
                    return Err(invalid("AI生成項目の名前と指示を確認してください。"));
                }
            }
        }
    }
    Ok(f)
}

impl Db {
    /// `PATCH /api/settings`: only the given fields change; the schedule must stay valid.
    pub fn update_settings(&mut self, patch: SettingsPatch) -> Result<Settings> {
        self.ensure_writable()?;
        if [patch.work_start, patch.work_end].iter().flatten().any(|m| !(0..=1440).contains(m)) {
            return Err(invalid("勤務時間を確認してください。"));
        }
        if patch.work_days.as_ref().is_some_and(|d| d.iter().any(|d| *d > 6)) {
            return Err(invalid("勤務日を確認してください。"));
        }
        let template = match &patch.weekly_report_template {
            Some(t) => {
                let t = t.trim().to_string();
                if t.is_empty() || len16(&t) > 10_000 {
                    return Err(invalid("出力テンプレートを入力してください。"));
                }
                Some(t)
            }
            None => None,
        };
        let formats = match &patch.report_copy_formats {
            Some(fs) => {
                if fs.is_empty() || fs.len() > 20 {
                    return Err(invalid("出力フォーマットは1〜20件にしてください。"));
                }
                Some(fs.iter().map(validate_report_copy_format).collect::<Result<Vec<_>>>()?)
            }
            None => None,
        };
        let current = self.settings()?;
        let start = patch.work_start.unwrap_or(current.work_start);
        let end = patch.work_end.unwrap_or(current.work_end);
        let days = patch.work_days.clone().unwrap_or(current.work_days.clone());
        if end <= start || days.is_empty() {
            return Err(CoreError::code_msg("invalid_schedule", "終了は開始より後にし、勤務日を1日以上選んでください。"));
        }
        let tx = self.conn_mut().savepoint()?;
        if let Some(v) = patch.work_start {
            tx.execute(r#"UPDATE "Settings" SET workStart = ?1"#, [v])?;
        }
        if let Some(v) = patch.work_end {
            tx.execute(r#"UPDATE "Settings" SET workEnd = ?1"#, [v])?;
        }
        if let Some(v) = &patch.work_days {
            let joined = v.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(",");
            tx.execute(r#"UPDATE "Settings" SET workDays = ?1"#, [joined])?;
        }
        if let Some(v) = &template {
            tx.execute(r#"UPDATE "Settings" SET weeklyReportTemplate = ?1"#, [v])?;
        }
        if let Some(v) = &formats {
            tx.execute(r#"UPDATE "Settings" SET reportCopyFormats = ?1"#, [serde_json::to_string(v).unwrap()])?;
        }
        tx.commit()?;
        self.settings()
    }

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
