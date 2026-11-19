//! reportAggregation.ts
use super::copy::report_copy_column_header;
use crate::model::{ReportCopyColumn, ReportCopyField, ReportCopyFormat, ReportEntriesResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiAggregationRow {
    pub label: String,
    pub summary: String,
    pub minutes: i64,
    pub entry_count: i64,
    pub generated_values: BTreeMap<String, String>,
}

pub fn build_report_aggregation_prompt(data: &ReportEntriesResponse, f: &ReportCopyFormat) -> String {
    let entries: Vec<Value> = data
        .entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "client": e.project.as_ref().map(|p| p.client.name.clone()).unwrap_or_default(),
                "project": e.project.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                "title": e.title.clone().unwrap_or_default(),
                "note": e.note.clone().unwrap_or_default(),
                "tags": e.tags.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
            })
        })
        .collect();
    #[derive(Serialize)]
    struct Col {
        order: usize,
        id: String,
        label: String,
        source: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        instruction: Option<String>,
    }
    let cols: Vec<Col> = f
        .columns
        .iter()
        .filter(|c| !matches!(c, ReportCopyColumn::Blank { .. }))
        .enumerate()
        .map(|(i, c)| Col {
            order: i + 1,
            id: c.id().to_string(),
            label: report_copy_column_header(c),
            source: match c {
                ReportCopyColumn::Ai { .. } => "AI生成",
                ReportCopyColumn::Field { field: ReportCopyField::Category | ReportCopyField::Summary, .. } => "AI集計結果",
                _ => "Trackが元データから計算",
            },
            instruction: match c {
                ReportCopyColumn::Ai { prompt, .. } => Some(prompt.clone()),
                _ => None,
            },
        })
        .collect();
    format!(
        "<集計の観点>\n{}\n</集計の観点>\n\n<出力フォーマット>\n形式名: {}\n区切り文字: {}\n列（指定順）: {}\nAI生成列は各グループのvalues配列に、{{\"id\":\"列ID\",\"value\":\"生成結果\"}}として入れてください。\n</出力フォーマット>\n\n<工数記録>\n{}\n</工数記録>",
        f.ai_prompt.trim(),
        f.name,
        if f.delimiter == "comma" { "カンマ" } else { "タブ" },
        serde_json::to_string(&cols).unwrap(),
        serde_json::to_string(&entries).unwrap(),
    )
    .trim()
    .to_string()
}

fn extract_json(text: &str) -> Result<Value, String> {
    let t = text.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```JSON")).or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.trim().strip_suffix("```").unwrap_or(t).trim();
    let (Some(first), Some(last)) = (t.find('{'), t.rfind('}')) else { return Err("AIの分類結果を読み取れませんでした。".into()) };
    if last <= first {
        return Err("AIの分類結果を読み取れませんでした。".into());
    }
    serde_json::from_str(&t[first..=last]).map_err(|_| "AIの分類結果を読み取れませんでした。".into())
}

pub fn aggregate_ai_result(response: &str, data: &ReportEntriesResponse) -> Result<Vec<AiAggregationRow>, String> {
    let parsed = extract_json(response)?;
    let Some(groups) = parsed.get("groups").and_then(Value::as_array) else {
        return Err("AIの分類結果にグループがありませんでした。".into());
    };
    let by_id: HashMap<&str, i64> = data.entries.iter().map(|e| (e.id.as_str(), e.minutes)).collect();
    let mut assigned: HashSet<String> = HashSet::new();
    let mut rows = Vec::new();
    for g in groups {
        let Some(label) = g.get("label").and_then(Value::as_str).map(str::trim).filter(|l| !l.is_empty()) else { continue };
        let Some(ids) = g.get("entryIds").and_then(Value::as_array) else { continue };
        let mut picked: Vec<String> = Vec::new();
        for id in ids.iter().filter_map(Value::as_str) {
            if by_id.contains_key(id) && !assigned.contains(id) && !picked.iter().any(|p| p == id) {
                picked.push(id.to_string());
            }
        }
        if picked.is_empty() {
            continue;
        }
        assigned.extend(picked.iter().cloned());
        let generated_values = match g.get("values") {
            Some(Value::Array(vs)) => vs
                .iter()
                .filter_map(|v| Some((v.get("id")?.as_str()?.to_string(), v.get("value")?.as_str()?.trim().to_string())))
                .collect(),
            Some(Value::Object(m)) => m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.trim().to_string()))).collect(),
            _ => BTreeMap::new(),
        };
        rows.push(AiAggregationRow {
            label: label.to_string(),
            summary: g.get("summary").and_then(Value::as_str).map(|s| s.trim().to_string()).unwrap_or_default(),
            minutes: picked.iter().map(|id| by_id[id.as_str()]).sum(),
            entry_count: picked.len() as i64,
            generated_values,
        });
    }
    let unassigned: Vec<_> = data.entries.iter().filter(|e| !assigned.contains(&e.id)).collect();
    if !unassigned.is_empty() {
        rows.push(AiAggregationRow {
            label: "その他".into(),
            summary: "AIが分類しなかった工数記録".into(),
            minutes: unassigned.iter().map(|e| e.minutes).sum(),
            entry_count: unassigned.len() as i64,
            generated_values: BTreeMap::new(),
        });
    }
    if rows.is_empty() {
        return Err("AIの分類結果が空でした。".into());
    }
    rows.sort_by(|a, b| b.minutes.cmp(&a.minutes).then_with(|| a.label.cmp(&b.label)));
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ReportCopyColumn as C, ReportCopyField as F, ReportEntriesResponse};
    use crate::report::fixtures::*;

    fn data() -> ReportEntriesResponse {
        ReportEntriesResponse {
            total_minutes: 90,
            entries: vec![report_entry("a", 60, Some("実装"), None, None), report_entry("b", 30, Some("レビュー"), None, None)],
        }
    }

    #[test]
    fn sums_minutes_from_source_entries_using_ai_ids() {
        let rows = aggregate_ai_result(
            "```json\n{\"groups\":[{\"label\":\"開発\",\"summary\":\"実装と確認\",\"entryIds\":[\"a\",\"b\"],\"values\":[{\"id\":\"outcome\",\"value\":\"機能を完成\"}]}]}\n```",
            &data(),
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].label.as_str(), rows[0].summary.as_str(), rows[0].minutes, rows[0].entry_count), ("開発", "実装と確認", 90, 2));
        assert_eq!(rows[0].generated_values.get("outcome").map(String::as_str), Some("機能を完成"));
    }

    #[test]
    fn unassigned_go_to_other_and_duplicates_count_once() {
        let rows = aggregate_ai_result(r#"{"groups":[{"label":"実装","entryIds":["a","a","unknown"]}]}"#, &data()).unwrap();
        assert_eq!(rows.iter().map(|r| r.minutes).sum::<i64>(), 90);
        assert_eq!(rows.iter().find(|r| r.label == "その他").map(|r| r.minutes), Some(30));
    }

    #[test]
    fn rejects_unreadable_or_empty_results() {
        assert!(aggregate_ai_result("no json here", &data()).is_err());
        assert!(aggregate_ai_result(r#"{"nothing":1}"#, &data()).is_err());
    }

    #[test]
    fn prompt_contains_columns_in_order_and_instructions() {
        let f = format("顧客報告", "comma", true, "目的別にまとめる", vec![
            C::Field { id: "category".into(), field: F::Category, label: Some("区分".into()), duration_format: None },
            C::Blank { id: "spacer".into(), label: Some("".into()) },
            C::Ai { id: "outcome".into(), label: "成果".into(), prompt: "成果を20文字以内で要約".into() },
        ]);
        let p = build_report_aggregation_prompt(&data(), &f);
        assert!(p.contains("形式名: 顧客報告"));
        assert!(p.contains("目的別にまとめる"));
        assert!(p.contains(r#""label":"区分""#));
        assert!(p.contains(r#""label":"成果""#));
        assert!(p.contains("成果を20文字以内で要約"));
        assert!(!p.contains(r#""id":"spacer""#));
        assert!(p.contains("区切り文字: カンマ"));
    }
}
