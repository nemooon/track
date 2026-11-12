use super::ndjson::{AiStreamEvent, NdjsonParser};
use futures::StreamExt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    pub status: Option<u16>,
    pub message: String,
}

impl ApiError {
    pub fn from_body(status: u16, body: &str) -> Self {
        let from_json = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("error").or_else(|| v.get("message")).and_then(|m| m.as_str()).map(str::to_string));
        let message = match from_json {
            Some(m) => m,
            None if !body.trim().is_empty() => body.trim().to_string(),
            None => format!("HTTP {status}"),
        };
        Self { status: Some(status), message }
    }

}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Decodes a byte stream as UTF-8, holding back an incomplete trailing
/// multibyte sequence until the next chunk arrives.
#[derive(Default)]
pub struct Utf8Chunks {
    pending: Vec<u8>,
}

impl Utf8Chunks {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&self.pending) {
            Ok(_) => self.pending.len(),
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => {
                let s = String::from_utf8_lossy(&self.pending).into_owned();
                self.pending.clear();
                return s;
            }
        };
        let rest = self.pending.split_off(valid);
        
        String::from_utf8(std::mem::replace(&mut self.pending, rest)).unwrap_or_default()
    }
}

/// Temporary: AI generation still streams from the Bun server (`/api/ai/generate-stream`)
/// until track-core has its own providers. Runs on a dedicated tokio runtime.
#[derive(Clone)]
pub struct ApiClient {
    base: Arc<str>,
    http: reqwest::Client,
    rt: Arc<tokio::runtime::Runtime>,
}

impl ApiClient {
    pub fn new(base: String) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");
        Self { base: base.into(), http: reqwest::Client::new(), rt: Arc::new(rt) }
    }

    /// `http://127.0.0.1:{TRACK_PORT|8787}/api`
    pub fn from_env() -> Self {
        let port = std::env::var("TRACK_PORT").ok().filter(|p| !p.is_empty()).unwrap_or_else(|| "8787".into());
        Self::new(format!("http://127.0.0.1:{port}/api"))
    }

    /// POST `/ai/generate-stream`, yielding NDJSON events. A transport
    /// failure is reported as a final `AiStreamEvent::Error`.
    pub fn generate_stream(&self, mode: &str, input: String) -> async_channel::Receiver<AiStreamEvent> {
        let (tx, rx) = async_channel::unbounded();
        let req = self.http.post(format!("{}/ai/generate-stream", self.base)).json(&serde_json::json!({ "mode": mode, "input": input }));
        self.rt.spawn(async move {
            let res = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(AiStreamEvent::Error { message: e.to_string() }).await;
                    return;
                }
            };
            let status = res.status().as_u16();
            if !(200..300).contains(&status) {
                let body = res.text().await.unwrap_or_default();
                let _ = tx.send(AiStreamEvent::Error { message: ApiError::from_body(status, &body).message }).await;
                return;
            }
            let mut parser = NdjsonParser::default();
            let mut utf8 = Utf8Chunks::default();
            let mut stream = res.bytes_stream();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        for ev in parser.push(&utf8.push(&bytes)) {
                            if tx.send(ev).await.is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AiStreamEvent::Error { message: e.to_string() }).await;
                        return;
                    }
                }
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_reads_error_key() {
        assert_eq!(ApiError::from_body(400, r#"{"error":"entry_must_be_same_day"}"#).message, "entry_must_be_same_day");
        assert_eq!(ApiError::from_body(500, "boom").message, "boom");
        assert_eq!(ApiError::from_body(400, "").message, "HTTP 400");
    }
}

#[cfg(test)]
mod utf8_tests {
    use super::*;

    #[test]
    fn utf8_chunks_split_inside_multibyte_char() {
        let bytes = "週報".as_bytes();
        let mut d = Utf8Chunks::default();
        assert_eq!(d.push(&bytes[..2]), "");
        assert_eq!(d.push(&bytes[2..4]), "週");
        assert_eq!(d.push(&bytes[4..]), "報");
    }
}
