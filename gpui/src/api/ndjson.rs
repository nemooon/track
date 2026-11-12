use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AiStreamEvent {
    Progress { kind: String, message: String },
    Heartbeat,
    Result { text: String, provider: String },
    Error { message: String },
}

#[derive(Default)]
pub struct NdjsonParser {
    buf: String,
}

impl NdjsonParser {
    pub fn push(&mut self, chunk: &str) -> Vec<AiStreamEvent> {
        self.buf.push_str(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=pos).collect();
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(ev) = serde_json::from_str::<AiStreamEvent>(line) {
                out.push(ev);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_parser_splits_lines() {
        let mut p = NdjsonParser::default();
        let a = p.push("{\"type\":\"progress\",\"kind\":\"status\",\"message\":\"a\"}\n{\"type\":\"res");
        assert_eq!(a, vec![AiStreamEvent::Progress { kind: "status".into(), message: "a".into() }]);
        let b = p.push("ult\",\"text\":\"T\",\"provider\":\"codex\"}\n");
        assert_eq!(b, vec![AiStreamEvent::Result { text: "T".into(), provider: "codex".into() }]);
    }

    #[test]
    fn ndjson_parser_reads_heartbeat_and_error() {
        let mut p = NdjsonParser::default();
        let ev = p.push("{\"type\":\"heartbeat\"}\n\n{\"type\":\"error\",\"message\":\"x\"}\n");
        assert_eq!(ev, vec![AiStreamEvent::Heartbeat, AiStreamEvent::Error { message: "x".into() }]);
    }
}
