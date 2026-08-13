//! Dedicated bounded writer for daily UTC JSONL snapshot files.

use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use super::record::JsonlRecord;

const DEFAULT_QUEUE_CAPACITY: usize = 4096;
const ENQUEUE_TIMEOUT: Duration = Duration::from_millis(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
enum Command {
    Record(JsonlRecord),
    Shutdown(mpsc::SyncSender<Result<(), String>>),
}

/// Cloneable request-path handle for the startup-owned HTTP JSONL writer.
#[derive(Clone)]
pub struct HttpJsonlWriter {
    tx: mpsc::SyncSender<Command>,
    unhealthy: Arc<AtomicBool>,
    drop_reported: Arc<AtomicBool>,
}

impl HttpJsonlWriter {
    /// Creates the owner-only directory and today's file before starting the writer thread.
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        Self::new_with_capacity(directory, DEFAULT_QUEUE_CAPACITY)
    }

    fn new_with_capacity(directory: PathBuf, capacity: usize) -> Result<Self, String> {
        if capacity == 0 {
            return Err("HTTP JSONL queue capacity must be greater than zero".to_owned());
        }
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)
            .map_err(|error| format!("failed to create HTTP JSONL directory: {error}"))?;
        let date = utc_date_string();
        drop(open_file(&directory, &date)?);

        let (tx, rx) = mpsc::sync_channel(capacity);
        let unhealthy = Arc::new(AtomicBool::new(false));
        let drop_reported = Arc::new(AtomicBool::new(false));
        let thread_unhealthy = Arc::clone(&unhealthy);
        thread::Builder::new()
            .name("openbridge-http-jsonl".to_owned())
            .spawn(move || writer_loop(directory, rx, thread_unhealthy))
            .map_err(|error| format!("failed to spawn HTTP JSONL writer thread: {error}"))?;
        Ok(Self {
            tx,
            unhealthy,
            drop_reported,
        })
    }

    /// Enqueues one owned snapshot with a short bounded wait.
    pub(crate) fn try_enqueue(&self, record: JsonlRecord) -> bool {
        if self.unhealthy.load(Ordering::Acquire) {
            return false;
        }
        let deadline = Instant::now() + ENQUEUE_TIMEOUT;
        let mut command = Command::Record(record);
        loop {
            match self.tx.try_send(command) {
                Ok(()) => return true,
                Err(mpsc::TrySendError::Full(returned)) if Instant::now() < deadline => {
                    command = returned;
                    thread::yield_now();
                }
                Err(_) => {
                    if !self.drop_reported.swap(true, Ordering::AcqRel) {
                        tracing::warn!("http_jsonl_snapshot_dropped");
                    }
                    return false;
                }
            }
        }
    }

    /// Requests FIFO drain and flush, returning an error if acknowledgement is not received in time.
    pub fn shutdown(self) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        let mut command = Command::Shutdown(ack_tx);
        loop {
            match self.tx.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(returned)) if Instant::now() < deadline => {
                    command = returned;
                    thread::yield_now();
                }
                Err(mpsc::TrySendError::Full(_)) => {
                    return Err("HTTP JSONL shutdown queue timeout".to_owned());
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("HTTP JSONL writer stopped before shutdown".to_owned());
                }
            }
        }
        ack_rx
            .recv_timeout(SHUTDOWN_TIMEOUT)
            .map_err(|_| "HTTP JSONL shutdown acknowledgement timeout".to_owned())?
    }
}

fn writer_loop(directory: PathBuf, rx: mpsc::Receiver<Command>, unhealthy: Arc<AtomicBool>) {
    let mut date = utc_date_string();
    let mut writer = match open_writer(&directory, &date) {
        Ok(writer) => writer,
        Err(error) => {
            unhealthy.store(true, Ordering::Release);
            tracing::error!(%error, "http_jsonl_writer_failed");
            return;
        }
    };

    while let Ok(command) = rx.recv() {
        match command {
            Command::Record(record) => {
                let today = utc_date_string();
                if today != date {
                    match open_writer(&directory, &today) {
                        Ok(next) => {
                            writer = next;
                            date = today;
                        }
                        Err(error) => {
                            unhealthy.store(true, Ordering::Release);
                            tracing::warn!(%error, "http_jsonl_roll_failed");
                            continue;
                        }
                    }
                }
                if let Err(error) = writer
                    .write_all(&record.to_jsonl_line())
                    .and_then(|()| writer.flush())
                {
                    unhealthy.store(true, Ordering::Release);
                    tracing::warn!(%error, "http_jsonl_write_failed");
                }
            }
            Command::Shutdown(ack) => {
                let result = writer.flush().map_err(|error| error.to_string());
                let _ = ack.send(result);
                return;
            }
        }
    }
    let _ = writer.flush();
}

fn open_file(directory: &Path, date: &str) -> Result<File, String> {
    let path = directory.join(format!("http-{date}.jsonl"));
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("failed to open HTTP JSONL file {}: {error}", path.display()))
}

fn open_writer(directory: &Path, date: &str) -> Result<BufWriter<File>, String> {
    open_file(directory, date).map(BufWriter::new)
}

fn utc_date_string() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        now.month() as u8,
        now.day()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{HeaderMap, HeaderValue};

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openbridge-http-jsonl-{}-{}",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn writes_parseable_redacted_records_and_drains_on_shutdown() {
        let directory = temp_dir();
        let writer = HttpJsonlWriter::new_with_capacity(directory.clone(), 8).unwrap();
        let mut headers = HeaderMap::new();
        headers.append("x-test", HeaderValue::from_static("first"));
        headers.append("x-test", HeaderValue::from_static("second"));
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer synthetic-secret"),
        );
        assert!(writer.try_enqueue(JsonlRecord::request_headers(
            "request-1",
            "POST",
            "/v1/chat/completions",
            &headers,
        )));
        assert!(writer.try_enqueue(JsonlRecord::request_body(
            "request-1",
            b"line 1\nline 2",
            13,
            true,
            false,
        )));
        writer.shutdown().unwrap();

        let path = directory.join(format!("http-{}.jsonl", utc_date_string()));
        let text = fs::read_to_string(path).unwrap();
        let rows = text
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["kind"], "request_headers");
        assert_eq!(rows[0]["headers"].as_array().unwrap().len(), 3);
        assert!(!text.contains("synthetic-secret"));
        assert_eq!(rows[1]["kind"], "request_body");
        assert_eq!(rows[1]["body_text"], "line 1\nline 2");
        fs::remove_dir_all(directory).unwrap();
    }
}
