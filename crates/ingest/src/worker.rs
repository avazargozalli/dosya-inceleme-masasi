//! Python belge worker'ı ile NDJSON protokolü üzerinden konuşan istemci.
//!
//! Güvenlik sınırı: worker'a kasa anahtarı, veritabanı veya klasör yolu
//! verilmez; yalnız iş için gereken belge baytları gönderilir (ADR-03).
//! Yanıt boyutu, çerçeve boyutu ve süre sınırlıdır; süreç ölürse çağıran
//! taraf hata alır, yarım sonuç tamamlanmış sayılmaz. Metin yanıtı 1 MiB'ı
//! aşarsa worker sayfaları `part`/`parts` ile böler; istemci parçaları sırayla
//! birleştirir, eksik/sırasız parçayı reddeder.

// Süreç başlatma bilinçli olarak yalnız burada ve `crates/cli-adapters` içinde
// yapılır: ADR-05 gereği worker/CLI denetimli zarf olmadan çalıştırılmaz.
#![allow(clippy::disallowed_methods)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// İptal denetim aralığı: bekleme bu dilimlere bölünür, iptal en geç bu kadar
/// sonra fark edilir (şartname §17.5: iptal en geç 250 ms'de "iptal isteniyor").
const CANCEL_POLL: Duration = Duration::from_millis(50);
/// Parçalı yanıtta kabul edilen en fazla çerçeve sayısı; sınırsız döngü yok.
pub const MAX_RESPONSE_PARTS: usize = 5000;
/// Tek istek için kabul edilen en fazla **ilerleme** çerçevesi (F04-T07).
/// Worker en çok `MAX_PAGES = 2000` sayfa bildirir; sınır onun çok üstünde,
/// ama sınırsız değil: sonsuz ilerleme akıtan süreç isteği kilitleyemez.
pub const MAX_PROGRESS_FRAMES: usize = 10_000;
/// Paketlenmiş worker başlatılırken **kaldırılan** ortam değişkenleri: gömülü
/// Python, depodaki kaynak ağacını `PYTHONPATH` üzerinden yüklerse paket
/// kusuru gizlenir.
pub const PYTHON_ENV_VARS: [&str; 4] = [
    "PYTHONPATH",
    "PYTHONHOME",
    "PYTHONSTARTUP",
    "PYTHONEXECUTABLE",
];

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("worker başlatılamadı: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("worker ile iletişim koptu")]
    Disconnected,
    /// Kullanıcı iptal etti; süreç **bizim** isteğimizle durduruldu. Kopma ve
    /// zaman aşımından ayrıdır: otomatik tekrar denenmez (şartname §17.4).
    #[error("ayrıştırma iptal edildi")]
    Canceled,
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
            Self::Canceled => "E_WORKER_CANCELED",
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

/// Worker'ın canlı ilerleme bildirimi (F04-T07): aşama + sayılan/toplam birim.
///
/// İlerleme çerçevesi yanıt **değildir** (`ok` alanı yoktur); belge metni,
/// dosya adı, yol veya kasa bilgisi taşımaz. Aşama adları worker'da tanımlıdır
/// (`start`, `open`, `pages`, `ocr`); bilinmeyen aşama olduğu gibi iletilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseProgress {
    pub stage: String,
    pub done: u32,
    pub total: Option<u32>,
}

/// Çerçevede `progress` varsa çözer. Alan yoksa `None` (normal yanıt).
///
/// Bozuk ilerleme çerçevesi **sessizce yutulmaz**: sayaç okunamıyorsa ya da
/// sayılan toplamı aşıyorsa protokol hatası döner. Aksi hâlde arayüz yanlış
/// bir "5 / 3" göstergesi çizerdi.
fn parse_progress(value: &Value) -> Result<Option<ParseProgress>, WorkerError> {
    let Some(entry) = value.get("progress") else {
        return Ok(None);
    };
    let object = entry
        .as_object()
        .ok_or_else(|| WorkerError::Protocol("ilerleme çerçevesi nesne olmalı".into()))?;
    let stage = object
        .get("stage")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|stage| !stage.is_empty())
        .ok_or_else(|| WorkerError::Protocol("ilerleme aşaması boş".into()))?;
    let done = object
        .get("done")
        .and_then(Value::as_u64)
        .ok_or_else(|| WorkerError::Protocol("ilerleme sayacı okunamadı".into()))?;
    let total = match object.get("total") {
        None | Some(Value::Null) => None,
        Some(raw) => Some(
            raw.as_u64()
                .ok_or_else(|| WorkerError::Protocol("ilerleme toplamı okunamadı".into()))?,
        ),
    };
    if total.is_some_and(|total| done > total) {
        return Err(WorkerError::Protocol(
            "ilerleme sayacı toplamı aşıyor".into(),
        ));
    }
    let done = u32::try_from(done)
        .map_err(|_| WorkerError::Protocol("ilerleme sayacı çok büyük".into()))?;
    let total = match total {
        Some(total) => Some(
            u32::try_from(total)
                .map_err(|_| WorkerError::Protocol("ilerleme toplamı çok büyük".into()))?,
        ),
        None => None,
    };
    Ok(Some(ParseProgress {
        stage: stage.to_string(),
        done,
        total,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedBlock {
    /// Unicode scalar aralığı; `ExtractedPage::text` içine göredir.
    pub start: u32,
    pub end: u32,
    /// Blok kutusu `[x0, y0, x1, y1]`; yapısal motorlarda yoktur.
    #[serde(default)]
    pub bbox: Option<[f64; 4]>,
    /// Satır kutuları; blok kutusunun içindedir.
    #[serde(default)]
    pub lines: Vec<[f64; 4]>,
    /// Yapısal motorlarda paragraf/bölüm yolu; PDF/görüntüde yok.
    #[serde(default)]
    pub path: Option<String>,
    /// `heading`, `paragraph`, `table`, `header`, `footer`, `annotation`.
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedPage {
    pub index: u32,
    pub text: String,
    /// Koordinat uzayının genişliği/yüksekliği (döndürme öncesi).
    pub width: f64,
    pub height: f64,
    /// Gösterim döndürmesi (0/90/180/270); worker yoksa 0.
    #[serde(default)]
    pub rotation: u32,
    /// `unrotated_cropbox_points`, `image_pixels` veya `structure`.
    #[serde(default)]
    pub coordinate_space: Option<String>,
    /// PDF `/PageLabels` ağacından gelen basılı sayfa etiketi (`iv`, `A-12`).
    /// Ağaç tanımlı değilse `None`: fiziksel numaradan etiket türetilmez.
    #[serde(default)]
    pub page_label: Option<String>,
    #[serde(default)]
    pub blocks: Vec<ExtractedBlock>,
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
    /// Sayfa görüntüsü OCR ile de okundu mu? Tesseract yoksa `false`.
    pub ocr_verified: bool,
    /// Çıktıyı doğrulayan motorlar (pymupdf, raw-scan, pypdf, tesseract).
    /// Çalışmayan doğrulayıcı burada **yer almaz**.
    pub engines: Vec<String>,
    /// Bağımsız taramada açılan Flate akışı sayısı.
    pub streams_scanned: u32,
    /// Görüntü/font gibi ikili akış sayısı (ham bayt taraması kapsar).
    pub streams_binary: u32,
    /// Açılamayan filtreli akış sayısı; sıfırdan büyükse uyarı yazılır.
    pub streams_unreadable: u32,
    /// Karartılan terimi taşıdığı için silinen açıklama sayısı.
    pub annotations_removed: u32,
    /// Sayfa kutusunun dışında bulunup kaldırılan metin parçası sayısı.
    pub offpage_spans_removed: u32,
    /// Kaldırılan gömülü JavaScript eylemi sayısı.
    pub scripts_removed: u32,
    /// Doğrulama sınırları: arayüz bunları olduğu gibi gösterir.
    pub warnings: Vec<String>,
}

/// Worker'ın karartma yanıtını çözer. İki komut (`redact`, `redact_regions`)
/// aynı alanları döner; ayrıştırma tek yerde tutulur.
fn parse_redaction(result: &serde_json::Value) -> Result<RedactionOutcome, WorkerError> {
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
    let count = |key: &str| -> u32 {
        result
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0)
    };
    Ok(RedactionOutcome {
        pdf,
        hits,
        remaining,
        ocr_verified: result
            .get("ocrVerified")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        engines: serde_json::from_value(
            result.get("engines").cloned().unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
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
        streams_scanned: count("streamsScanned"),
        streams_binary: count("streamsBinary"),
        streams_unreadable: count("streamsUnreadable"),
        annotations_removed: count("annotationsRemoved"),
        offpage_spans_removed: count("offpageSpansRemoved"),
        scripts_removed: count("scriptsRemoved"),
        warnings: serde_json::from_value(
            result.get("warnings").cloned().unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
    })
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

/// Parçalı `extract_text` yanıtlarını tek çıkarıma birleştirir.
///
/// Worker büyük yanıtı sayfalara bölerek birden çok çerçevede gönderir; sıra
/// ve parça sayısı tutmazsa birleştirme yapılmaz (yarım metin tam sayılmaz).
/// Uyarılar her parçada tekrarlandığı için tekilleştirilir, sayfa sırası
/// korunur.
fn merge_extraction_parts(parts: &[Value]) -> Result<Extraction, WorkerError> {
    let mut pages: Vec<ExtractedPage> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut engine: Option<String> = None;
    let mut engine_version: Option<String> = None;
    let mut declared: Option<usize> = None;

    for (index, part) in parts.iter().enumerate() {
        if let Some(total) = part.get("parts").and_then(Value::as_u64) {
            let total = usize::try_from(total).unwrap_or(usize::MAX);
            match declared {
                None => declared = Some(total),
                Some(existing) if existing != total => {
                    return Err(WorkerError::Protocol("parça sayısı tutarsız".into()));
                }
                Some(_) => {}
            }
            let part_no = part.get("part").and_then(Value::as_u64).unwrap_or(0);
            if part_no != index as u64 + 1 {
                return Err(WorkerError::Protocol("parça sırası bozuk".into()));
            }
        }
        if engine.is_none() {
            engine = part
                .get("engine")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        if engine_version.is_none() {
            engine_version = part
                .get("engineVersion")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        let chunk: Vec<ExtractedPage> = serde_json::from_value(
            part.get("pages")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        )
        .map_err(|_| WorkerError::BadResponse)?;
        pages.extend(chunk);
        if let Some(items) = part.get("warnings").and_then(Value::as_array) {
            for item in items {
                if let Some(text) = item.as_str() {
                    if !warnings.iter().any(|existing| existing == text) {
                        warnings.push(text.to_string());
                    }
                }
            }
        }
    }

    if let Some(total) = declared {
        if total != parts.len() {
            return Err(WorkerError::Protocol("eksik parça".into()));
        }
    }

    Ok(Extraction {
        engine: engine.unwrap_or_else(|| "unknown".to_string()),
        engine_version: engine_version.unwrap_or_else(|| "unknown".to_string()),
        pages,
        warnings,
    })
}

/// Çizilmiş sayfa önizlemesi: PNG baytları + belgenin toplam sayfa sayısı.
#[derive(Debug, Clone)]
pub struct PagePreview {
    pub png: Vec<u8>,
    pub page_count: u32,
}

/// Worker süreci. `Drop` süreci kapatır; yarım kalan iş yok sayılır.
pub struct WorkerClient {
    /// Süreç tutamacı paylaşılır: iptal düğmesi ayrı iş parçacığından
    /// `WorkerKiller` ile süreci durdurabilir (şartname §17.5, F04-T07).
    child: Arc<Mutex<Child>>,
    /// İptal isteği bu istemci için verildi mi? Kopma/zaman aşımı ile gerçek
    /// iptali ayırır: iptal edilen iş otomatik yeniden denenmez.
    cancel: Arc<AtomicBool>,
    stdin: ChildStdin,
    rx: Receiver<Result<Vec<u8>, String>>,
    next_id: u64,
    timeout: Duration,
}

/// Süren worker'ı durdurma tutamağı. `WorkerClient` sahibi onu bir yuvada
/// tutar; iptal komutu kasa kilidini beklemeden süreci öldürebilir.
#[derive(Clone)]
pub struct WorkerKiller {
    child: Arc<Mutex<Child>>,
    cancel: Arc<AtomicBool>,
}

impl std::fmt::Debug for WorkerKiller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerKiller")
            .field("canceled", &self.is_canceled())
            .finish()
    }
}

impl WorkerKiller {
    /// İptal isteğini işaretler ve süreci öldürür (geri dönüşü yok).
    ///
    /// Dönen değer iptali **bu çağrının** başlattığını söyler; ikinci çağrı
    /// `false` döner (zaten iptal edilmiş). Öldürülen süreç hemen toplanır:
    /// geride zombi bırakılmaz.
    pub fn cancel(&self) -> bool {
        let first = !self.cancel.swap(true, Ordering::SeqCst);
        if let Ok(mut child) = self.child.lock() {
            if first {
                let _ = child.kill();
            }
            // Zaten durmuş süreçte `kill()` de başarılı görünür (zombiye
            // sinyal); bu yüzden "durdurdum" iddiası bayrağa dayanır.
            let _ = child.wait();
        }
        first
    }

    #[must_use]
    pub fn is_canceled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
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

    /// Paketlenmiş (PyInstaller onedir) worker ikilisini çalıştırır. Protokol
    /// aynıdır: NDJSON, stdin/stdout. Python kurulu olması gerekmez.
    ///
    /// Python ortam değişkenleri **kaldırılır**: paketlenmiş ikili `PYTHONPATH`/
    /// `PYTHONHOME` devralırsa depodaki kaynak modülleri yükleyebilir ve paket
    /// kusuru "çalışıyor" gibi görünür (F01-T02'nin ölçtüğü risk).
    pub fn spawn_bundled(exe: &Path) -> Result<Self, WorkerError> {
        let dir = exe.parent().unwrap_or_else(|| Path::new("."));
        Self::spawn_with_env(exe, &[], dir, &PYTHON_ENV_VARS)
    }

    fn spawn(program: &Path, args: &[&str], working_dir: &Path) -> Result<Self, WorkerError> {
        Self::spawn_with_env(program, args, working_dir, &[])
    }

    fn spawn_with_env(
        program: &Path,
        args: &[&str],
        working_dir: &Path,
        removed_env: &[&str],
    ) -> Result<Self, WorkerError> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // F01-T07: worker ortamı devralmaz. Kullanıcının kabuk sırları (token,
        // AWS/OPENAI anahtarları, HOME) çocuk sürece geçmez; yalnız çalışması
        // için gereken asgari değişkenler verilir.
        command.env_clear();
        for (name, value) in worker_env() {
            command.env(name, value);
        }
        for name in removed_env {
            command.env_remove(name);
        }
        let mut child = command.spawn().map_err(WorkerError::Spawn)?;

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
            child: Arc::new(Mutex::new(child)),
            cancel: Arc::new(AtomicBool::new(false)),
            stdin,
            rx,
            next_id: 0,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// İptal tutamağı. Aynı worker için birden çok kez alınabilir; tutamak
    /// istemci düştükten sonra da süreci durdurabilir (Arc).
    #[must_use]
    pub fn killer(&self) -> WorkerKiller {
        WorkerKiller {
            child: Arc::clone(&self.child),
            cancel: Arc::clone(&self.cancel),
        }
    }

    /// Bağlantı arızasını sınıflandırır: iptal istediysak kopma **bizim**
    /// işimizdir ve `E_WORKER_CANCELED` döner; aksi hâlde gerçek arıza.
    fn interrupt_error(&self) -> WorkerError {
        if self.cancel.load(Ordering::SeqCst) {
            WorkerError::Canceled
        } else {
            WorkerError::Disconnected
        }
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// İsteği yazar ve kimliğini döndürür. Yanıt okuma ayrı adımdır: parçalı
    /// yanıtta aynı kimlikle birden çok çerçeve gelebilir.
    fn send_request(&mut self, method: &str, params: Value) -> Result<String, WorkerError> {
        self.next_id += 1;
        let id = format!("req-{}", self.next_id);
        let frame = json!({"v": 1, "id": id, "method": method, "params": params});
        let mut line = serde_json::to_vec(&frame).map_err(|_| WorkerError::BadResponse)?;
        line.push(b'\n');
        // İptal edilen süreçte yazma `BrokenPipe` verir; bu bir arıza değil,
        // kullanıcının iptalinin sonucudur.
        if self.stdin.write_all(&line).is_err() {
            return Err(self.interrupt_error());
        }
        if self.stdin.flush().is_err() {
            return Err(self.interrupt_error());
        }
        Ok(id)
    }

    /// Yanıt çerçevesini bekler. Bekleme **dilimlere** bölünür: iptal, okuyucu
    /// iş parçacığı EOF görmese bile (torun süreç boruyu tutabilir) en geç
    /// `CANCEL_POLL` içinde fark edilir. Ubuntu CI'da `sh -c "sleep 30"` vekili
    /// tam bu yüzden 30 sn beklemişti.
    fn recv_frame(&mut self) -> Result<Vec<u8>, WorkerError> {
        let deadline = std::time::Instant::now() + self.timeout;
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(WorkerError::Canceled);
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(WorkerError::Timeout);
            }
            match self.rx.recv_timeout(remaining.min(CANCEL_POLL)) {
                Ok(Ok(bytes)) => return Ok(bytes),
                Ok(Err(reason)) => {
                    // Okuyucu iş parçacığı iki farklı arıza bildirir: çerçeve
                    // sınırı aşımı ve okuma hatası. İkisini "bağlantı koptu"
                    // diye birleştirmek çağıranın ayırt etmesini engellerdi
                    // (aşırı çerçeve protokol/kötü niyet işareti, kopma ise
                    // süreç ölümü); ayrı kod döner.
                    return Err(if reason == "frame_too_large" {
                        WorkerError::FrameTooLarge
                    } else {
                        self.interrupt_error()
                    });
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Err(self.interrupt_error()),
            }
        }
    }

    /// Verilen kimliğe ait ilk yanıtı okur.
    ///
    /// **İlerleme çerçeveleri (F04-T07):** `progress` alanlı satırlar yanıt
    /// değildir; aynı isteğe aitse `progress` geri çağrısına verilir ve
    /// okunmaya devam edilir. Başka isteğin ilerlemesi sayaca karışmaz.
    fn read_response<F: FnMut(ParseProgress)>(
        &mut self,
        id: &str,
        mut progress: F,
    ) -> Result<Value, WorkerError> {
        // Yanıtlar sıralıdır; id eşleşmeyen eski çerçeveler atlanır.
        let mut stale = 0usize;
        let mut progress_frames = 0usize;
        loop {
            let raw = self.recv_frame()?;
            let value: Value =
                serde_json::from_slice(&raw).map_err(|_| WorkerError::BadResponse)?;
            if let Some(update) = parse_progress(&value)? {
                progress_frames += 1;
                if progress_frames > MAX_PROGRESS_FRAMES {
                    return Err(WorkerError::Protocol(
                        "ilerleme çerçevesi sınırı aşıldı".into(),
                    ));
                }
                if value.get("id").and_then(Value::as_str) == Some(id) {
                    progress(update);
                }
                continue;
            }
            if value.get("id").and_then(Value::as_str) != Some(id) {
                stale += 1;
                if stale >= 8 {
                    return Err(WorkerError::Protocol("yanıt kimliği eşleşmedi".into()));
                }
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
    }

    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, WorkerError> {
        let id = self.send_request(method, params)?;
        self.read_response(&id, |_| {})
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
        text_encoding: Option<&str>,
        password: Option<&str>,
    ) -> Result<Extraction, WorkerError> {
        self.extract_text_with_progress(
            data,
            mime,
            ocr,
            ocr_lang,
            text_encoding,
            password,
            &mut |_| {},
        )
    }

    /// `extract_text` + canlı ilerleme (F04-T07). Worker sayfa bittikçe
    /// `progress` çerçevesi yazar; `progress` geri çağrısı **yanıt beklerken**
    /// çağrılır, böylece arayüz sayaç gösterebilir.
    ///
    /// İlerleme çağrısı sonucu değiştirmez; yarım kalan iş yine tamamlanmış
    /// sayılmaz (parça birleştirme kuralları aynı).
    #[allow(clippy::too_many_arguments)]
    pub fn extract_text_with_progress(
        &mut self,
        data: &[u8],
        mime: &str,
        ocr: bool,
        ocr_lang: &str,
        text_encoding: Option<&str>,
        password: Option<&str>,
        progress: &mut dyn FnMut(ParseProgress),
    ) -> Result<Extraction, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let mut payload = json!({
            "contentBase64": encoded,
            "mime": mime,
            "ocr": ocr,
            "ocrLang": ocr_lang,
        });
        // Metin kodlaması yalnız düz metin belgelerde anlamlıdır; `auto`
        // worker'ın BOM/UTF-8/Türkçe tek baytlık seçimini kullanır.
        if let Some(choice) = text_encoding.filter(|value| !value.is_empty()) {
            payload["encoding"] = json!(choice);
        }
        // Parola yalnız bu istek için taşınır; kayda geçmez.
        if let Some(secret) = password.filter(|p| !p.is_empty()) {
            payload["password"] = json!(secret);
        }
        let id = self.send_request("extract_text", payload)?;
        let mut parts: Vec<Value> = Vec::new();
        loop {
            let result = self.read_response(&id, &mut *progress)?;
            let declared = result.get("parts").and_then(Value::as_u64);
            parts.push(result);
            match declared {
                Some(total) if (parts.len() as u64) < total => {
                    if parts.len() > MAX_RESPONSE_PARTS {
                        return Err(WorkerError::Protocol(
                            "yanıt çok fazla parçaya bölündü".into(),
                        ));
                    }
                }
                _ => break,
            }
        }
        merge_extraction_parts(&parts)
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
        parse_redaction(&result)
    }

    /// Bir PDF sayfasını PNG önizleme olarak çizer (F07-T11). Sayfa numarası
    /// fiziksel sıradır; dpi worker'da 48–200 aralığına kırpılır.
    pub fn render_page(
        &mut self,
        data: &[u8],
        page_index: u32,
        dpi: u32,
    ) -> Result<PagePreview, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let result = self.request(
            "render_page",
            json!({"contentBase64": encoded, "pageIndex": page_index, "dpi": dpi}),
        )?;
        let png = result
            .get("pngBase64")
            .and_then(Value::as_str)
            .ok_or(WorkerError::BadResponse)?;
        let png = base64::engine::general_purpose::STANDARD
            .decode(png)
            .map_err(|_| WorkerError::BadResponse)?;
        let page_count = result.get("pageCount").and_then(Value::as_u64).unwrap_or(0);
        Ok(PagePreview {
            png,
            page_count: u32::try_from(page_count).unwrap_or(0),
        })
    }

    /// Worker yetenekleri: motor sürümleri, OCR dilleri, tessdata parmak izi.
    pub fn capabilities(&mut self) -> Result<serde_json::Value, WorkerError> {
        self.request("capabilities", json!({}))
    }

    /// Belge envanteri (yapı, ekler, değişiklik izi). Gövde metni gönderilmez,
    /// yanıt da metin taşımaz; yalnız sayılar ve ek adları döner.
    pub fn inventory(
        &mut self,
        data: &[u8],
        mime: &str,
    ) -> Result<crate::inventory::DocumentInventory, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let result = self.request(
            "inventory",
            json!({ "contentBase64": encoded, "mime": mime }),
        )?;
        crate::inventory::parse_inventory(&result)
    }

    /// DOCX rapor üretir. Girdi çekirdekte hazırlanır; worker DB görmez.
    pub fn render_report(&mut self, payload: &serde_json::Value) -> Result<Vec<u8>, WorkerError> {
        let result = self.request("render_report", json!({"payload": payload}))?;
        let encoded = result
            .get("docxBase64")
            .and_then(Value::as_str)
            .ok_or(WorkerError::BadResponse)?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| WorkerError::BadResponse)
    }

    /// Kullanıcının seçtiği sayfa bölgelerini gerçekten kaldırır.
    pub fn redact_regions(
        &mut self,
        data: &[u8],
        regions: &[(u32, [f64; 4])],
    ) -> Result<RedactionOutcome, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let items: Vec<serde_json::Value> = regions
            .iter()
            .map(|(page, rect)| json!({"page": page, "rect": rect}))
            .collect();
        let result = self.request(
            "redact_regions",
            json!({"contentBase64": encoded, "regions": items}),
        )?;
        parse_redaction(&result)
    }

    /// PDF rapor üretir (HTML kaçışlanmış, Story ile sayfalanmış).
    pub fn render_report_pdf(
        &mut self,
        payload: &serde_json::Value,
    ) -> Result<Vec<u8>, WorkerError> {
        let result = self.request("render_report_pdf", json!({"payload": payload}))?;
        let encoded = result
            .get("pdfBase64")
            .and_then(Value::as_str)
            .ok_or(WorkerError::BadResponse)?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| WorkerError::BadResponse)
    }

    pub fn shutdown(mut self) {
        let _ = self.request("shutdown", json!({}));
        if let Ok(mut child) = self.child.lock() {
            let _ = child.wait();
        }
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Geliştirme ortamında Python yorumlayıcısını bulur (`python3`, sonra `python`).
///
/// Süreç başlatma bu modülde merkezîdir (ADR-05); worker'ı sınayan testler de
/// yorumlayıcıyı bu yardımcıyla arar, kendi `Command` çağrısını yazmaz.
#[must_use]
pub fn dev_python() -> Option<PathBuf> {
    for candidate in ["python3", "python"] {
        if let Ok(out) = Command::new(candidate).arg("--version").output() {
            if out.status.success() {
                return Some(PathBuf::from(candidate));
            }
        }
    }
    None
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

/// Geliştirme/test: verilen Python modülünü (`pymupdf` gibi) içe aktarabilen
/// yorumlayıcıyı bulur.
///
/// Sıra: `DIM_WORKER_PYTHON` → `workers/document/.venv/bin/python` → PATH'teki
/// `python3`/`python`. Modül hiçbirinde yoksa `None` döner; çağıran test
/// "atlandı" yazar — motor yokken "temiz" ya da "bozuk" demek yanıltıcı olur.
/// Süreç başlatma tek yerde tutulur (clippy `disallowed_methods` kapısı).
#[must_use]
pub fn dev_python_with_module(worker_dir: &Path, module: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("DIM_WORKER_PYTHON") {
        candidates.push(PathBuf::from(path));
    }
    let venv = worker_dir.join(".venv").join("bin").join("python");
    if venv.is_file() {
        candidates.push(venv);
    }
    candidates.extend(dev_python());
    candidates.into_iter().find(|candidate| {
        Command::new(candidate)
            .args(["-c", &format!("import {module}")])
            .status()
            .is_ok_and(|status| status.success())
    })
}

/// Worker sürecine geçen **asgari** ortam. `env_clear()` sonrası yalnız bu
/// değişkenler verilir; sırlar ve kullanıcı oturumu devralınmaz.
///
/// Kapsam sınırı (dürüst): ağ erişimi bu katmanda engellenemez — worker
/// yalnız stdin/stdout ile konuşur ve ürün ona dosya yolu değil bayt verir,
/// ancak işletim sistemi düzeyinde ağ yalıtımı yoktur (F01-T07).
pub fn worker_env() -> Vec<(String, std::ffi::OsString)> {
    let mut out = Vec::new();
    for name in ["PATH", "LANG", "LC_ALL"] {
        if let Some(value) = std::env::var_os(name) {
            out.push((name.to_string(), value));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn python() -> Option<PathBuf> {
        dev_python()
    }

    fn extraction_part(index: u64, total: u64, page_index: u64) -> Value {
        json!({
            "engine": "pymupdf",
            "engineVersion": "2.0",
            "warnings": ["ortak uyarı"],
            "part": index,
            "parts": total,
            "pages": [{
                "index": page_index,
                "text": format!("sayfa {page_index}"),
                "width": 595.0,
                "height": 842.0
            }]
        })
    }

    #[test]
    fn merge_parts_keeps_page_order_and_dedupes_warnings() {
        let parts = vec![
            extraction_part(1, 3, 0),
            extraction_part(2, 3, 1),
            extraction_part(3, 3, 2),
        ];
        let extraction = merge_extraction_parts(&parts).expect("birleşim");
        assert_eq!(extraction.engine, "pymupdf");
        assert_eq!(extraction.engine_version, "2.0");
        assert_eq!(extraction.pages.len(), 3);
        assert_eq!(
            extraction
                .pages
                .iter()
                .map(|page| page.index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(extraction.warnings, vec!["ortak uyarı"]);
    }

    #[test]
    fn single_frame_extraction_merges_without_part_fields() {
        let single = json!({
            "engine": "plain",
            "engineVersion": "1",
            "pages": [{"index": 0, "text": "metin", "width": 0.0, "height": 0.0}],
            "warnings": []
        });
        let extraction = merge_extraction_parts(&[single]).expect("tek çerçeve");
        assert_eq!(extraction.pages.len(), 1);
        assert!(extraction.warnings.is_empty());
    }

    #[test]
    fn merge_parts_rejects_missing_or_misordered_parts() {
        let missing = merge_extraction_parts(&[extraction_part(1, 3, 0), extraction_part(2, 3, 1)])
            .unwrap_err();
        assert!(matches!(missing, WorkerError::Protocol(_)));

        let misordered =
            merge_extraction_parts(&[extraction_part(2, 2, 0), extraction_part(1, 2, 1)])
                .unwrap_err();
        assert!(matches!(misordered, WorkerError::Protocol(_)));
    }

    #[test]
    fn redaction_response_parses_verification_counts_and_tolerates_old_workers() {
        let pdf = base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.4");
        let full = json!({
            "pdfBase64": pdf,
            "hits": [{"page": 0, "term": "Ahmet", "rect": [1.0, 2.0, 3.0, 4.0]}],
            "remaining": [],
            "engine": "pymupdf",
            "engineVersion": "1.24.14",
            "ocrVerified": true,
            "engines": ["pymupdf", "raw-scan", "pypdf", "tesseract"],
            "streamsScanned": 4,
            "streamsBinary": 1,
            "streamsUnreadable": 0,
            "annotationsRemoved": 2,
            "offpageSpansRemoved": 1,
            "scriptsRemoved": 3,
            "warnings": ["2 açıklama silindi: karartılan terimi taşıyordu"]
        });
        let outcome = parse_redaction(&full).expect("tam yanıt");
        assert_eq!(outcome.pdf, b"%PDF-1.4");
        assert_eq!(outcome.hits.len(), 1);
        assert!(outcome.ocr_verified);
        assert_eq!(
            outcome.engines,
            vec!["pymupdf", "raw-scan", "pypdf", "tesseract"]
        );
        assert_eq!(outcome.streams_scanned, 4);
        assert_eq!(outcome.streams_binary, 1);
        assert_eq!(outcome.streams_unreadable, 0);
        assert_eq!(outcome.annotations_removed, 2);
        assert_eq!(outcome.offpage_spans_removed, 1);
        assert_eq!(outcome.scripts_removed, 3);
        assert_eq!(outcome.warnings.len(), 1);

        // Bu alanları bilmeyen eski worker: sıfır/boş, hata değil.
        let legacy = json!({"pdfBase64": pdf, "hits": [], "remaining": []});
        let outcome = parse_redaction(&legacy).expect("eski worker");
        assert_eq!(outcome.streams_scanned, 0);
        assert_eq!(outcome.annotations_removed, 0);
        assert!(outcome.warnings.is_empty());
        assert!(outcome.engines.is_empty());
    }

    #[test]
    fn extraction_deserializes_geometry_and_tolerates_old_workers() {
        let payload = json!({
            "engine": "pymupdf",
            "engineVersion": "2.0",
            "pages": [{
                "index": 0,
                "text": "A\n",
                "width": 300.0,
                "height": 400.0,
                "rotation": 90,
                "coordinateSpace": "unrotated_cropbox_points",
                "pageLabel": "iv",
                "blocks": [{
                    "start": 0,
                    "end": 2,
                    "bbox": [1.0, 2.0, 3.0, 4.0],
                    "lines": [[1.0, 2.0, 3.0, 4.0]],
                    "path": "Kira Sözleşmesi · p/1",
                    "kind": "paragraph"
                }],
                "warnings": []
            }],
            "warnings": []
        });
        let extraction: Extraction = serde_json::from_value(payload).expect("şema");
        assert_eq!(extraction.pages[0].rotation, 90);
        assert_eq!(
            extraction.pages[0].coordinate_space.as_deref(),
            Some("unrotated_cropbox_points")
        );
        assert_eq!(
            extraction.pages[0].blocks[0].bbox,
            Some([1.0, 2.0, 3.0, 4.0])
        );
        assert_eq!(extraction.pages[0].blocks[0].lines.len(), 1);
        assert_eq!(
            extraction.pages[0].blocks[0].path.as_deref(),
            Some("Kira Sözleşmesi · p/1")
        );
        assert_eq!(
            extraction.pages[0].blocks[0].kind.as_deref(),
            Some("paragraph")
        );
        assert_eq!(extraction.pages[0].page_label.as_deref(), Some("iv"));

        // Geometri alanlarını göndermeyen eski worker yanıtı da okunur; kutular
        // boş kalır, konum uydurulmaz.
        let legacy = json!({
            "engine": "plain",
            "engineVersion": "1",
            "pages": [{"index": 0, "text": "metin", "width": 0.0, "height": 0.0, "warnings": []}],
            "warnings": []
        });
        let old: Extraction = serde_json::from_value(legacy).expect("eski şema");
        assert_eq!(old.pages[0].rotation, 0);
        assert!(old.pages[0].coordinate_space.is_none());
        assert!(old.pages[0].page_label.is_none());
        assert!(old.pages[0].blocks.is_empty());
    }

    #[test]
    fn text_encoding_auto_detection_roundtrip_when_python_available() {
        let (Some(py), Some(dir)) = (python(), dev_worker_dir()) else {
            eprintln!("python veya workers/document bulunamadı; test atlandı");
            return;
        };
        let mut client = WorkerClient::spawn_dev(&py, &dir).expect("başlat");
        // Windows-1254 Türkçe baytlar: ı = 0xFD, ğ = 0xF0, ş = 0xFE, ç = 0xE7.
        // Bu dizi katı UTF-8'de geçersizdir; otomatik algılama olmadan metin
        // çözülemez (eski davranış E_TEXT_ENCODING ile duruyordu).
        let data = b"Mahkeme itiraz\xfd reddetti \xf0\xfe\xfd\xe7";
        let extraction = client
            .extract_text(data, "text/plain", false, "eng", Some("auto"), None)
            .expect("çıkarım");
        assert_eq!(extraction.engine, "plain");
        assert_eq!(extraction.pages.len(), 1);
        assert_eq!(extraction.pages[0].text, "Mahkeme itirazı reddetti ğşıç");
        assert!(extraction
            .warnings
            .iter()
            .any(|warning| warning.contains("Windows-1254")));

        // Açıkça yanlış kodlama seçilirse bozuk metin üretilmez; açık hata döner.
        let err = client
            .extract_text(data, "text/plain", false, "eng", Some("utf-8"), None)
            .unwrap_err();
        match err {
            WorkerError::Remote { code, .. } => assert_eq!(code, "E_TEXT_ENCODING"),
            other => panic!("beklenmeyen: {other:?}"),
        }
        client.shutdown();
    }

    /// F01-T10 — süreç ölümü, yanıtsız süreç ve aşırı büyük çerçeve **ayrı**
    /// kodlarla ve asılmadan raporlanır. Vekil süreç `/bin/sh` olduğu için bu
    /// üç test Unix'e özgüdür; Windows'ta bu durumlar denenmedi (dürüst sınır).
    #[cfg(unix)]
    #[test]
    fn worker_process_death_is_reported_without_hanging() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(Path::new("/bin/sh"), &["-c", "exit 7"], dir.path())
            .expect("başlat")
            .with_timeout(Duration::from_secs(2));
        let started = std::time::Instant::now();
        let err = client.ping().unwrap_err();
        assert!(
            matches!(err, WorkerError::Disconnected),
            "beklenmeyen hata: {err:?}"
        );
        assert_eq!(err.code(), "E_WORKER_DISCONNECTED");
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "süreç ölümü asılmamalı"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unresponsive_worker_times_out_instead_of_hanging() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(Path::new("/bin/sh"), &["-c", "sleep 30"], dir.path())
            .expect("başlat")
            .with_timeout(Duration::from_millis(500));
        let started = std::time::Instant::now();
        let err = client.ping().unwrap_err();
        assert!(
            matches!(err, WorkerError::Timeout),
            "beklenmeyen hata: {err:?}"
        );
        assert_eq!(err.code(), "E_WORKER_TIMEOUT");
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "zaman aşımı sınırında dönmeli"
        );
    }

    /// 1 MiB çerçeve sınırını aşan tek satır: hata **`E_WORKER_FRAME`** olarak
    /// döner. Bu ayrım olmadan aşırı çerçeve "bağlantı koptu" gibi görünüyordu
    /// ve çağıran süreç ölümü ile protokol ihlalini ayırt edemiyordu.
    #[cfg(unix)]
    #[test]
    fn oversized_frame_is_reported_as_frame_error() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(
            Path::new("/bin/sh"),
            // 1,2 MB sıfır bayt, tek satır sonu yok: çerçeve sınırı aşılır.
            // `sleep` şart: süreç hemen ölürse okuyucu EOF'u (`Disconnected`)
            // çerçeve hatasından önce görebilir ve test yarışa girer.
            &["-c", "head -c 1200000 /dev/zero; sleep 5"],
            dir.path(),
        )
        .expect("başlat")
        .with_timeout(Duration::from_secs(10));
        let err = client.ping().unwrap_err();
        assert!(
            matches!(err, WorkerError::FrameTooLarge),
            "beklenmeyen hata: {err:?}"
        );
        assert_eq!(err.code(), "E_WORKER_FRAME");
    }

    /// **İptal gerçekten durdurur (F04-T07).** Yanıt vermeyen süreç, 30 sn
    /// zaman aşımını beklemeden öldürülür ve hata `E_WORKER_CANCELED` olur:
    /// kopma/zaman aşımından ayrı kod, çünkü iptal otomatik yeniden denenmez.
    ///
    /// `sh -c "sleep 30"` bilinçli: Linux'ta `sh` torun süreç bırakabilir ve
    /// borunun yazma ucu açık kalır (EOF gelmez). İptal bu yüzden **EOF'a
    /// bağlı değildir** — bekleme 50 ms dilimlerle yapılır (Ubuntu CI'da bu
    /// vaka 30 sn bekliyordu, kusur buydu).
    #[cfg(unix)]
    #[test]
    fn cancel_kills_the_running_worker_and_reports_canceled() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(Path::new("/bin/sh"), &["-c", "sleep 30"], dir.path())
            .expect("başlat")
            .with_timeout(Duration::from_secs(30));
        let killer = client.killer();
        // İsteği gönder, yanıtı beklemeden öldür: gerçek yarış bu.
        let handle = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let err = client.ping().unwrap_err();
            (err, started.elapsed())
        });
        std::thread::sleep(Duration::from_millis(150));
        assert!(killer.cancel(), "süreç sinyal almalı");
        assert!(killer.is_canceled());
        let (err, elapsed) = handle.join().expect("iş parçacığı");
        // Ölçüm: iptal, zaman aşımını beklemeden dönmeli (değer rapora yazılır).
        println!("iptal gecikmesi: {elapsed:?} (zaman aşımı 30 sn)");
        assert_eq!(
            err.code(),
            "E_WORKER_CANCELED",
            "iptal ayrı kodla dönmeli: {err:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "iptal zaman aşımını beklememeli: {elapsed:?}"
        );
    }

    /// İptal işareti istekten **önce** verilirse de ayrı kod döner; ikinci
    /// `cancel()` yanlış bir «durdurdum» iddiası üretmez.
    #[cfg(unix)]
    #[test]
    fn cancel_before_request_reports_canceled_and_is_idempotent() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(Path::new("/bin/sh"), &["-c", "sleep 30"], dir.path())
            .expect("başlat")
            .with_timeout(Duration::from_secs(30));
        let killer = client.killer();
        assert!(killer.cancel(), "ilk iptal sinyal vermeli");
        assert!(!killer.cancel(), "ikinci iptal süreç yok diye false döner");
        let err = client.ping().unwrap_err();
        assert_eq!(err.code(), "E_WORKER_CANCELED");
        assert!(
            !err.retryable(),
            "iptal otomatik tekrar döngüsüne girmemeli (§17.4)"
        );
    }

    /// **Ubuntu CI'da yakalanan vaka (kusur #45):** çocuk süreç ölür ama
    /// **torun** borunun yazma ucunu tutar → EOF gelmez. İptal yine de zaman
    /// aşımını beklemez, çünkü bekleme 50 ms dilimlerle yapılır.
    #[cfg(unix)]
    #[test]
    fn cancel_does_not_wait_for_eof_when_a_grandchild_holds_the_pipe() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(
            Path::new("/bin/sh"),
            // `sleep` arka planda boruyu devralır, `sh` onu bekler: öldürülen
            // çocükten sonra boru 10 sn daha açık kalır.
            &["-c", "sleep 10 & wait"],
            dir.path(),
        )
        .expect("başlat")
        .with_timeout(Duration::from_secs(10));
        let killer = client.killer();
        let handle = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let err = client.ping().unwrap_err();
            (err, started.elapsed())
        });
        std::thread::sleep(Duration::from_millis(150));
        assert!(killer.cancel(), "süreç sinyal almalı");
        let (err, elapsed) = handle.join().expect("iş parçacığı");
        assert_eq!(err.code(), "E_WORKER_CANCELED", "ayrı kod: {err:?}");
        assert!(
            elapsed < Duration::from_secs(2),
            "torun süreç boruyu tutsa da iptal EOF beklemez: {elapsed:?}"
        );
    }

    /// Bozuk çerçeve (JSON olmayan satır) `BadResponse` döner; ham çerçeve
    /// hiçbir yere yazılmaz.
    #[cfg(unix)]
    #[test]
    fn malformed_frame_is_reported_as_bad_response() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(
            Path::new("/bin/sh"),
            // Süreç satırı yazdıktan sonra **yaşamalı**: hemen ölürse okuyucu
            // EOF'u görüp `Disconnected` dönebilir (CI'da yaşandı, 25 Eki).
            &["-c", "printf 'bu bir json degil\\n'; sleep 5"],
            dir.path(),
        )
        .expect("başlat")
        .with_timeout(Duration::from_secs(5));
        let err = client.ping().unwrap_err();
        assert!(
            matches!(err, WorkerError::BadResponse),
            "beklenmeyen hata: {err:?}"
        );
        assert_eq!(err.code(), "E_WORKER_RESPONSE");
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

    /// Vekil worker: verilen çerçeve satırlarını **dosyadan** yazar, sonra
    /// yaşar. Dosya kullanılır çünkü 10.000 satır `sh` argümanına sığmaz;
    /// `sleep` ölü süreç yarışını önler (EOF erken gelebilir).
    #[cfg(unix)]
    fn spawn_frames(lines: &[String], timeout: Duration) -> (WorkerClient, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tmp");
        let script = dir.path().join("frames.ndjson");
        std::fs::write(&script, lines.join("\n") + "\n").expect("çerçeve dosyası");
        let command = format!("cat '{}'; sleep 5", script.display());
        let client =
            WorkerClient::spawn(Path::new("/bin/sh"), &["-c", command.as_str()], dir.path())
                .expect("başlat")
                .with_timeout(timeout);
        (client, dir)
    }

    #[cfg(unix)]
    fn extraction_response(id: &str) -> String {
        json!({
            "v": 1,
            "id": id,
            "ok": true,
            "result": {
                "engine": "plain",
                "engineVersion": "1",
                "pages": [{"index": 0, "text": "metin", "width": 0.0, "height": 0.0}],
                "warnings": []
            }
        })
        .to_string()
    }

    /// **F04-T07 canlı sayaç:** ilerleme çerçeveleri yanıttan önce gelir ve
    /// **yanıt beklenirken** geri çağrıya verilir; sonuç değişmez.
    #[cfg(unix)]
    #[test]
    fn progress_frames_are_forwarded_before_the_response() {
        let lines = vec![
            json!({"v": 1, "id": "req-1", "progress": {"stage": "open", "done": 0, "total": 3}})
                .to_string(),
            json!({"v": 1, "id": "req-1", "progress": {"stage": "pages", "done": 2, "total": 3}})
                .to_string(),
            extraction_response("req-1"),
        ];
        let (mut client, _dir) = spawn_frames(&lines, Duration::from_secs(10));
        let mut seen: Vec<ParseProgress> = Vec::new();
        let extraction = client
            .extract_text_with_progress(
                b"x",
                "text/plain",
                false,
                "eng",
                None,
                None,
                &mut |update| seen.push(update),
            )
            .expect("çıkarım");
        assert_eq!(
            seen,
            vec![
                ParseProgress {
                    stage: "open".into(),
                    done: 0,
                    total: Some(3)
                },
                ParseProgress {
                    stage: "pages".into(),
                    done: 2,
                    total: Some(3)
                },
            ]
        );
        assert_eq!(extraction.pages.len(), 1);
        assert_eq!(extraction.pages[0].text, "metin");
    }

    /// Başka isteğin ilerlemesi bu isteğin sayacına karışmaz (kimlik kapısı).
    #[cfg(unix)]
    #[test]
    fn progress_for_another_request_is_ignored() {
        let lines = vec![
            json!({"v": 1, "id": "req-9", "progress": {"stage": "pages", "done": 1, "total": 2}})
                .to_string(),
            extraction_response("req-1"),
        ];
        let (mut client, _dir) = spawn_frames(&lines, Duration::from_secs(10));
        let mut seen: Vec<ParseProgress> = Vec::new();
        client
            .extract_text_with_progress(
                b"x",
                "text/plain",
                false,
                "eng",
                None,
                None,
                &mut |update| seen.push(update),
            )
            .expect("çıkarım");
        assert!(seen.is_empty(), "yabancı sayaç gösterilmemeli: {seen:?}");
    }

    /// Tutarsız sayaç (5 / 3) sessizce gösterilmez: protokol hatası döner.
    #[cfg(unix)]
    #[test]
    fn inconsistent_progress_counts_are_a_protocol_error() {
        let lines = vec![
            json!({"v": 1, "id": "req-1", "progress": {"stage": "pages", "done": 5, "total": 3}})
                .to_string(),
            extraction_response("req-1"),
        ];
        let (mut client, _dir) = spawn_frames(&lines, Duration::from_secs(10));
        let err = client
            .extract_text_with_progress(b"x", "text/plain", false, "eng", None, None, &mut |_| {})
            .unwrap_err();
        assert!(
            matches!(err, WorkerError::Protocol(_)),
            "beklenmeyen: {err:?}"
        );
        assert_eq!(err.code(), "E_WORKER_PROTOCOL");
    }

    /// Sonsuz ilerleme akıtan süreç isteği kilitleyemez: çerçeve sınırı var.
    #[cfg(unix)]
    #[test]
    fn progress_frames_are_bounded() {
        let storm: Vec<String> = (0..=MAX_PROGRESS_FRAMES)
            .map(|_| {
                json!({"v": 1, "id": "req-1", "progress": {"stage": "pages", "done": 0}})
                    .to_string()
            })
            .collect();
        let (mut client, _dir) = spawn_frames(&storm, Duration::from_secs(30));
        let started = std::time::Instant::now();
        let err = client
            .extract_text_with_progress(b"x", "text/plain", false, "eng", None, None, &mut |_| {})
            .unwrap_err();
        assert_eq!(err.code(), "E_WORKER_PROTOCOL", "{err:?}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "sınır hızla dönmeli: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn progress_parser_reads_counts_and_rejects_bad_frames() {
        let good = json!({"progress": {"stage": "pages", "done": 2, "total": 5}});
        assert_eq!(
            parse_progress(&good).expect("geçerli"),
            Some(ParseProgress {
                stage: "pages".into(),
                done: 2,
                total: Some(5)
            })
        );
        // Toplam yoksa `None`: sıfır uydurulmaz.
        assert_eq!(
            parse_progress(&json!({"progress": {"stage": "open", "done": 0}})).expect("toplamsız"),
            Some(ParseProgress {
                stage: "open".into(),
                done: 0,
                total: None
            })
        );
        // İlerleme yoksa normal yanıt.
        assert!(parse_progress(&json!({"ok": true}))
            .expect("yanıt")
            .is_none());
        for bad in [
            json!({"progress": {"stage": "  ", "done": 0}}),
            json!({"progress": {"stage": "pages", "done": "2"}}),
            json!({"progress": {"stage": "pages", "done": 2, "total": "5"}}),
            json!({"progress": {"stage": "pages", "done": 6, "total": 5}}),
            json!({"progress": 7}),
        ] {
            assert!(
                matches!(parse_progress(&bad), Err(WorkerError::Protocol(_))),
                "bozuk çerçeve kabul edilmemeli: {bad}"
            );
        }
    }
}

#[cfg(test)]
mod env_isolation_tests {
    use super::worker_env;

    #[test]
    fn worker_env_is_allowlisted_and_leaks_no_secrets() {
        std::env::set_var("DIM_TEST_SECRET_TOKEN", "s3cr3t");
        let env = worker_env();
        let names: Vec<&str> = env.iter().map(|(name, _)| name.as_str()).collect();

        assert!(!names.contains(&"DIM_TEST_SECRET_TOKEN"), "{names:?}");
        assert!(!names.contains(&"HOME"), "{names:?}");
        assert!(!names.contains(&"OPENAI_API_KEY"), "{names:?}");
        for name in &names {
            assert!(
                ["PATH", "LANG", "LC_ALL"].contains(name),
                "beklenmeyen değişken: {name}"
            );
        }
        std::env::remove_var("DIM_TEST_SECRET_TOKEN");
    }
}
