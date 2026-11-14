//! Weekly report prompt (ReportsPage.tsx buildWeeklyReportPrompt / replaceMarkdownSection / periodLabel).
use super::copy::format_report_duration;
use crate::model::{ReportDurationFormat, ReportEntriesResponse};
use crate::dates::week_start;
use chrono::{Datelike, Duration, NaiveDate};

pub fn japanese_duration(m: i64) -> String {
    format_report_duration(m, ReportDurationFormat::Japanese)
}

pub fn period_label(anchor: NaiveDate, is_month: bool) -> String {
    if is_month {
        return format!("{}年{}月", anchor.year(), anchor.month());
    }
    let s = week_start(anchor);
    let e = s + Duration::days(6);
    if (s.year(), s.month()) == (e.year(), e.month()) {
        format!("{}/{}/{} – {}", s.year(), s.month(), s.day(), e.day())
    } else {
        format!("{}/{}/{} – {}/{}", s.year(), s.month(), s.day(), e.month(), e.day())
    }
}

struct ProjectTotal {
    key: String,
    label: String,
    minutes: i64,
    count: usize,
    facts: Vec<(String, String)>,
}

/// Returns (prompt, fixed 「主な作業」 body).
pub fn build_weekly_report_prompt(template: &str, period: &str, data: &ReportEntriesResponse) -> (String, String) {
    let template = template.replace("{{期間}}", period).replace("{{合計時間}}", &japanese_duration(data.total_minutes));
    let mut totals: Vec<ProjectTotal> = Vec::new();
    for e in &data.entries {
        let label = match &e.project {
            Some(p) => format!("{} / {}", p.client.name, p.name),
            None => "プロジェクトなし".into(),
        };
        let title = e.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).unwrap_or("タイトルなし").to_string();
        let note = e.note.as_deref().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
        let key = e.project.as_ref().map(|p| p.id.clone()).unwrap_or_else(|| "__no_project__".into());
        let fact = (title, note);
        match totals.iter_mut().find(|t| t.key == key) {
            Some(t) => {
                t.minutes += e.minutes;
                t.count += 1;
                if !t.facts.contains(&fact) {
                    t.facts.push(fact);
                }
            }
            None => totals.push(ProjectTotal { key, label, minutes: e.minutes, count: 1, facts: vec![fact] }),
        }
    }
    totals.sort_by(|a, b| b.minutes.cmp(&a.minutes).then_with(|| a.label.cmp(&b.label)));
    let projects: Vec<String> = totals.iter().map(|t| format!("- {}: {}（{}件）", t.label, japanese_duration(t.minutes), t.count)).collect();
    let main_work = totals
        .iter()
        .map(|t| {
            let facts = t
                .facts
                .iter()
                .map(|(title, note)| {
                    if title == "タイトルなし" && !note.is_empty() {
                        note.clone()
                    } else if note.is_empty() {
                        title.clone()
                    } else {
                        format!("{title}（メモ: {note}）")
                    }
                })
                .collect::<Vec<_>>()
                .join("、");
            format!("- **{}**（{}）：{}", t.label, japanese_duration(t.minutes), facts)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "以下の出力テンプレートに沿って、工数記録から週報を作成してください。\n同じプロジェクトや関連する作業はまとめ、工数記録にある事実だけを簡潔に記載してください。\nテンプレートの構成を保ち、「主な作業」には確定本文をそのまま使用してください。\n確定集計の数値は変更せず、週報の本文だけをMarkdownで返してください。\n\n<出力テンプレート>\n{template}\n</出力テンプレート>\n\n<主な作業の確定本文>\n{main_work}\n</主な作業の確定本文>\n\n<確定集計>\n対象期間: {period}\n合計時間: {}\n記録件数: {}件\n\nプロジェクト別:\n{}\n</確定集計>",
        japanese_duration(data.total_minutes),
        data.entries.len(),
        projects.join("\n"),
    );
    (prompt, main_work)
}

pub fn replace_markdown_section(markdown: &str, heading: &str, body: &str) -> String {
    let trimmed = markdown.trim();
    let lines: Vec<&str> = trimmed.split('\n').collect();
    let target = format!("## {heading}");
    let Some(h) = lines.iter().position(|l| l.trim() == target) else { return trimmed.to_string() };
    let is_heading = |l: &str| {
        let t = l.trim();
        t.starts_with("##") && t[2..].starts_with(char::is_whitespace)
    };
    let next = lines.iter().enumerate().skip(h + 1).find(|(_, l)| is_heading(l)).map(|(i, _)| i).unwrap_or(lines.len());
    let mut out: Vec<&str> = lines[..=h].to_vec();
    out.push("");
    out.push(body.trim());
    out.push("");
    out.extend_from_slice(&lines[next..]);
    out.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ReportEntriesResponse;
    use crate::report::fixtures::*;

    #[test]
    fn weekly_prompt_fills_placeholders_and_main_work() {
        let data = ReportEntriesResponse {
            total_minutes: 150,
            entries: vec![
                report_entry("1", 90, Some("設計"), Some("認証\n周り"), Some(("p1", "Track", "社内"))),
                report_entry("2", 60, None, Some("調査メモ"), None),
            ],
        };
        let (prompt, main) = build_weekly_report_prompt("期間={{期間}} 合計={{合計時間}}", "2026/11/8 – 14", &data);
        assert!(prompt.contains("期間=2026/11/8 – 14 合計=2時間30分"));
        assert_eq!(main, "- **社内 / Track**（1時間30分）：設計（メモ: 認証 周り）\n- **プロジェクトなし**（1時間）：調査メモ");
        assert!(prompt.contains("記録件数: 2件"));
        assert!(prompt.contains("- 社内 / Track: 1時間30分（1件）"));
    }

    #[test]
    fn replaces_main_work_section_only() {
        let md = "## 今週の概要\n要約\n\n## 主な作業\n- AI の文\n\n## 課題\n- なし";
        assert_eq!(replace_markdown_section(md, "主な作業", "- 確定"), "## 今週の概要\n要約\n\n## 主な作業\n\n- 確定\n\n## 課題\n- なし");
        assert_eq!(replace_markdown_section("本文のみ", "主な作業", "x"), "本文のみ");
    }

    #[test]
    fn period_labels() {
        let d = |m, day| chrono::NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        assert_eq!(period_label(d(11, 12), false), "2026/11/8 – 14");
        assert_eq!(period_label(d(11, 1), false), "2026/11/1 – 7");
        assert_eq!(period_label(d(10, 28), false), "2026/10/25 – 31");
        assert_eq!(period_label(d(11, 30), false), "2026/11/29 – 12/5");
        assert_eq!(period_label(d(11, 12), true), "2026年11月");
    }
}
