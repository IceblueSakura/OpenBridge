//! Opt-in operator metadata. Never observes bodies, arbitrary headers or identities.
//! Request paths only try-send to a bounded writer; failures cannot alter responses.
use axum::http::HeaderMap;
use serde::Serialize;
use std::{fs::OpenOptions, io::Write, path::Path, sync::mpsc, time::Instant};

const LIMIT: usize = 1 << 20;
#[derive(Clone)]
pub(super) struct Sink(mpsc::SyncSender<Message>);
enum Message {
    Record(Record),
    Flush(tokio::sync::oneshot::Sender<()>),
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Stage {
    Admission,
    Prepare,
    Connect,
    ResponseHead,
    Intake,
    Terminal,
    Projection,
    Delivery,
    Complete,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Outcome {
    Error,
    Interrupted,
    Timeout,
    Shutdown,
    Complete,
}
#[derive(Serialize)]
struct CandidateRecord {
    ordinal: usize,
    stage: Stage,
    upstream_status: Option<u16>,
    retry_after_seconds: Option<u64>,
    received_bytes: u64,
    handed_off_bytes: u64,
    elapsed_ms: u64,
    error_code: Option<&'static str>,
    advanced: bool,
}
#[derive(Serialize)]
struct Record {
    attempt: String,
    stage: Stage,
    outcome: Outcome,
    upstream_status: Option<u16>,
    retry_after_seconds: Option<u64>,
    received_bytes: u64,
    handed_off_bytes: u64,
    elapsed_ms: u64,
    upstream_head_ms: Option<u64>,
    first_upstream_bytes_ms: Option<u64>,
    candidates: Vec<CandidateRecord>,
}
impl Sink {
    pub(super) fn open(path: &Path) -> std::io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        let (tx, rx) = mpsc::sync_channel(64);
        std::thread::Builder::new()
            .name("probe-metadata".into())
            .spawn(move || {
                let mut used = 0;
                for message in rx {
                    match message {
                        Message::Record(record) => {
                            let Ok(mut bytes) = serde_json::to_vec(&record) else {
                                continue;
                            };
                            bytes.push(b'\n');
                            if used + bytes.len() > LIMIT {
                                continue;
                            }
                            if file.write_all(&bytes).is_err() {
                                break;
                            }
                            used += bytes.len();
                        }
                        Message::Flush(done) => {
                            let _ = file.flush();
                            let _ = done.send(());
                        }
                    }
                }
            })?;
        Ok(Self(tx))
    }
    pub(super) async fn flush(&self) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self.0.try_send(Message::Flush(tx)).is_ok() {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), rx).await;
        }
    }
}

pub(super) struct Trace {
    sink: Option<Sink>,
    record: Record,
    start: Instant,
    candidate_start: Option<(usize, Instant, u64, u64)>,
}
impl Trace {
    pub(super) fn new(sink: Option<&Sink>, headers: &HeaderMap) -> Self {
        let mut ids = headers.get_all("x-openbridge-probe-id").iter();
        let id = ids
            .next()
            .and_then(|v| v.to_str().ok())
            .filter(|v| {
                let Some((run, seq)) = v.split_once(':') else {
                    return false;
                };
                run.len() == 32
                    && run
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    && !seq.is_empty()
                    && seq.len() <= 6
                    && !seq.starts_with('0')
                    && seq.bytes().all(|b| b.is_ascii_digit())
            })
            .filter(|_| ids.next().is_none());
        Self {
            sink: sink.filter(|_| id.is_some()).cloned(),
            record: Record {
                attempt: id.unwrap_or_default().into(),
                stage: Stage::Admission,
                outcome: Outcome::Error,
                upstream_status: None,
                retry_after_seconds: None,
                received_bytes: 0,
                handed_off_bytes: 0,
                elapsed_ms: 0,
                upstream_head_ms: None,
                first_upstream_bytes_ms: None,
                candidates: vec![],
            },
            start: Instant::now(),
            candidate_start: None,
        }
    }
    pub(super) fn begin_candidate(&mut self, ordinal: usize) {
        self.record.upstream_status = None;
        self.record.retry_after_seconds = None;
        self.record.upstream_head_ms = None;
        if self.sink.is_some() && ordinal <= crate::topology::route::MAX_ROUTE_CANDIDATES {
            self.candidate_start = Some((
                ordinal,
                Instant::now(),
                self.record.received_bytes,
                self.record.handed_off_bytes,
            ));
        }
    }
    pub(super) fn end_candidate(&mut self, error_code: Option<&'static str>, advanced: bool) {
        if let Some((ordinal, start, received, handed)) = self.candidate_start.take()
            && self.record.candidates.len() < crate::topology::route::MAX_ROUTE_CANDIDATES
        {
            self.record.candidates.push(CandidateRecord {
                ordinal,
                stage: self.record.stage,
                upstream_status: self.record.upstream_status,
                retry_after_seconds: self.record.retry_after_seconds,
                received_bytes: self.record.received_bytes.saturating_sub(received),
                handed_off_bytes: self.record.handed_off_bytes.saturating_sub(handed),
                elapsed_ms: start.elapsed().as_millis().min(u64::MAX as u128) as u64,
                error_code,
                advanced,
            });
        }
    }
    pub(super) fn stage(&mut self, stage: Stage) {
        self.record.stage = stage;
    }
    pub(super) fn outcome(&mut self, outcome: Outcome) {
        self.record.outcome = outcome;
    }
    pub(super) fn head(&mut self, status: u16, headers: &reqwest::header::HeaderMap) {
        self.record.upstream_status = Some(status);
        self.record.upstream_head_ms =
            Some(self.start.elapsed().as_millis().min(u64::MAX as u128) as u64);
        self.record.retry_after_seconds = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty() && v.len() <= 128)
            .and_then(|v| {
                if v.len() <= 5 && v.bytes().all(|b| b.is_ascii_digit()) {
                    v.parse::<u64>().ok()
                } else {
                    httpdate::parse_http_date(v).ok().map(|date| {
                        date.duration_since(std::time::SystemTime::now())
                            .unwrap_or_default()
                            .as_secs()
                    })
                }
            })
            .filter(|n| *n <= 86400);
    }
    pub(super) fn received(&mut self, n: usize) {
        if n > 0 && self.record.first_upstream_bytes_ms.is_none() {
            self.record.first_upstream_bytes_ms =
                Some(self.start.elapsed().as_millis().min(u64::MAX as u128) as u64);
        }
        self.record.received_bytes = self.record.received_bytes.saturating_add(n as u64);
    }
    pub(super) fn handed_off(&mut self, n: usize) {
        self.record.handed_off_bytes = self.record.handed_off_bytes.saturating_add(n as u64);
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        self.end_candidate(Some("interrupted"), false);
        if let Some(sink) = self.sink.take() {
            self.record.elapsed_ms = self.start.elapsed().as_millis().min(u64::MAX as u128) as u64;
            let placeholder = Record {
                attempt: String::new(),
                stage: Stage::Complete,
                outcome: Outcome::Complete,
                upstream_status: None,
                retry_after_seconds: None,
                received_bytes: 0,
                handed_off_bytes: 0,
                elapsed_ms: 0,
                upstream_head_ms: None,
                first_upstream_bytes_ms: None,
                candidates: vec![],
            };
            let _ = sink.0.try_send(Message::Record(std::mem::replace(
                &mut self.record,
                placeholder,
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_observations_do_not_mix_status_bytes_or_unbounded_identity() {
        let (tx, _rx) = mpsc::sync_channel(1);
        let sink = Sink(tx);
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-openbridge-probe-id",
            "00000000000000000000000000000001:1".parse().unwrap(),
        );
        let mut trace = Trace::new(Some(&sink), &headers);
        trace.begin_candidate(1);
        trace.head(429, &HeaderMap::new());
        trace.received(7);
        trace.end_candidate(Some("upstream_rate_limit"), true);
        trace.begin_candidate(2);
        trace.stage(Stage::Connect);
        trace.end_candidate(Some("upstream_timeout"), false);
        assert_eq!(trace.record.candidates[0].upstream_status, Some(429));
        assert_eq!(trace.record.candidates[0].received_bytes, 7);
        assert_eq!(trace.record.candidates[1].upstream_status, None);
        assert_eq!(trace.record.candidates[1].received_bytes, 0);
        for n in 3..=100 {
            trace.begin_candidate(n);
            trace.end_candidate(None, false);
        }
        assert_eq!(trace.record.candidates.len(), 64);
        let wire = serde_json::to_value(&trace.record.candidates[0]).unwrap();
        assert_eq!(wire["ordinal"], 1);
        assert_eq!(wire["advanced"], true);
        assert!(wire.get("endpoint").is_none());
        assert!(wire.get("body").is_none());
    }
    #[test]
    fn full_or_disconnected_sink_does_not_block_a_request() {
        let (tx, rx) = mpsc::sync_channel(1);
        let sink = Sink(tx);
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-openbridge-probe-id",
            "00000000000000000000000000000001:1".parse().unwrap(),
        );
        drop(Trace::new(Some(&sink), &headers));
        let trace = Trace::new(Some(&sink), &headers);
        let (done, completed) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            drop(trace);
            let _ = done.send(());
        });
        completed
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        worker.join().unwrap();
        drop(rx);
        drop(Trace::new(Some(&sink), &headers));
    }
    #[tokio::test]
    async fn metadata_is_closed_bounded_and_never_replaces_existing_files() {
        let dir = std::env::temp_dir().join(format!(
            "openbridge-diag-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let sink = Sink::open(&path).unwrap();
        assert!(Sink::open(&path).is_err());
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer synthetic-secret".parse().unwrap());
        headers.insert(
            "x-openbridge-probe-id",
            "00000000000000000000000000000001:1".parse().unwrap(),
        );
        let mut trace = Trace::new(Some(&sink), &headers);
        let mut upstream = reqwest::header::HeaderMap::new();
        upstream.insert("retry-after", "7".parse().unwrap());
        upstream.insert("x-private", "synthetic-private".parse().unwrap());
        trace.stage(Stage::ResponseHead);
        trace.head(429, &upstream);
        drop(trace);
        headers.insert("x-openbridge-probe-id", "synthetic-secret".parse().unwrap());
        drop(Trace::new(Some(&sink), &headers));
        sink.flush().await;
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(!text.contains("synthetic"));
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(value["upstream_status"], 429);
        assert_eq!(value["retry_after_seconds"], 7);
        assert!(text.len() <= LIMIT);
        drop(sink);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
