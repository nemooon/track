/// Incremental parser for a text/event-stream body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Default)]
pub struct SseParser {
    buf: String,
    event: Option<String>,
    data: Vec<String>,
}

impl SseParser {
    pub fn push(&mut self, chunk: &str) -> Vec<SseEvent> {
        self.buf.push_str(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=pos).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                if !self.data.is_empty() || self.event.is_some() {
                    out.push(SseEvent {
                        event: self.event.take().unwrap_or_else(|| "message".into()),
                        data: std::mem::take(&mut self.data).join("\n"),
                    });
                }
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = match line.split_once(':') {
                Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
                None => (line, ""),
            };
            match field {
                "event" => self.event = Some(value.to_string()),
                "data" => self.data.push(value.to_string()),
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parser_handles_partial_chunks() {
        let mut p = SseParser::default();
        assert!(p.push("event: entries.changed\nda").is_empty());
        let ev = p.push("ta: {\"x\":1}\n\n");
        assert_eq!(ev, vec![SseEvent { event: "entries.changed".into(), data: "{\"x\":1}".into() }]);
    }

    #[test]
    fn sse_parser_defaults_event_to_message_and_ignores_comments() {
        let mut p = SseParser::default();
        let ev = p.push(": keepalive\nretry: 1000\ndata: a\ndata: b\n\n");
        assert_eq!(ev, vec![SseEvent { event: "message".into(), data: "a\nb".into() }]);
    }
}
