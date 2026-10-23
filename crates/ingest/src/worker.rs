//! Python belge worker'ı ile NDJSON protokolü üzerinden konuşan istemci.
//!
//! Güvenlik sınırı: worker'a kasa anahtarı, veritabanı veya klasör yolu
//! verilmez; yalnız iş için gereken belge baytları gönderilir (ADR-03).
//! Yanıt boyutu, çerçeve boyutu ve süre sınırlıdır; süreç ölürse çağıran
//! taraf hata alır, yarım sonuç tamamlanmış sayılmaz.

// Süreç başlatma bilinçli olarak yalnız burada ve `crates/cli-adapters` içinde
// yapılır: ADR-05 gereği worker/CLI denetimli zarf olmadan çalıştırılmaz.
#![allow(clippy::disallowed_methods)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("worker başlatılamadı: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("worker ile iletişim koptu")]
    Disconnected,
    #[error("worker zaman aşımına uğradı")]
    Timeout,
    #[error("worker çerçevesi 1 MiB sınırını aşıyor")]
    FrameTooLarge,
    #[error("worker protokol hatası: {0}")]
    Protocol(String),
    #[error("worker hatası: {code} — {message}")]
    Remote {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("yanıt şeması beklenen biçimde değil")]
    BadResponse,
}

impl WorkerError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Spawn(_) => "E_WORKER_SPAWN",
            Self::Disconnected => "E_WORKER_DISCONNECTED",
            Self::Timeout => "E_WORKER_TIMEOUT",
            Self::FrameTooLarge => "E_WORKER_FRAME",
            Self::Protocol(_) => "E_WORKER_PROTOCOL",
            Self::Remote { .. } => "E_WORKER_REMOTE",
            Self::BadResponse => "E_WORKER_RESPONSE",
        }
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout
                | Self::Disconnected
                | Self::Remote {
                    retryable: true,
                    ..
                }
                | Self::Spawn(_)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedPage {
    pub index: u32,
    pub text: String,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionHit {
    pub page: u32,
    pub term: String,
    pub rect: [f64; 4],
}

#[derive(Debug, Clone)]
pub struct RedactionOutcome {
    pub pdf: Vec<u8>,
    pub hits: Vec<RedactionHit>,
    pub remaining: Vec<String>,
    pub engine: String,
    pub engine_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extraction {
    pub engine: String,
    pub engine_version: String,
    pub pages: Vec<ExtractedPage>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Worker süreci. `Drop` süreci kapatır; yarım kalan iş yok sayılır.
pub struct WorkerClient {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Result<Vec<u8>, String>>,
    next_id: u64,
    timeout: Duration,
}

impl std::fmt::Debug for WorkerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerClient")
            .field("next_id", &self.next_id)
            .finish()
    }
}

impl WorkerClient {
    /// Geliştirme modunda `python3 -m dim_worker.worker`. `cwd`, modülün
    /// bulunduğu klasördür; paketlenmiş sürümde gömülü Python yolu verilir.
    pub fn spawn_dev(python: &Path, working_dir: &Path) -> Result<Self, WorkerError> {
        Self::spawn(python, &["-m", "dim_worker.worker"], working_dir)
    }

    pub fn spawn(program: &Path, args: &[&str], working_dir: &Path) -> Result<Self, WorkerError> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(WorkerError::Spawn)?;

        let stdin = child.stdin.take().ok_or(WorkerError::Disconnected)?;
        let stdout = child.stdout.take().ok_or(WorkerError::Disconnected)?;
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if buf.len() > MAX_FRAME_BYTES {
                            let _ = tx.send(Err("frame_too_large".to_string()));
                            break;
                        }
                        if tx.send(Ok(buf.clone())).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        break;
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            rx,
            next_id: 0,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, WorkerError> {
        self.next_id += 1;
        let id = format!("req-{}", self.next_id);
        let frame = json!({"v": 1, "id": id, "method": method, "params": params});
        let mut line = serde_json::to_vec(&frame).map_err(|_| WorkerError::BadResponse)?;
        line.push(b'\n');
        self.stdin
            .write_all(&line)
            .map_err(|_| WorkerError::Disconnected)?;
        self.stdin.flush().map_err(|_| WorkerError::Disconnected)?;

        // Yanıtlar sıralıdır; id eşleşmeyen eski çerçeveler atlanır.
        for _ in 0..8 {
            let raw = match self.rx.recv_timeout(self.timeout) {
                Ok(Ok(bytes)) => bytes,
                Ok(Err(_)) => return Err(WorkerError::Disconnected),
                Err(RecvTimeoutError::Timeout) => return Err(WorkerError::Timeout),
                Err(RecvTimeoutError::Disconnected) => return Err(WorkerError::Disconnected),
            };
            let value: Value =
                serde_json::from_slice(&raw).map_err(|_| WorkerError::BadResponse)?;
            if value.get("id").and_then(Value::as_str) != Some(id.as_str()) {
                continue;
            }
            if value.get("ok").and_then(Value::as_bool) == Some(true) {
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            let error = value.get("error").cloned().unwrap_or(Value::Null);
            return Err(WorkerError::Remote {
                code: error
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("E_WORKER_REMOTE")
                    .to_string(),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("worker hatası")
                    .to_string(),
                retryable: error
                    .get("retryable")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            });
        }
        Err(WorkerError::Protocol("yanıt kimliği eşleşmedi".into()))
    }

    pub fn ping(&mut self) -> Result<(), WorkerError> {
        self.request("ping", json!({}))?;
        Ok(())
    }

    pub fn extract_text(
        &mut self,
        data: &[u8],
        mime: &str,
        ocr: bool,
        ocr_lang: &str,
    ) -> Result<Extraction, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let result = self.request(
            "extract_text",
            json!({
                "contentBase64": encoded,
                "mime": mime,
                "ocr": ocr,
                "ocrLang": ocr_lang,
            }),
        )?;
        serde_json::from_value(result).map_err(|_| WorkerError::BadResponse)
    }

    /// Terimleri PDF'ten gerçekten çıkarır. `remaining` boş değilse çağıran
    /// taraf çıktıyı kullanmamalıdır; bu kontrol worker'ın kendi doğrulamasıdır.
    pub fn redact(
        &mut self,
        data: &[u8],
        terms: &[String],
    ) -> Result<RedactionOutcome, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let result = self.request("redact", json!({"contentBase64": encoded, "terms": terms}))?;
        let pdf_b64 = result
            .get("pdfBase64")
            .and_then(Value::as_str)
            .ok_or(WorkerError::BadResponse)?;
        let pdf = base64::engine::general_purpose::STANDARD
            .decode(pdf_b64)
            .map_err(|_| WorkerError::BadResponse)?;
        let hits: Vec<RedactionHit> =
            serde_json::from_value(result.get("hits").cloned().unwrap_or_else(|| json!([])))
                .map_err(|_| WorkerError::BadResponse)?;
        let remaining: Vec<String> = serde_json::from_value(
            result
                .get("remaining")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|_| WorkerError::BadResponse)?;
        Ok(RedactionOutcome {
            pdf,
            hits,
            remaining,
            engine: result
                .get("engine")
                .and_then(Value::as_str)
                .unwrap_or("pymupdf")
                .to_string(),
            engine_version: result
                .get("engineVersion")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
        })
    }

    pub fn shutdown(mut self) {
        let _ = self.request("shutdown", json!({}));
        let _ = self.child.wait();
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Geliştirme ortamında worker klasörünü bulur: depo kökündeki
/// `workers/document`. Paketlenmiş sürümde çağıran açık yol verir.
#[must_use]
pub fn dev_worker_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DIM_WORKER_DIR") {
        return Some(PathBuf::from(dir));
    }
    let cwd = std::env::current_dir().ok()?;
    let mut current = cwd.as_path();
    loop {
        let candidate = current.join("workers").join("document");
        if candidate.join("dim_worker").is_dir() {
            return Some(candidate);
        }
        current = current.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn python() -> Option<PathBuf> {
        for candidate in ["python3", "python"] {
            if let Ok(out) = Command::new(candidate).arg("--version").output() {
                if out.status.success() {
                    return Some(PathBuf::from(candidate));
                }
            }
        }
        None
    }

    #[test]
    fn ping_roundtrip_when_python_available() {
        let (Some(py), Some(dir)) = (python(), dev_worker_dir()) else {
            eprintln!("python veya workers/document bulunamadı; test atlandı");
            return;
        };
        let mut client = WorkerClient::spawn_dev(&py, &dir).expect("başlat");
        client.ping().expect("ping");
        client.shutdown();
    }

    #[test]
    fn unknown_method_returns_remote_error() {
        let (Some(py), Some(dir)) = (python(), dev_worker_dir()) else {
            return;
        };
        let mut client = WorkerClient::spawn_dev(&py, &dir).expect("başlat");
        let err = client.request("yok_boyle", json!({})).unwrap_err();
        assert!(matches!(err, WorkerError::Remote { .. }));
        assert_eq!(err.code(), "E_WORKER_REMOTE");
    }

    #[test]
    fn missing_pdf_engine_is_reported_not_panicked() {
        let (Some(py), Some(dir)) = (python(), dev_worker_dir()) else {
            return;
        };
        let mut client = WorkerClient::spawn_dev(&py, &dir).expect("başlat");
        let err = client
            .request(
                "extract_text",
                json!({"contentBase64": "JVBERi0xLjQK", "mime": "application/pdf"}),
            )
            .unwrap_err();
        // PyMuPDF kuruluysa E_PDF_OPEN, değilse E_ENGINE_MISSING döner.
        match err {
            WorkerError::Remote { code, .. } => {
                assert!(
                    code == "E_ENGINE_MISSING"
                        || code == "E_PDF_OPEN"
                        || code == "E_INPUT_TOO_LARGE",
                    "{code}"
                );
            }
            other => panic!("beklenmeyen: {other:?}"),
        }
    }
}
