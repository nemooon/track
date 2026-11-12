//! The data types live in track-core; this module keeps the HTTP-only helpers.
pub use track_core::model::*;

pub trait QueryParams {
    fn params(&self, with_group: bool) -> Vec<(&'static str, String)>;
}

impl QueryParams for ReportQuery {
    fn params(&self, with_group: bool) -> Vec<(&'static str, String)> {
        let mut p = vec![
            ("range", match self.range { ReportRange::Week => "week", ReportRange::Month => "month" }.to_string()),
            ("anchor", self.anchor.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        ];
        if with_group {
            p.push(("groupBy", match self.group_by { GroupBy::Client => "client", GroupBy::Project => "project", GroupBy::Tag => "tag" }.to_string()));
        }
        for (k, v) in [("clientIds", &self.client_ids), ("projectIds", &self.project_ids), ("tagIds", &self.tag_ids)] {
            if !v.is_empty() {
                p.push((k, v.join(",")));
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_time_entry() {
        let json = r##"{"id":"e1","projectId":"p1","start":"2026-11-12T01:00:00.000Z","end":"2026-11-12T02:00:00.000Z",
          "title":"定例","note":null,"breakMinutes":30,"externalEventId":null,"externalEventSource":null,
          "project":{"id":"p1","clientId":"c1","name":"P","color":"#123456","archived":false,
             "client":{"id":"c1","name":"C","archived":false},"tags":[{"tagId":"t1","tag":{"id":"t1","name":"T","color":"#000000"}}]},
          "tags":[{"tagId":"t2","tag":{"id":"t2","name":"U","color":"#ffffff"}}],"createdAt":"x","updatedAt":"y"}"##;
        let e: TimeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.break_minutes, 30);
        assert_eq!(e.project.as_ref().unwrap().client.name, "C");
        assert_eq!(e.project.as_ref().unwrap().tags[0].tag.id, "t1");
        assert_eq!(e.tags[0].tag.name, "U");
        assert_eq!(e.start.to_rfc3339(), "2026-11-12T01:00:00+00:00");
    }

    #[test]
    fn deserializes_copy_format_columns() {
        let a: ReportCopyColumn = serde_json::from_str(r#"{"kind":"ai","id":"a","label":"L","prompt":"P"}"#).unwrap();
        assert_eq!(a, ReportCopyColumn::Ai { id: "a".into(), label: "L".into(), prompt: "P".into() });
        let b: ReportCopyColumn = serde_json::from_str(r#"{"kind":"blank","id":"b"}"#).unwrap();
        assert_eq!(b, ReportCopyColumn::Blank { id: "b".into(), label: None });
        let f: ReportCopyColumn =
            serde_json::from_str(r#"{"kind":"field","id":"f","field":"durationMinutes","durationFormat":"decimal-with-unit"}"#).unwrap();
        assert_eq!(
            f,
            ReportCopyColumn::Field {
                id: "f".into(),
                field: ReportCopyField::DurationMinutes,
                label: None,
                duration_format: Some(ReportDurationFormat::DecimalWithUnit)
            }
        );
    }
}
