use super::ndjson::{AiStreamEvent, NdjsonParser};
use super::sse::{SseEvent, SseParser};
use super::types::*;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

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

    fn network(e: impl std::fmt::Display) -> Self {
        Self { status: None, message: e.to_string() }
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
        let s = String::from_utf8(std::mem::replace(&mut self.pending, rest)).unwrap_or_default();
        s
    }
}

/// HTTP client for the local Bun API. All requests run on a dedicated tokio
/// runtime; the returned futures can be awaited from GPUI's executor.
#[derive(Clone)]
pub struct ApiClient {
    base: Arc<str>,
    http: reqwest::Client,
    rt: Arc<tokio::runtime::Runtime>,
}

pub type ApiResult<T> = Result<T, ApiError>;

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

    fn spawn<T: Send + 'static>(&self, fut: impl Future<Output = ApiResult<T>> + Send + 'static) -> impl Future<Output = ApiResult<T>> + Send + 'static {
        let handle = self.rt.spawn(fut);
        async move { handle.await.map_err(ApiError::network)? }
    }

    fn request<T: DeserializeOwned + Send + 'static>(&self, req: reqwest::RequestBuilder) -> impl Future<Output = ApiResult<T>> + Send + 'static {
        self.spawn(async move {
            let res = req.send().await.map_err(ApiError::network)?;
            let status = res.status().as_u16();
            let body = res.text().await.map_err(ApiError::network)?;
            if !(200..300).contains(&status) {
                return Err(ApiError::from_body(status, &body));
            }
            serde_json::from_str(&body).map_err(|e| ApiError { status: Some(status), message: format!("invalid response: {e}") })
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn iso(d: DateTime<Utc>) -> String {
        d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }

    pub fn entries(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> impl Future<Output = ApiResult<Vec<TimeEntry>>> + Send + 'static {
        self.request(self.http.get(self.url("/entries")).query(&[("from", Self::iso(from)), ("to", Self::iso(to))]))
    }

    pub fn entry_titles(&self, project_id: Option<&str>) -> impl Future<Output = ApiResult<Vec<String>>> + Send + 'static {
        self.request(self.http.get(self.url("/entries/titles")).query(&[("projectId", project_id.unwrap_or("none"))]))
    }

    pub fn create_entry(&self, body: NewEntry) -> impl Future<Output = ApiResult<TimeEntry>> + Send + 'static {
        self.request(self.http.post(self.url("/entries")).json(&body))
    }

    pub fn update_entry(&self, id: &str, patch: EntryPatch) -> impl Future<Output = ApiResult<TimeEntry>> + Send + 'static {
        self.request(self.http.patch(self.url(&format!("/entries/{id}"))).json(&patch))
    }

    pub fn delete_entry(&self, id: &str) -> impl Future<Output = ApiResult<serde_json::Value>> + Send + 'static {
        self.request(self.http.delete(self.url(&format!("/entries/{id}"))))
    }

    pub fn projects(&self) -> impl Future<Output = ApiResult<Vec<Project>>> + Send + 'static {
        self.request(self.http.get(self.url("/projects")))
    }

    pub fn clients(&self) -> impl Future<Output = ApiResult<Vec<Client>>> + Send + 'static {
        self.request(self.http.get(self.url("/clients")))
    }

    pub fn tags(&self) -> impl Future<Output = ApiResult<Vec<Tag>>> + Send + 'static {
        self.request(self.http.get(self.url("/tags")))
    }

    pub fn settings(&self) -> impl Future<Output = ApiResult<Settings>> + Send + 'static {
        self.request(self.http.get(self.url("/settings")))
    }

    pub fn config(&self) -> impl Future<Output = ApiResult<AppConfig>> + Send + 'static {
        self.request(self.http.get(self.url("/config")))
    }

    pub fn report(&self, q: &ReportQuery) -> impl Future<Output = ApiResult<ReportResponse>> + Send + 'static {
        self.request(self.http.get(self.url("/reports")).query(&q.params(true)))
    }

    pub fn report_entries(&self, q: &ReportQuery) -> impl Future<Output = ApiResult<ReportEntriesResponse>> + Send + 'static {
        self.request(self.http.get(self.url("/reports/entries")).query(&q.params(false)))
    }

    /// Subscribes to `/events` (SSE). Reconnects 1s after any disconnect.
    /// The stream ends when the receiver is dropped.
    pub fn subscribe_events(&self) -> async_channel::Receiver<SseEvent> {
        let (tx, rx) = async_channel::unbounded();
        let http = self.http.clone();
        let url = self.url("/events");
        self.rt.spawn(async move {
            loop {
                if tx.is_closed() {
                    return;
                }
                if let Ok(res) = http.get(&url).send().await {
                    if res.status().is_success() {
                        let mut parser = SseParser::default();
                        let mut utf8 = Utf8Chunks::default();
                        let mut stream = res.bytes_stream();
                        while let Some(Ok(chunk)) = stream.next().await {
                            for ev in parser.push(&utf8.push(&chunk)) {
                                if tx.send(ev).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        rx
    }

    /// POST `/ai/generate-stream`, yielding NDJSON events. A transport
    /// failure is reported as a final `AiStreamEvent::Error`.
    pub fn generate_stream(&self, mode: &str, input: String) -> async_channel::Receiver<AiStreamEvent> {
        let (tx, rx) = async_channel::unbounded();
        let req = self.http.post(self.url("/ai/generate-stream")).json(&serde_json::json!({ "mode": mode, "input": input }));
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

/// Runs against a live API: `cargo test live_api -- --ignored`
#[cfg(test)]
mod live_tests {
    use super::*;
    use chrono::{Duration as CDuration, Local, TimeZone};

    #[test]
    #[ignore]
    fn live_api_deserializes_all_endpoints() {
        let api = ApiClient::from_env();
        fn b<F: std::future::Future>(f: F) -> F::Output { futures::executor::block_on(f) }
        let now = Local::now();
        let from = Local.from_local_datetime(&(now.date_naive() - CDuration::days(30)).and_hms_opt(0, 0, 0).unwrap()).unwrap();
        let entries = b(api.entries(from.to_utc(), now.to_utc())).unwrap();
        let projects = b(api.projects()).unwrap();
        b(api.clients()).unwrap();
        b(api.tags()).unwrap();
        let settings = b(api.settings()).unwrap();
        b(api.config()).unwrap();
        b(api.entry_titles(None)).unwrap();
        let q = ReportQuery {
            range: ReportRange::Month,
            anchor: now.to_utc(),
            group_by: GroupBy::Tag,
            client_ids: vec![],
            project_ids: vec![],
            tag_ids: vec![],
        };
        let report = b(api.report(&q)).unwrap();
        let re = b(api.report_entries(&q)).unwrap();
        println!(
            "entries={} projects={} formats={} rows={} reportEntries={}",
            entries.len(), projects.len(), settings.report_copy_formats.len(), report.rows.len(), re.entries.len()
        );
        assert!(!projects.is_empty());
    }
}

/// Mutates the live (copy) DB: `cargo test live_crud -- --ignored`
#[cfg(test)]
mod live_crud_tests {
    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    #[ignore]
    fn live_crud_roundtrip() {
        fn b<F: std::future::Future>(f: F) -> F::Output { futures::executor::block_on(f) }
        let api = ApiClient::from_env();
        let t = |h, m| Local.with_ymd_and_hms(2030, 1, 7, h, m, 0).unwrap().to_utc();
        let tag = b(api.tags()).unwrap().into_iter().next();
        let created = b(api.create_entry(NewEntry {
            start: t(9, 0), end: t(10, 0), title: Some("gpui live test".into()),
            tag_ids: tag.iter().map(|t| t.id.clone()).collect(), break_minutes: 15, ..Default::default()
        })).unwrap();
        assert_eq!(created.break_minutes, 15);
        let moved = b(api.update_entry(&created.id, EntryPatch { start: Some(t(9, 30)), end: Some(t(10, 30)), ..Default::default() })).unwrap();
        assert_eq!(moved.start, t(9, 30));
        let cleared = b(api.update_entry(&created.id, EntryPatch { title: Some(None), note: Some(Some("memo".into())), project_id: Some(None), tag_ids: Some(vec![]), ..Default::default() })).unwrap();
        assert_eq!((cleared.title, cleared.note.as_deref(), cleared.tags.len()), (None, Some("memo"), 0));
        let err = b(api.update_entry(&created.id, EntryPatch { end: Some(t(9, 0) + chrono::Duration::days(2)), ..Default::default() })).unwrap_err();
        println!("cross-day error: {}", err.message);
        b(api.delete_entry(&created.id)).unwrap();
    }
}
