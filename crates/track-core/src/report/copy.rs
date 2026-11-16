//! reportCopy.ts
use super::aggregation::AiAggregationRow;
use crate::model::{ReportCopyColumn, ReportCopyField, ReportCopyFormat, ReportDurationFormat};
#[cfg(test)]
use crate::model::ReportEntry;
#[cfg(test)]
use chrono::Local;

pub fn field_label(f: ReportCopyField) -> &'static str {
    use ReportCopyField::*;
    match f {
        Date => "日付",
        Start => "開始",
        End => "終了",
        Client => "クライアント",
        Project => "プロジェクト",
        Title => "内容",
        Note => "メモ",
        Tags => "タグ",
        Duration => "時間",
        DurationMinutes => "分数",
        Percentage => "割合",
        Category => "集計名",
        Summary => "概要",
        EntryCount => "件数",
    }
}

pub fn report_copy_format_uses_ai(f: &ReportCopyFormat) -> bool {
    !f.ai_prompt.trim().is_empty()
        || f.columns.iter().any(|c| matches!(c, ReportCopyColumn::Ai { .. } | ReportCopyColumn::Field { field: ReportCopyField::Summary, .. }))
}

pub fn report_copy_column_header(c: &ReportCopyColumn) -> String {
    match c {
        ReportCopyColumn::Ai { label, .. } => label.clone(),
        ReportCopyColumn::Field { label: Some(l), .. } | ReportCopyColumn::Blank { label: Some(l), .. } => l.clone(),
        ReportCopyColumn::Blank { label: None, .. } => String::new(),
        ReportCopyColumn::Field { field, label: None, .. } => field_label(*field).to_string(),
    }
}

#[cfg(test)] // used by the settings screen in the React app; kept for parity tests
pub fn reorder_report_copy_columns(columns: &[ReportCopyColumn], column_id: &str, target_id: &str) -> Vec<ReportCopyColumn> {
    let mut next = columns.to_vec();
    if column_id == target_id {
        return next;
    }
    let (Some(from), Some(to)) = (next.iter().position(|c| c.id() == column_id), next.iter().position(|c| c.id() == target_id)) else {
        return next;
    };
    let c = next.remove(from);
    next.insert(to, c);
    next
}

/// `Number((m / 60).toFixed(2)).toString()`
fn decimal_hours(minutes: i64) -> String {
    let s = format!("{:.2}", minutes as f64 / 60.);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" { "0".into() } else { s.to_string() }
}

pub fn format_report_duration(minutes: i64, f: ReportDurationFormat) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match f {
        ReportDurationFormat::Decimal => decimal_hours(minutes),
        ReportDurationFormat::DecimalWithUnit => format!("{}時間", decimal_hours(minutes)),
        ReportDurationFormat::Japanese if h == 0 => format!("{m}分"),
        ReportDurationFormat::Japanese if m > 0 => format!("{h}時間{m}分"),
        ReportDurationFormat::Japanese => format!("{h}時間"),
        ReportDurationFormat::HoursMinutes if m > 0 => format!("{h}h {m}m"),
        ReportDurationFormat::HoursMinutes => format!("{h}h"),
    }
}

fn percentage(minutes: i64, total: i64) -> String {
    if total > 0 { format!("{}%", (minutes as f64 / total as f64 * 100.).round() as i64) } else { "0%".into() }
}

#[cfg(test)]
fn entry_column_value(e: &ReportEntry, c: &ReportCopyColumn, total: i64) -> String {
    let ReportCopyColumn::Field { field, duration_format, .. } = c else { return String::new() };
    let (start, end) = (e.start.with_timezone(&Local), e.end.with_timezone(&Local));
    match field {
        ReportCopyField::Date => start.format("%Y/%m/%d").to_string(),
        ReportCopyField::Start => start.format("%H:%M").to_string(),
        ReportCopyField::End => end.format("%H:%M").to_string(),
        ReportCopyField::Client => e.project.as_ref().map(|p| p.client.name.clone()).unwrap_or_default(),
        ReportCopyField::Project => e.project.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
        ReportCopyField::Title => e.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).unwrap_or("（タイトルなし）").to_string(),
        ReportCopyField::Note => e.note.clone().unwrap_or_default(),
        ReportCopyField::Tags => e.tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(" / "),
        ReportCopyField::Duration => format_report_duration(e.minutes, duration_format.unwrap_or(ReportDurationFormat::HoursMinutes)),
        ReportCopyField::DurationMinutes => e.minutes.to_string(),
        ReportCopyField::Percentage => percentage(e.minutes, total),
        _ => String::new(),
    }
}

pub fn aggregation_column_value(r: &AiAggregationRow, c: &ReportCopyColumn, total: i64) -> String {
    match c {
        ReportCopyColumn::Blank { .. } => String::new(),
        ReportCopyColumn::Ai { id, .. } => r.generated_values.get(id).cloned().unwrap_or_default(),
        ReportCopyColumn::Field { field, duration_format, .. } => match field {
            ReportCopyField::Category => r.label.clone(),
            ReportCopyField::Summary => r.summary.clone(),
            ReportCopyField::Duration => format_report_duration(r.minutes, duration_format.unwrap_or(ReportDurationFormat::HoursMinutes)),
            ReportCopyField::DurationMinutes => r.minutes.to_string(),
            ReportCopyField::Percentage => percentage(r.minutes, total),
            ReportCopyField::EntryCount => r.entry_count.to_string(),
            _ => String::new(),
        },
    }
}

pub fn escape_cell(v: &str, delimiter: &str) -> String {
    if v.contains(delimiter) || v.contains('\n') || v.contains('\r') || v.contains('"') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn serialize_rows(rows: Vec<Vec<String>>, f: &ReportCopyFormat) -> String {
    let d = if f.delimiter == "comma" { "," } else { "\t" };
    let mut lines = Vec::new();
    if f.include_header {
        lines.push(f.columns.iter().map(report_copy_column_header).collect::<Vec<_>>());
    }
    lines.extend(rows);
    lines.iter().map(|l| l.iter().map(|c| escape_cell(c, d)).collect::<Vec<_>>().join(d)).collect::<Vec<_>>().join("\n")
}

#[cfg(test)] // per-entry export exists only in TS tests
pub fn build_report_copy_text(entries: &[ReportEntry], total: i64, f: &ReportCopyFormat) -> String {
    serialize_rows(entries.iter().map(|e| f.columns.iter().map(|c| entry_column_value(e, c, total)).collect()).collect(), f)
}

pub fn build_aggregation_copy_text(rows: &[AiAggregationRow], total: i64, f: &ReportCopyFormat) -> String {
    serialize_rows(rows.iter().map(|r| f.columns.iter().map(|c| aggregation_column_value(r, c, total)).collect()).collect(), f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ReportCopyColumn as C, ReportCopyField as F, ReportDurationFormat as D, Tag};
    use crate::report::aggregation::AiAggregationRow;
    use crate::report::fixtures::*;

    fn entry() -> crate::model::ReportEntry {
        let mut e = report_entry("entry-1", 90, Some("設計, レビュー"), Some(""), Some(("project-1", "Track", "社内")));
        e.tags = vec![Tag { id: "tag-1".into(), name: "開発".into(), color: "#000000".into() }];
        e
    }

    fn dur(id: &str, f: Option<D>, label: Option<&str>) -> C {
        C::Field { id: id.into(), field: F::Duration, label: label.map(Into::into), duration_format: f }
    }

    #[test]
    fn uses_ai_when_prompt_or_ai_column_or_summary() {
        let f = format("standard", "tab", true, "", vec![col_field("category", F::Category)]);
        assert!(!report_copy_format_uses_ai(&f));
        assert!(report_copy_format_uses_ai(&ReportCopyFormat { ai_prompt: "成果別に分類する".into(), ..f.clone() }));
        let mut with_ai = f.clone();
        with_ai.columns.push(C::Ai { id: "outcome".into(), label: "成果".into(), prompt: "成果を要約する".into() });
        assert!(report_copy_format_uses_ai(&with_ai));
        assert!(report_copy_format_uses_ai(&ReportCopyFormat { columns: vec![col_field("summary", F::Summary)], ..f }));
    }

    #[test]
    fn formats_duration_variants() {
        assert_eq!(format_report_duration(90, D::HoursMinutes), "1h 30m");
        assert_eq!(format_report_duration(90, D::Japanese), "1時間30分");
        assert_eq!(format_report_duration(90, D::DecimalWithUnit), "1.5時間");
        assert_eq!(format_report_duration(90, D::Decimal), "1.5");
        assert_eq!(format_report_duration(80, D::Decimal), "1.33");
        assert_eq!(format_report_duration(30, D::Japanese), "30分");
        assert_eq!(format_report_duration(120, D::Decimal), "2");
    }

    #[test]
    fn duration_format_per_column() {
        let f = format("d", "tab", false, "", vec![dur("duration-ja", Some(D::Japanese), None), dur("duration-decimal", Some(D::Decimal), None)]);
        assert_eq!(build_report_copy_text(&[entry()], 90, &f), "1時間30分\t1.5");
    }

    #[test]
    fn tab_separated_in_given_order_with_header() {
        let f = format("slack", "tab", true, "", vec![
            col_field("column-0", F::Project),
            col_field("column-1", F::Title),
            dur("column-2", Some(D::HoursMinutes), None),
            col_field("column-3", F::Percentage),
        ]);
        assert_eq!(build_report_copy_text(&[entry()], 180, &f), "プロジェクト\t内容\t時間\t割合\nTrack\t設計, レビュー\t1h 30m\t50%");
    }

    #[test]
    fn csv_escapes_commas_and_quotes() {
        let f = format("csv", "comma", false, "", vec![col_field("title-1", F::Title), col_field("minutes", F::DurationMinutes)]);
        assert_eq!(build_report_copy_text(&[entry()], 90, &f), "\"設計, レビュー\",90");
        assert_eq!(escape_cell("a\"b", ","), "\"a\"\"b\"");
    }

    #[test]
    fn same_field_in_multiple_columns() {
        let f = format("dup", "tab", true, "", vec![col_field("project-1", F::Project), col_field("project-2", F::Project)]);
        assert_eq!(build_report_copy_text(&[entry()], 90, &f), "プロジェクト\tプロジェクト\nTrack\tTrack");
    }

    #[test]
    fn custom_headers_and_blank_columns() {
        let f = format("h", "tab", true, "", vec![
            C::Field { id: "project".into(), field: F::Project, label: Some("案件".into()), duration_format: None },
            C::Blank { id: "blank".into(), label: Some("".into()) },
            dur("duration", Some(D::HoursMinutes), Some("工数")),
        ]);
        assert_eq!(build_report_copy_text(&[entry()], 90, &f), "案件\t\t工数\nTrack\t\t1h 30m");
    }

    #[test]
    fn aggregation_with_ai_column() {
        let mut gv = std::collections::BTreeMap::new();
        gv.insert("outcome".to_string(), "コピー機能を改善".to_string());
        let rows = vec![AiAggregationRow { label: "開発".into(), summary: "機能実装".into(), minutes: 90, entry_count: 2, generated_values: gv }];
        let f = format("ai", "tab", true, "成果が同じ作業を分類する", vec![
            col_field("category", F::Category),
            C::Ai { id: "outcome".into(), label: "成果".into(), prompt: "成果を要約".into() },
        ]);
        assert_eq!(build_aggregation_copy_text(&rows, 90, &f), "集計名\t成果\n開発\tコピー機能を改善");
    }

    #[test]
    fn reorder_columns() {
        let cols = vec![col_field("a", F::Category), col_field("b", F::Summary), col_field("c", F::Duration)];
        let ids = |v: Vec<C>| v.iter().map(|c| c.id().to_string()).collect::<Vec<_>>();
        assert_eq!(ids(reorder_report_copy_columns(&cols, "a", "c")), vec!["b", "c", "a"]);
        assert_eq!(ids(reorder_report_copy_columns(&cols, "c", "a")), vec!["c", "a", "b"]);
    }
}
