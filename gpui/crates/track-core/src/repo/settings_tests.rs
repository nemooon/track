use crate::model::*;
use crate::repo::default_report_copy_formats;
use crate::repo::repo_tests::db;

fn code<T: std::fmt::Debug>(r: crate::error::Result<T>) -> String {
    r.unwrap_err().code().to_string()
}

fn agg_format(id: &str, columns: Vec<ReportCopyColumn>) -> ReportCopyFormat {
    ReportCopyFormat {
        id: id.into(),
        name: format!(" {id} "),
        target: "ai-aggregation".into(),
        delimiter: "comma".into(),
        include_header: false,
        ai_prompt: " 目的別 ".into(),
        columns,
    }
}

fn field(id: &str, f: ReportCopyField) -> ReportCopyColumn {
    ReportCopyColumn::Field { id: id.into(), field: f, label: None, duration_format: None }
}

#[test]
fn update_settings_partial_keeps_other_fields() {
    let (_d, mut db) = db();
    let before = db.settings().unwrap();
    let s = db.update_settings(SettingsPatch { work_start: Some(540), ..Default::default() }).unwrap();
    assert_eq!(s.work_start, 540);
    assert_eq!((s.work_end, s.work_days.clone(), s.weekly_report_template.clone()), (before.work_end, before.work_days, before.weekly_report_template));
    let s = db.update_settings(SettingsPatch { work_days: Some(vec![1, 3, 5]), ..Default::default() }).unwrap();
    assert_eq!(s.work_days, vec![1, 3, 5]);
    assert_eq!(db.settings().unwrap(), s);
}

#[test]
fn end_before_start_is_invalid_schedule() {
    let (_d, mut db) = db();
    assert_eq!(code(db.update_settings(SettingsPatch { work_start: Some(600), work_end: Some(600), ..Default::default() })), "invalid_schedule");
    assert_eq!(code(db.update_settings(SettingsPatch { work_start: Some(1441), ..Default::default() })), "invalid_input");
    assert_eq!(code(db.update_settings(SettingsPatch { work_days: Some(vec![7]), ..Default::default() })), "invalid_input");
}

#[test]
fn empty_work_days_is_invalid_schedule() {
    let (_d, mut db) = db();
    assert_eq!(code(db.update_settings(SettingsPatch { work_days: Some(vec![]), ..Default::default() })), "invalid_schedule");
}

#[test]
fn formats_roundtrip_in_tauri_json_shape() {
    let (_d, mut db) = db();
    let f = agg_format(
        "f1",
        vec![
            field("c1", ReportCopyField::Category),
            ReportCopyColumn::Field { id: "c2".into(), field: ReportCopyField::Duration, label: Some("時間".into()), duration_format: Some(ReportDurationFormat::Decimal) },
            ReportCopyColumn::Ai { id: "c3".into(), label: " 要約 ".into(), prompt: " 一行で ".into() },
        ],
    );
    let s = db.update_settings(SettingsPatch { report_copy_formats: Some(vec![f]), ..Default::default() }).unwrap();
    let raw: String = db.conn().query_row(r#"SELECT reportCopyFormats FROM "Settings""#, [], |r| r.get(0)).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(v[0]["name"], "f1", "trimmed like zod");
    assert_eq!(v[0]["includeHeader"], false);
    assert_eq!(v[0]["aiPrompt"], "目的別");
    assert_eq!(v[0]["columns"][1]["durationFormat"], "decimal");
    assert_eq!(v[0]["columns"][1]["kind"], "field");
    assert_eq!(v[0]["columns"][2]["label"], "要約");
    assert_eq!(db.settings().unwrap().report_copy_formats, s.report_copy_formats);
    assert_eq!(s.report_copy_formats[0].name, "f1");
}

#[test]
fn ai_column_only_for_aggregation() {
    let (_d, mut db) = db();
    let mut f = agg_format("f", vec![ReportCopyColumn::Ai { id: "a".into(), label: "x".into(), prompt: "y".into() }]);
    f.target = "entries".into();
    assert_eq!(code(db.update_settings(SettingsPatch { report_copy_formats: Some(vec![f]), ..Default::default() })), "invalid_input");
}

#[test]
fn field_not_allowed_for_target_is_rejected() {
    let (_d, mut db) = db();
    let f = agg_format("f", vec![field("c", ReportCopyField::Title)]);
    assert_eq!(code(db.update_settings(SettingsPatch { report_copy_formats: Some(vec![f]), ..Default::default() })), "invalid_input");
    let mut e = agg_format("e", vec![field("c", ReportCopyField::Title)]);
    e.target = "entries".into();
    assert!(db.update_settings(SettingsPatch { report_copy_formats: Some(vec![e]), ..Default::default() }).is_ok());
    assert_eq!(code(db.update_settings(SettingsPatch { report_copy_formats: Some(vec![]), ..Default::default() })), "invalid_input");
    assert_eq!(code(db.update_settings(SettingsPatch { report_copy_formats: Some(vec![agg_format("n", vec![])]), ..Default::default() })), "invalid_input");
    let many: Vec<_> = (0..21).map(|i| agg_format(&format!("f{i}"), vec![field("c", ReportCopyField::Category)])).collect();
    assert_eq!(code(db.update_settings(SettingsPatch { report_copy_formats: Some(many), ..Default::default() })), "invalid_input");
    assert_eq!(default_report_copy_formats().len(), 1);
}

#[test]
fn blank_template_is_rejected() {
    let (_d, mut db) = db();
    assert_eq!(code(db.update_settings(SettingsPatch { weekly_report_template: Some("  \n ".into()), ..Default::default() })), "invalid_input");
    let s = db.update_settings(SettingsPatch { weekly_report_template: Some(" ## 今週\n ".into()), ..Default::default() }).unwrap();
    assert_eq!(s.weekly_report_template, "## 今週");
}
