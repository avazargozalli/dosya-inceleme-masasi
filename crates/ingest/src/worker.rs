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
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::budget::{BudgetBreak, BudgetDimension, BudgetReport, BudgetUsage, WorkerBudget};

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
    /// Kaynak bütçesi aşıldı (§17.3). Zaman aşımından **ayrı** koddur: hangi
    /// sınırın aşıldığını taşır ve otomatik tekrar denenmez — aynı belge aynı
    /// sınırı yeniden aşar, tekrar denemek kullanıcıyı döngüye sokar (§17.4).
    #[error("worker bütçesi aşıldı: {0}")]
    Budget(BudgetBreak),
    /// `sh` zarfı sınırları uygulayamadı (`ulimit` başarısız → `exit 126`).
    /// Sınırsız çalışan bir worker "bütçeli" sayılmaz: iş baştan reddedilir.
    #[error("worker kaynak sınırları uygulanamadı; süreç başlatılmadı")]
    LimitsNotApplied,
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
            Self::Budget(_) => "E_WORKER_BUDGET",
            Self::LimitsNotApplied => "E_WORKER_LIMIT",
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
    /// §17.3 kaynak bütçesi. `None` yalnız protokol testleri/araçları içindir;
    /// ürün yolları (`spawn_*_budgeted`) her zaman bir profil verir.
    budget: Option<WorkerBudget>,
    /// Gözlenen kullanım: okuyucu iş parçacığı çıktıyı, bellek gözcüsü tepeyi
    /// yazar. İkisi de **sayı** taşır (belge adı/yolu yok).
    usage: Arc<Mutex<BudgetUsage>>,
    /// Süreç başlatma anı; raporun `elapsed` alanı bundan türetilir.
    started: Instant,
    /// Bellek gözcüsünün durdurma bayrağı (`Drop` işaretler).
    watchdog: Option<Arc<AtomicBool>>,
    /// Süreç kimliği: rapor anındaki son bellek örneği için (gözcü aralığından
    /// bağımsız).
    pid: u32,
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

/// Worker sürecini başlatacak komutu kurar.
///
/// Bütçe varsa POSIX `sh` zarfı sınırları **çocuğa** uygular
/// (`WorkerBudget::limit_script`) ve süreci `exec` ile devralır: pid değişmez,
/// borular aynı kalır, `WorkerKiller` yine doğru süreci öldürür. Bütçe yoksa
/// program doğrudan koşar (yalnız protokol testleri/araçları).
fn worker_command(program: &Path, args: &[&str], budget: Option<WorkerBudget>) -> Command {
    #[cfg(unix)]
    {
        if let Some(budget) = budget {
            let mut command = Command::new("/bin/sh");
            // `$0` = program, `$@` = argümanlar: kabuk yorumlaması yok, yalnız
            // konumsal parametre (enjeksiyon yüzeyi açılmaz).
            command
                .arg("-c")
                .arg(budget.limit_script())
                .arg(program)
                .args(args);
            return command;
        }
    }
    // Windows'ta POSIX rlimit yok: CPU/dosya sınırı uygulanmaz, süre ve çıktı
    // bütçesi yine uygulanır (dürüst sınır, F01/F17'de ayrıca ele alınır).
    #[cfg(not(unix))]
    let _ = budget;
    let mut command = Command::new(program);
    command.args(args);
    command
}

impl WorkerClient {
    /// Geliştirme modunda `python3 -m dim_worker.worker`. `cwd`, modülün
    /// bulunduğu klasördür; paketlenmiş sürümde gömülü Python yolu verilir.
    ///
    /// Bütçesiz: yalnız protokol testleri/araçları içindir. Ürün yolu
    /// [`WorkerClient::spawn_dev_budgeted`] (§17.3).
    pub fn spawn_dev(python: &Path, working_dir: &Path) -> Result<Self, WorkerError> {
        Self::spawn(python, &["-m", "dim_worker.worker"], working_dir)
    }

    /// §17.3 bütçeli geliştirme worker'ı: sınırlar süreç **başlatılırken**
    /// uygulanır (sonradan daraltılamaz), süre bütçesi `timeout` olur.
    pub fn spawn_dev_budgeted(
        python: &Path,
        working_dir: &Path,
        budget: WorkerBudget,
    ) -> Result<Self, WorkerError> {
        Self::spawn_with_env(
            python,
            &["-m", "dim_worker.worker"],
            working_dir,
            &[],
            Some(budget),
        )
    }

    /// Paketlenmiş (PyInstaller onedir) worker ikilisini çalıştırır. Protokol
    /// aynıdır: NDJSON, stdin/stdout. Python kurulu olması gerekmez.
    ///
    /// Python ortam değişkenleri **kaldırılır**: paketlenmiş ikili `PYTHONPATH`/
    /// `PYTHONHOME` devralırsa depodaki kaynak modülleri yükleyebilir ve paket
    /// kusuru "çalışıyor" gibi görünür (F01-T02'nin ölçtüğü risk).
    pub fn spawn_bundled(exe: &Path) -> Result<Self, WorkerError> {
        let dir = exe.parent().unwrap_or_else(|| Path::new("."));
        Self::spawn_with_env(exe, &[], dir, &PYTHON_ENV_VARS, None)
    }

    /// §17.3 bütçeli paketlenmiş worker: ürün yolu.
    pub fn spawn_bundled_budgeted(exe: &Path, budget: WorkerBudget) -> Result<Self, WorkerError> {
        let dir = exe.parent().unwrap_or_else(|| Path::new("."));
        Self::spawn_with_env(exe, &[], dir, &PYTHON_ENV_VARS, Some(budget))
    }

    fn spawn(program: &Path, args: &[&str], working_dir: &Path) -> Result<Self, WorkerError> {
        Self::spawn_with_env(program, args, working_dir, &[], None)
    }

    fn spawn_with_env(
        program: &Path,
        args: &[&str],
        working_dir: &Path,
        removed_env: &[&str],
        budget: Option<WorkerBudget>,
    ) -> Result<Self, WorkerError> {
        let mut command = worker_command(program, args, budget);
        command
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

        let pid = child.id();
        let stdin = child.stdin.take().ok_or(WorkerError::Disconnected)?;
        let stdout = child.stdout.take().ok_or(WorkerError::Disconnected)?;
        let (tx, rx) = mpsc::channel();
        let child = Arc::new(Mutex::new(child));
        let usage = Arc::new(Mutex::new(BudgetUsage::default()));
        // Bellek gözcüsü yalnız bütçe varsa koşar: her örnekleme bir `ps`
        // süreci başlatır (macOS), boşa maliyet çıkarmaz.
        let watchdog = match budget {
            Some(budget) if budget.memory_bytes > 0 => Some(start_memory_watchdog(
                pid,
                budget.memory_bytes,
                Arc::clone(&usage),
                Arc::clone(&child),
            )),
            _ => None,
        };
        let reader_usage = Arc::clone(&usage);
        let output_limit = budget.map_or(0, |budget| budget.output_bytes);

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
                        if let Ok(mut usage) = reader_usage.lock() {
                            usage.note_output(
                                u64::try_from(buf.len()).unwrap_or(u64::MAX),
                                output_limit,
                            );
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
            cancel: Arc::new(AtomicBool::new(false)),
            stdin,
            rx,
            next_id: 0,
            // §17.3: süre bütçesi tek "timeout" değerinin yerine geçer.
            timeout: budget.map_or(DEFAULT_TIMEOUT, |budget| budget.wall_clock),
            budget,
            usage,
            started: Instant::now(),
            watchdog,
            pid,
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
    /// işimizdir ve `E_WORKER_CANCELED` döner; bütçe aşıldıysa hangi boyutun
    /// aşıldığı ayrı kodla döner; aksi hâlde gerçek arıza.
    fn interrupt_error(&self) -> WorkerError {
        if self.cancel.load(Ordering::SeqCst) {
            return WorkerError::Canceled;
        }
        if let Some(error) = self.budget_error() {
            return error;
        }
        if let Some(error) = self.signal_budget_error() {
            return error;
        }
        WorkerError::Disconnected
    }

    /// Gözlenen ihlali hataya çevirir (bellek gözcüsü ve çıktı sayacı yazar).
    fn budget_error(&self) -> Option<WorkerError> {
        let budget = self.budget.as_ref()?;
        let usage = self.usage.lock().ok()?;
        let dimension = usage.violated?;
        Some(WorkerError::Budget(BudgetBreak {
            dimension,
            limit: budget.limit_for(dimension),
        }))
    }

    /// Süreç bütçe sinyaliyle mi öldü? `SIGXCPU` (24) CPU sınırını gösterir;
    /// POSIX'te sabittir. Çıkış kodu hemen görünmeyebilir (EOF önce gelir):
    /// en çok 200 ms beklenir.
    ///
    /// **`SIGXFSZ` (25) bilinçli olarak eşlenmez:** `RLIMIT_FSIZE` uygulanmıyor
    /// (çıktı bütçesi yanıt **baytıyla** ölçülür), dolayısıyla bu sinyalle ölen
    /// sürece "çıktı bütçesi aşıldı" demek yanlış olurdu.
    ///
    /// **`SIGKILL` bilinçli olarak eşlenmez:** iki farklı neden olabilir (bellek
    /// gözcüsü — ki o ihlali bayrakla yazar ve `budget_error()` önce görür — ya
    /// da dışarıdan öldürme). Sinyalden neden uydurulmaz.
    #[cfg(unix)]
    fn signal_budget_error(&self) -> Option<WorkerError> {
        use std::os::unix::process::ExitStatusExt;

        let budget = self.budget.as_ref()?;
        let mut waited = Duration::ZERO;
        loop {
            let status = self
                .child
                .lock()
                .ok()
                .and_then(|mut child| child.try_wait().ok().flatten());
            match status {
                Some(status) => {
                    // `ulimit` uygulanamadıysa kabuk `exit 126` ile döner:
                    // sınırsız çalışan worker "bütçeli" sayılmaz.
                    if status.code() == Some(126) {
                        return Some(WorkerError::LimitsNotApplied);
                    }
                    let dimension = match status.signal() {
                        Some(24) => BudgetDimension::Cpu,
                        _ => return None,
                    };
                    // Rapor da aynı nedeni taşısın: sinyal yolu sayaç yazmaz.
                    if let Ok(mut usage) = self.usage.lock() {
                        usage.mark_violation(dimension);
                    }
                    return Some(WorkerError::Budget(BudgetBreak {
                        dimension,
                        limit: budget.limit_for(dimension),
                    }));
                }
                None if waited < Duration::from_millis(200) => {
                    std::thread::sleep(Duration::from_millis(10));
                    waited += Duration::from_millis(10);
                }
                None => return None,
            }
        }
    }

    #[cfg(not(unix))]
    fn signal_budget_error(&self) -> Option<WorkerError> {
        None
    }

    /// §17.3 raporu: profil, gözlenen tepe bellek/çıktı, geçen süre ve ihlal.
    /// Bütçesiz istemcide `None` (uydurma sayı yok).
    ///
    /// Rapor anında **son bir örnek** alınır: kısa işte (gözcü aralığı 500 ms)
    /// hiç örnek alınmamış olabilir ve "tepe bellek 0 MiB" yanlış olurdu. Süreç
    /// hâlâ yaşıyorsa pid yeniden kullanılmamıştır (`try_wait` süreci toplar,
    /// toplanmış pid'e örnek alınmaz → başka sürecin belleği yazılmaz).
    #[must_use]
    pub fn budget_report(&self) -> Option<BudgetReport> {
        let budget = *self.budget.as_ref()?;
        let running = self
            .child
            .lock()
            .ok()
            .and_then(|mut child| child.try_wait().ok().flatten())
            .is_none();
        let mut usage = *self.usage.lock().ok()?;
        if running {
            if let Some(bytes) = sample_rss_bytes(self.pid) {
                usage.note_peak(bytes);
            }
        }
        Some(BudgetReport {
            budget,
            elapsed: self.started.elapsed(),
            output_bytes: usage.output_bytes,
            peak_memory_bytes: usage.peak_memory_bytes,
            violated: usage.violated,
        })
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
        // §17.3: çıktı bütçesi **istek başına** sayılır; parçalı yanıtın bütün
        // parçaları aynı işin çıktısıdır (sayaç sıfırlanmaz, parça eklenir).
        if let Ok(mut usage) = self.usage.lock() {
            usage.reset_output();
        }
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
        let deadline = Instant::now() + self.timeout;
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(WorkerError::Canceled);
            }
            // Bellek gözcüsü/çıktı sayacı ihlali yazdıysa bekleme uzamaz.
            if let Some(error) = self.budget_error() {
                return Err(error);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                // §17.3: bütçeli işte süre aşımı **bütçe** hatasıdır; bütçesiz
                // istemcide eski `E_WORKER_TIMEOUT` korunur.
                return Err(match self.budget {
                    Some(budget) => {
                        if let Ok(mut usage) = self.usage.lock() {
                            usage.mark_violation(BudgetDimension::Time);
                        }
                        WorkerError::Budget(BudgetBreak {
                            dimension: BudgetDimension::Time,
                            limit: budget.wall_clock.as_secs(),
                        })
                    }
                    None => WorkerError::Timeout,
                });
            }
            match self.rx.recv_timeout(remaining.min(CANCEL_POLL)) {
                Ok(Ok(bytes)) => {
                    // Çıktı bütçesi okuyucu iş parçacığında sayılır; burada
                    // yalnız görülür. Sınırı aşan çerçeve sonuç sayılmaz.
                    if let Some(error) = self.budget_error() {
                        return Err(error);
                    }
                    return Ok(bytes);
                }
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
        self.redact_with_progress(data, terms, &mut |_| {})
    }

    /// Terim maskesini **ilerleme çerçeveleriyle** çalıştırır (F04-T07).
    ///
    /// Worker `redact` aşamasında sayfa sayacı, sonra `pack`/`verify`/`scan`
    /// aşamalarını bildirir. Çerçeve gelmezse sonuç yine aynıdır; ilerleme
    /// yalnız bilgilendirir.
    pub fn redact_with_progress(
        &mut self,
        data: &[u8],
        terms: &[String],
        progress: &mut dyn FnMut(ParseProgress),
    ) -> Result<RedactionOutcome, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let id = self.send_request("redact", json!({"contentBase64": encoded, "terms": terms}))?;
        let result = self.read_response(&id, &mut *progress)?;
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
        self.redact_regions_with_progress(data, regions, &mut |_| {})
    }

    /// Bölge maskesini **ilerleme çerçeveleriyle** çalıştırır (F04-T07).
    pub fn redact_regions_with_progress(
        &mut self,
        data: &[u8],
        regions: &[(u32, [f64; 4])],
        progress: &mut dyn FnMut(ParseProgress),
    ) -> Result<RedactionOutcome, WorkerError> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let items: Vec<serde_json::Value> = regions
            .iter()
            .map(|(page, rect)| json!({"page": page, "rect": rect}))
            .collect();
        let id = self.send_request(
            "redact_regions",
            json!({"contentBase64": encoded, "regions": items}),
        )?;
        let result = self.read_response(&id, &mut *progress)?;
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
        // Bellek gözcüsü süreç tutamacını paylaşır; önce durdurulur ki kapanan
        // istemci için örnekleme yapmasın.
        if let Some(stop) = &self.watchdog {
            stop.store(true, Ordering::SeqCst);
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Bellek gözcüsünün örnekleme aralığı. Daha sık örnekleme, macOS'ta her
/// örnek için bir `ps` süreci başlatmak demektir; 500 ms taşmayı saniye
/// içinde yakalar.
const MEMORY_SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

/// Bekleme dilimi: gözcü, istemci düşünce bu dilimde uyanıp çıkar.
const MEMORY_POLL_SLICE: Duration = Duration::from_millis(100);

/// Worker'ın **yerleşik** belleğini (RSS) örnekler.
///
/// macOS'ta `/proc` yok; `libproc`/`proc_pid_rusage` ise `unsafe` ister ve
/// çalışma alanı `unsafe_code = "forbid"` der. Bu yüzden `ps -o rss=` okunur
/// (süreç başına bir alt süreç; bütçe yoksa hiç çağrılmaz).
#[cfg(target_os = "macos")]
fn sample_rss_bytes(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let kilobytes: u64 = text.trim().parse().ok()?;
    Some(kilobytes.saturating_mul(1024))
}

/// Linux: `/proc/<pid>/status` içindeki `VmRSS` (kB). Süreç başına dosya
/// okuması; alt süreç başlatılmaz.
#[cfg(target_os = "linux")]
fn sample_rss_bytes(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kilobytes: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kilobytes.saturating_mul(1024));
        }
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn sample_rss_bytes(_pid: u32) -> Option<u64> {
    None
}

/// Bellek gözcüsü: tepe RSS bütçeyi aşarsa ihlali yazar ve süreci öldürür.
///
/// İlk örnek **hemen** alınır (beklemeden): kısa işte 500 ms beklemek "tepe
/// bellek 0" demekti. Sonra her [`MEMORY_SAMPLE_INTERVAL`]'da bir örnek alınır.
///
/// Dürüst sınır: örnekleme **aralıklıdır** (macOS 500 ms), yani süreç örnekler
/// arasında sınırı aşabilir; öldürme gerçekleşmezse bile ihlal bayrağı yazılır
/// ve çağıran en geç `CANCEL_POLL` (50 ms) içinde bütçe hatasını görür.
fn start_memory_watchdog(
    pid: u32,
    limit: u64,
    usage: Arc<Mutex<BudgetUsage>>,
    child: Arc<Mutex<Child>>,
) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    std::thread::spawn(move || loop {
        if let Some(bytes) = sample_rss_bytes(pid) {
            let breached = match usage.lock() {
                Ok(mut usage) => usage.note_memory(bytes, limit).is_some(),
                Err(_) => return,
            };
            if breached {
                if let Ok(mut child) = child.try_lock() {
                    let _ = child.kill();
                }
                return;
            }
        }
        let mut waited = Duration::ZERO;
        while waited < MEMORY_SAMPLE_INTERVAL {
            if flag.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(MEMORY_POLL_SLICE);
            waited += MEMORY_POLL_SLICE;
        }
    });
    stop
}

/// Geliştirme ortamında Python yorumlayıcısını bulur (`python3`, sonra `python`).
///
/// `DIM_WORKER_PYTHON` verildiyse **o** kullanılır (Tauri ile aynı kural:
/// ölçüm/ölçek koşuları kendi sanal ortamını verebilir). Verilen yol çalışmıyorsa
/// sistem python'una **düşülmez** — yanlış yol sessizce gizlenmez.
///
/// Süreç başlatma bu modülde merkezîdir (ADR-05); worker'ı sınayan testler de
/// yorumlayıcıyı bu yardımcıyla arar, kendi `Command` çağrısını yazmaz.
#[must_use]
pub fn dev_python() -> Option<PathBuf> {
    if let Some(chosen) = std::env::var_os("DIM_WORKER_PYTHON") {
        let path = PathBuf::from(chosen);
        let works = Command::new(&path)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success());
        return works.then_some(path);
    }
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
    // Geçici dizin **açıkça** verilir (F04-T07'de ölçüldü): allowlist'te yokken
    // Python `tempfile` macOS'ta `/tmp`e düşüyor; `/tmp` bir sembolik bağ
    // olduğu için pytesseract geçici çıktıyı okuyamıyor ve OCR doğrulaması
    // `UnicodeDecodeError` ile çöküyordu → maskeleme ürün yolunda hiç
    // kaydedilmiyordu. `canonicalize` gerçek yolu verir (`/private/tmp`).
    if let Some(dir) = worker_temp_dir() {
        for name in ["TMPDIR", "TMP", "TEMP"] {
            out.push((name.to_string(), dir.clone().into_os_string()));
        }
    }
    out
}

/// Worker'ın kullanacağı geçici dizin: kabuk ortamından devralınmaz, kendi
/// çözülmüş yolundan türetilir (sembolik bağ verilmez).
fn worker_temp_dir() -> Option<PathBuf> {
    let dir = std::env::temp_dir();
    let resolved = dir.canonicalize().unwrap_or(dir);
    if resolved.is_dir() {
        Some(resolved)
    } else {
        None
    }
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

    /// **F04-T07:** maskeleme de canlı sayaç verir. Çerçeveler yanıttan önce
    /// geri çağrıya verilir; sonuç (`pdf`, `hits`, `remaining`) aynı çözülür.
    #[cfg(unix)]
    #[test]
    fn redaction_progress_frames_are_forwarded_before_the_response() {
        let lines = vec![
            json!({"v": 1, "id": "req-1", "progress": {"stage": "redact", "done": 0, "total": 2}})
                .to_string(),
            json!({"v": 1, "id": "req-1", "progress": {"stage": "redact", "done": 2, "total": 2}})
                .to_string(),
            json!({"v": 1, "id": "req-1", "progress": {"stage": "verify", "done": 2, "total": 2}})
                .to_string(),
            json!({
                "v": 1,
                "id": "req-1",
                "ok": true,
                "result": {
                    "pdfBase64": base64::engine::general_purpose::STANDARD.encode(b"%PDF-maskeli"),
                    "hits": [],
                    "remaining": [],
                    "engine": "pymupdf",
                    "engineVersion": "1.24.14",
                    "engines": ["pymupdf", "raw-scan"]
                }
            })
            .to_string(),
        ];
        let (mut client, _dir) = spawn_frames(&lines, Duration::from_secs(10));
        let mut seen: Vec<ParseProgress> = Vec::new();
        let outcome = client
            .redact_with_progress(b"x", &["Gizli".to_string()], &mut |update| {
                seen.push(update)
            })
            .expect("maskeleme");
        assert_eq!(
            seen.iter()
                .map(|update| (update.stage.as_str(), update.done, update.total))
                .collect::<Vec<_>>(),
            vec![
                ("redact", 0, Some(2)),
                ("redact", 2, Some(2)),
                ("verify", 2, Some(2)),
            ]
        );
        assert_eq!(outcome.pdf, b"%PDF-maskeli");
        assert!(outcome.remaining.is_empty());
        assert_eq!(outcome.engines, vec!["pymupdf", "raw-scan"]);
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

    // ---- §17.3 kaynak bütçesi -------------------------------------------------
    //
    // Bu testler **gerçek süreç** başlatır: sınırın gerçekten uygulandığını
    // (yalnız tanımlı olduğunu değil) kanıtlar. Python yoksa atlanır.

    use crate::budget::{WorkerKind, MIB};

    /// Verilen python betiğini bütçeyle başlatır. `args`, betiğe **aynen**
    /// geçirilir (kabuk yorumlaması olmadığını sınamak için).
    fn spawn_budgeted(
        script: &str,
        args: &[&str],
        budget: WorkerBudget,
    ) -> Option<(WorkerClient, tempfile::TempDir)> {
        let py = python()?;
        let dir = tempfile::tempdir().expect("tmp");
        let mut full: Vec<&str> = vec!["-c", script];
        full.extend_from_slice(args);
        let client = WorkerClient::spawn_with_env(&py, &full, dir.path(), &[], Some(budget))
            .expect("başlat");
        Some((client, dir))
    }

    /// Bütçeli iş **`sh` zarfıyla** başlar; bütçesiz iş doğrudan koşar.
    #[cfg(unix)]
    #[test]
    fn budget_wraps_the_program_in_a_limit_shell_only_when_given() {
        let budget = WorkerBudget::for_kind(WorkerKind::Extract);
        let wrapped = worker_command(Path::new("/usr/bin/python3"), &["-c", "pass"], Some(budget));
        assert_eq!(wrapped.get_program(), std::ffi::OsStr::new("/bin/sh"));
        let plain = worker_command(Path::new("/usr/bin/python3"), &["-c", "pass"], None);
        assert_eq!(
            plain.get_program(),
            std::ffi::OsStr::new("/usr/bin/python3")
        );
    }

    /// CPU bütçesi: sonsuz döngü `SIGXCPU` ile durur ve hata **kendi koduyla**
    /// döner (`E_WORKER_BUDGET`), zaman aşımı beklemez.
    #[cfg(unix)]
    #[test]
    fn cpu_budget_stops_a_runaway_worker_with_its_own_code() {
        let mut budget = WorkerBudget::for_kind(WorkerKind::Extract);
        budget.cpu_seconds = 1;
        budget.wall_clock = Duration::from_secs(30);
        let script = "x = 0\nwhile True:\n    x += 1\n";
        let Some((mut client, _dir)) = spawn_budgeted(script, &[], budget) else {
            eprintln!("python bulunamadı; test atlandı");
            return;
        };
        let started = Instant::now();
        let err = client.ping().unwrap_err();
        assert!(
            matches!(
                err,
                WorkerError::Budget(BudgetBreak {
                    dimension: BudgetDimension::Cpu,
                    ..
                })
            ),
            "beklenmeyen: {err:?}"
        );
        assert_eq!(err.code(), "E_WORKER_BUDGET");
        assert!(!err.retryable(), "bütçe aşımı tekrar denenmez (§17.4)");
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "CPU sınırı duvar saatini beklemez: {:?}",
            started.elapsed()
        );
        let report = client.budget_report().expect("bütçe raporu");
        assert_eq!(report.budget.kind, WorkerKind::Extract);
        assert_eq!(report.violated, Some(BudgetDimension::Cpu));
        assert_eq!(report.budget_break().expect("ihlal").limit, 1);
    }

    /// Sinyal **eşlemesi** (çekirdek `ulimit` semantiğinden bağımsız): kendini
    /// `SIGXCPU` ile öldüren süreç CPU bütçesi olarak raporlanır. Linux'ta
    /// soft = hard sınırı `SIGKILL` ürettiği için bu yol ayrıca tutulmalı.
    #[cfg(unix)]
    #[test]
    fn a_worker_killed_by_the_cpu_signal_reports_the_cpu_budget() {
        let mut budget = WorkerBudget::for_kind(WorkerKind::Extract);
        budget.cpu_seconds = 0; // sınır zarfı uygulanmasın: yalnız sinyal yolu
        budget.memory_bytes = 0;
        budget.wall_clock = Duration::from_secs(20);
        let script =
            "import os, signal, time\nos.kill(os.getpid(), signal.SIGXCPU)\ntime.sleep(5)\n";
        let Some((mut client, _dir)) = spawn_budgeted(script, &[], budget) else {
            eprintln!("python bulunamadı; test atlandı");
            return;
        };
        let err = client.ping().unwrap_err();
        assert!(
            matches!(
                err,
                WorkerError::Budget(BudgetBreak {
                    dimension: BudgetDimension::Cpu,
                    ..
                })
            ),
            "beklenmeyen: {err:?}"
        );
        assert_eq!(err.code(), "E_WORKER_BUDGET");
        assert!(!err.retryable(), "CPU aşımı tekrar denenmez (§17.4)");
    }

    /// Bellek bütçesi: 200 MiB ayıran süreç 64 MiB sınırında durdurulur.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn memory_budget_stops_a_worker_that_outgrows_the_limit() {
        let mut budget = WorkerBudget::for_kind(WorkerKind::Extract);
        budget.cpu_seconds = 0;
        budget.memory_bytes = 64 * MIB;
        budget.wall_clock = Duration::from_secs(30);
        let script = "import time\nx = bytearray(200 * 1024 * 1024)\ntime.sleep(30)\n";
        let Some((mut client, _dir)) = spawn_budgeted(script, &[], budget) else {
            eprintln!("python bulunamadı; test atlandı");
            return;
        };
        let err = client.ping().unwrap_err();
        assert!(
            matches!(
                err,
                WorkerError::Budget(BudgetBreak {
                    dimension: BudgetDimension::Memory,
                    ..
                })
            ),
            "beklenmeyen: {err:?}"
        );
        let report = client.budget_report().expect("bütçe raporu");
        assert!(
            report.peak_memory_bytes > 64 * MIB,
            "gözlenen tepe: {}",
            report.peak_memory_bytes
        );
    }

    /// Çıktı bütçesi: sınırı aşan yanıt sonuç sayılmaz.
    #[cfg(unix)]
    #[test]
    fn output_budget_rejects_a_response_over_the_limit() {
        let mut budget = WorkerBudget::for_kind(WorkerKind::Extract);
        budget.cpu_seconds = 0;
        budget.memory_bytes = 0;
        budget.output_bytes = 64 * 1024;
        let script = concat!(
            "import json, sys\n",
            "line = sys.stdin.readline()\n",
            "request = json.loads(line)\n",
            "blob = \"x\" * 200000\n",
            "sys.stdout.write(json.dumps({\"v\": 1, \"id\": request[\"id\"], \"ok\": True, ",
            "\"result\": {\"blob\": blob}}) + \"\\n\")\n",
            "sys.stdout.flush()\n",
            "sys.stdin.readline()\n",
        );
        let Some((mut client, _dir)) = spawn_budgeted(script, &[], budget) else {
            eprintln!("python bulunamadı; test atlandı");
            return;
        };
        let err = client.ping().unwrap_err();
        assert!(
            matches!(
                err,
                WorkerError::Budget(BudgetBreak {
                    dimension: BudgetDimension::Output,
                    ..
                })
            ),
            "beklenmeyen: {err:?}"
        );
        let report = client.budget_report().expect("bütçe raporu");
        assert!(report.output_bytes > 64 * 1024);
    }

    /// Duvar saati bütçesi zaman aşımından **ayrı** raporlanır.
    #[cfg(unix)]
    #[test]
    fn wall_clock_budget_is_reported_as_budget_not_timeout() {
        let mut budget = WorkerBudget::for_kind(WorkerKind::Render);
        budget.cpu_seconds = 0;
        budget.memory_bytes = 0;
        budget.wall_clock = Duration::from_secs(1);
        let script = "import time\ntime.sleep(30)\n";
        let Some((mut client, _dir)) = spawn_budgeted(script, &[], budget) else {
            eprintln!("python bulunamadı; test atlandı");
            return;
        };
        let err = client.ping().unwrap_err();
        assert_eq!(err.code(), "E_WORKER_BUDGET");
        match err {
            WorkerError::Budget(BudgetBreak {
                dimension: BudgetDimension::Time,
                limit,
            }) => assert_eq!(limit, 1),
            other => panic!("beklenmeyen: {other:?}"),
        }
    }

    /// Rapor gerçek sayıları taşır ve **argümanlar kabuk yorumlamasına girmez**.
    #[cfg(unix)]
    #[test]
    fn budget_report_records_limits_and_arguments_are_not_shell_expanded() {
        let budget = WorkerBudget::for_kind(WorkerKind::Ocr);
        let script = concat!(
            "import json, sys, time\n",
            "line = sys.stdin.readline()\n",
            "request = json.loads(line)\n",
            "time.sleep(1.0)\n",
            "sys.stdout.write(json.dumps({\"v\": 1, \"id\": request[\"id\"], \"ok\": True, ",
            "\"result\": dict(enumerate(sys.argv[1:]))}) + \"\\n\")\n",
            "sys.stdout.flush()\n",
            "sys.stdin.readline()\n",
        );
        let tricky = ["$(whoami)", "a b", "tek'tırnak", "noktalı;virgül"];
        let Some((mut client, _dir)) = spawn_budgeted(script, &tricky, budget) else {
            eprintln!("python bulunamadı; test atlandı");
            return;
        };
        let result = client.request("ping", json!({})).expect("yanıt");
        for (index, expected) in tricky.iter().enumerate() {
            assert_eq!(
                result.get(index.to_string()).and_then(Value::as_str),
                Some(*expected),
                "argüman kabuk tarafından değiştirilmemeli"
            );
        }
        let report = client.budget_report().expect("bütçe raporu");
        assert_eq!(report.budget, budget);
        assert_eq!(report.budget.kind, WorkerKind::Ocr);
        assert!(report.elapsed >= Duration::from_millis(900));
        assert!(report.output_bytes > 0);
        assert!(report.peak_memory_bytes > 0, "gözcü en az bir örnek almalı");
        assert_eq!(report.violated, None);
        assert!(report.budget_break().is_none());
        client.shutdown();
    }

    /// Kısa iş: gözcü aralığı (500 ms) dolmadan biten istekte tepe bellek
    /// **0 yazılmaz**. Ölçülen kusur: 433 ms'lik gerçek ayrıştırmada rapor
    /// "tepe bellek 0 MiB" demişti, çünkü gözcü hiç uyanmamıştı. Bu testte
    /// bellek bütçesi kapalı → gözcü **koşmaz**; tek örnek rapor anında alınır.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn report_samples_peak_memory_even_when_the_job_ends_before_the_watchdog_wakes() {
        let mut budget = WorkerBudget::for_kind(WorkerKind::Extract);
        budget.cpu_seconds = 0;
        budget.memory_bytes = 0;
        budget.wall_clock = Duration::from_secs(20);
        let script = concat!(
            "import json, sys\n",
            "line = sys.stdin.readline()\n",
            "request = json.loads(line)\n",
            "sys.stdout.write(json.dumps({\"v\": 1, \"id\": request[\"id\"], \"ok\": True, ",
            "\"result\": {\"pong\": True}}) + \"\\n\")\n",
            "sys.stdout.flush()\n",
            "sys.stdin.readline()\n",
        );
        let Some((mut client, _dir)) = spawn_budgeted(script, &[], budget) else {
            eprintln!("python bulunamadı; test atlandı");
            return;
        };
        let started = Instant::now();
        client.request("ping", json!({})).expect("yanıt");
        let process_time = started.elapsed();
        assert!(
            process_time < Duration::from_millis(500),
            "iş gözcü aralığından kısa olmalı: {process_time:?}"
        );
        let report = client.budget_report().expect("bütçe raporu");
        assert!(
            report.peak_memory_bytes > 0,
            "rapor anındaki örnek tepeyi yazmalı (0 MiB değil)"
        );
    }

    /// Gerçek worker + kurgu fikstür: ölçüm, varsayım değil. `#[ignore]` çünkü
    /// Python + PyMuPDF ister ve süre ölçer (CI'da koşmaz; elle koşulur).
    #[cfg(unix)]
    #[test]
    #[ignore = "gerçek worker + fikstür ölçümü: DIM_WORKER_PYTHON ile elle koşulur"]
    fn real_parse_reports_peak_memory_within_the_extract_budget() {
        let (Some(py), Some(dir)) = (python(), dev_worker_dir()) else {
            eprintln!("python veya workers/document bulunamadı; ölçüm atlandı");
            return;
        };
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic/progress-pages.pdf");
        let data = std::fs::read(&path).expect("fikstür");
        let budget = WorkerBudget::for_kind(WorkerKind::Extract);
        let started = Instant::now();
        let mut client = WorkerClient::spawn_dev_budgeted(&py, &dir, budget).expect("başlat");
        let extraction = client
            .extract_text(&data, "application/pdf", false, "eng", None, None)
            .expect("çıkarım");
        let process_time = started.elapsed();
        let report = client.budget_report().expect("rapor");
        println!(
            "ölçüm: sayfa={} süreç_süresi={:?} tepe_bellek={} MiB yanıt={} B ihlal={:?} sınırlar: cpu={} sn bellek={} MiB çıktı={} MiB dosya={} süre={:?}",
            extraction.pages.len(),
            process_time,
            report.peak_memory_bytes / MIB,
            report.output_bytes,
            report.violated,
            budget.cpu_seconds,
            budget.memory_bytes / MIB,
            budget.output_bytes / MIB,
            budget.open_files,
            budget.wall_clock
        );
        assert_eq!(report.violated, None, "gerçek iş bütçeyi aşmamalı");
        assert!(report.peak_memory_bytes > 0, "gözcü en az bir örnek almalı");
        assert!(report.output_bytes > 0);
        client.shutdown();
    }
}

#[cfg(test)]
mod env_isolation_tests {
    use super::worker_env;
    use std::path::PathBuf;

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
                ["PATH", "LANG", "LC_ALL", "TMPDIR", "TMP", "TEMP"].contains(name),
                "beklenmeyen değişken: {name}"
            );
        }
        std::env::remove_var("DIM_TEST_SECRET_TOKEN");
    }

    /// F04-T07'de ölçülen kusurun kapısı: worker'a verilen geçici dizin
    /// **gerçek** bir dizin olmalı. `/tmp` gibi bir sembolik bağ verilirse
    /// pytesseract geçici çıktıyı okuyamıyor (`UnicodeDecodeError`) ve OCR
    /// doğrulaması çöküyor — maskeleme ürün yolunda hiç kaydedilmiyordu.
    #[test]
    fn worker_env_hands_a_real_temp_directory() {
        let env = worker_env();
        let (_, tmp) = env
            .iter()
            .find(|(name, _)| name == "TMPDIR")
            .expect("TMPDIR verilmeli");
        let path = PathBuf::from(tmp);
        assert!(path.is_absolute(), "{path:?}");
        assert!(path.is_dir(), "{path:?}");
        assert_eq!(
            path.canonicalize().expect("çözülmeli"),
            path,
            "geçici dizin sembolik bağ olmamalı: {path:?}"
        );
        // Üç ad da aynı değeri taşır (Windows `TEMP`/`TMP` okur).
        for name in ["TMP", "TEMP"] {
            let value = env
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
                .expect("kardeş ad");
            assert_eq!(value, *tmp, "{name}");
        }
    }
}
