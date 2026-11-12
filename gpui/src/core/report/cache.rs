//! reportOutputCache.ts — stored in PersistState instead of localStorage.
use super::aggregation::AiAggregationRow;
use crate::api::{GroupBy, ReportCopyFormat, ReportEntry, ReportRange, ReportRow};
use serde::{Deserialize, Serialize};

pub const MAX_CACHE_ITEMS: usize = 8;
pub const MAX_CACHE_AGE_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedOutput {
    pub key: String,
    pub created_at: i64,
    pub total_minutes: i64,
    pub rows: Vec<AiAggregationRow>,
}

pub struct CacheKeyInput {
    pub range: ReportRange,
    pub anchor: String,
    pub group_by: GroupBy,
    pub client_ids: Vec<String>,
    pub project_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub report_rows: Vec<ReportRow>,
    pub entries: Vec<ReportEntry>,
    pub total_minutes: i64,
    pub copy_format: ReportCopyFormat,
}

pub fn output_cache_key(i: &CacheKeyInput) -> String {
    let mut entries = i.entries.clone();
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    let sorted = |v: &Vec<String>| {
        let mut v = v.clone();
        v.sort();
        v
    };
    serde_json::json!({
        "version": 2,
        "entries": entries,
        "range": i.range,
        "anchor": i.anchor,
        "groupBy": i.group_by,
        "clientIds": sorted(&i.client_ids),
        "projectIds": sorted(&i.project_ids),
        "tagIds": sorted(&i.tag_ids),
        "reportRows": i.report_rows.iter().map(|r| serde_json::json!({"key": r.key, "label": r.label, "totalMinutes": r.total_minutes})).collect::<Vec<_>>(),
        "totalMinutes": i.total_minutes,
        "copyFormat": i.copy_format,
    })
    .to_string()
}

fn fresh(c: &CachedOutput, now: i64) -> bool {
    now - c.created_at <= MAX_CACHE_AGE_MS
}

pub fn load_output<'a>(cache: &'a [CachedOutput], key: &str, now: i64) -> Option<&'a CachedOutput> {
    cache.iter().find(|c| c.key == key && fresh(c, now))
}

/// Newest first, expired and same-key items dropped, at most 8.
pub fn save_output(cache: &mut Vec<CachedOutput>, entry: CachedOutput, now: i64) {
    cache.retain(|c| c.key != entry.key && fresh(c, now));
    cache.insert(0, entry);
    cache.truncate(MAX_CACHE_ITEMS);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{GroupBy, ReportCopyField as F, ReportRange, ReportRow};
    use crate::core::report::aggregation::AiAggregationRow;
    use crate::core::report::fixtures::*;

    fn input() -> CacheKeyInput {
        CacheKeyInput {
            range: ReportRange::Month,
            anchor: "2026-08-01".into(),
            group_by: GroupBy::Project,
            client_ids: vec!["client-b".into(), "client-a".into()],
            project_ids: vec![],
            tag_ids: vec![],
            report_rows: vec![ReportRow { key: "project".into(), label: "案件".into(), color: None, total_minutes: 90 }],
            total_minutes: 90,
            entries: vec![],
            copy_format: format("standard", "tab", true, "目的別に分類する", vec![col_field("category", F::Category)]),
        }
    }

    #[test]
    fn key_ignores_selection_order() {
        let mut other = input();
        other.client_ids = vec!["client-a".into(), "client-b".into()];
        assert_eq!(output_cache_key(&input()), output_cache_key(&other));
    }

    #[test]
    fn key_changes_with_entry_text() {
        let mut a = input();
        a.entries = vec![report_entry("entry", 90, Some("実装"), Some("認証"), None)];
        let mut b = input();
        b.entries = vec![report_entry("entry", 90, Some("実装"), Some("決済"), None)];
        assert_ne!(output_cache_key(&a), output_cache_key(&b));
    }

    #[test]
    fn key_changes_with_format() {
        let mut b = input();
        b.copy_format.delimiter = "comma".into();
        assert_ne!(output_cache_key(&input()), output_cache_key(&b));
    }

    #[test]
    fn saved_result_restores_and_expires_after_7_days() {
        let mut cache = Vec::new();
        let key = output_cache_key(&input());
        let created = 1_785_801_600_000i64; // 2026-08-04 UTC
        let rows = vec![AiAggregationRow { label: "開発".into(), summary: "実装".into(), minutes: 90, entry_count: 2, generated_values: Default::default() }];
        save_output(&mut cache, CachedOutput { key: key.clone(), created_at: created, total_minutes: 90, rows }, created);
        assert_eq!(load_output(&cache, &key, created).unwrap().rows[0].label, "開発");
        assert!(load_output(&cache, &key, created + 8 * 24 * 60 * 60 * 1000).is_none());
    }

    #[test]
    fn keeps_at_most_8_newest() {
        let mut cache = Vec::new();
        for i in 0..10 {
            save_output(&mut cache, CachedOutput { key: format!("k{i}"), created_at: i, total_minutes: 0, rows: vec![] }, i);
        }
        assert_eq!(cache.len(), 8);
        assert_eq!(cache[0].key, "k9");
    }
}
