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
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// Parçalı yanıtta kabul edilen en fazla çerçeve sayısı; sınırsız döngü yok.
pub const MAX_RESPONSE_PARTS: usize = 5000;

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
    /// Çıktıyı doğrulayan motorlar (pymupdf, pypdf, tesseract).
    pub engines: Vec<String>,
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

    /// Paketlenmiş (PyInstaller onedir) worker ikilisini çalıştırır. Protokol
    /// aynıdır: NDJSON, stdin/stdout. Python kurulu olması gerekmez.
    pub fn spawn_bundled(exe: &Path) -> Result<Self, WorkerError> {
        let dir = exe.parent().unwrap_or_else(|| Path::new("."));
        Self::spawn(exe, &[], dir)
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

    /// İsteği yazar ve kimliğini döndürür. Yanıt okuma ayrı adımdır: parçalı
    /// yanıtta aynı kimlikle birden çok çerçeve gelebilir.
    fn send_request(&mut self, method: &str, params: Value) -> Result<String, WorkerError> {
        self.next_id += 1;
        let id = format!("req-{}", self.next_id);
        let frame = json!({"v": 1, "id": id, "method": method, "params": params});
        let mut line = serde_json::to_vec(&frame).map_err(|_| WorkerError::BadResponse)?;
        line.push(b'\n');
        self.stdin
            .write_all(&line)
            .map_err(|_| WorkerError::Disconnected)?;
        self.stdin.flush().map_err(|_| WorkerError::Disconnected)?;
        Ok(id)
    }

    /// Verilen kimliğe ait ilk yanıtı okur.
    fn read_response(&mut self, id: &str) -> Result<Value, WorkerError> {
        // Yanıtlar sıralıdır; id eşleşmeyen eski çerçeveler atlanır.
        for _ in 0..8 {
            let raw = match self.rx.recv_timeout(self.timeout) {
                Ok(Ok(bytes)) => bytes,
                Ok(Err(reason)) => {
                    // Okuyucu iş parçacığı iki farklı arıza bildirir: çerçeve
                    // sınırı aşımı ve okuma hatası. İkisini "bağlantı koptu"
                    // diye birleştirmek çağıranın ayırt etmesini engellerdi
                    // (aşırı çerçeve protokol/kötü niyet işareti, kopma ise
                    // süreç ölümü); ayrı kod döner.
                    return Err(if reason == "frame_too_large" {
                        WorkerError::FrameTooLarge
                    } else {
                        WorkerError::Disconnected
                    });
                }
                Err(RecvTimeoutError::Timeout) => return Err(WorkerError::Timeout),
                Err(RecvTimeoutError::Disconnected) => return Err(WorkerError::Disconnected),
            };
            let value: Value =
                serde_json::from_slice(&raw).map_err(|_| WorkerError::BadResponse)?;
            if value.get("id").and_then(Value::as_str) != Some(id) {
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

    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, WorkerError> {
        let id = self.send_request(method, params)?;
        self.read_response(&id)
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
            let result = self.read_response(&id)?;
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
        })
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
        })
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
        let _ = self.child.wait();
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
            &["-c", "head -c 1200000 /dev/zero"],
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

    /// Bozuk çerçeve (JSON olmayan satır) `BadResponse` döner; ham çerçeve
    /// hiçbir yere yazılmaz.
    #[cfg(unix)]
    #[test]
    fn malformed_frame_is_reported_as_bad_response() {
        let dir = tempfile::tempdir().expect("tmp");
        let mut client = WorkerClient::spawn(
            Path::new("/bin/sh"),
            &["-c", "printf 'bu bir json degil\\n'"],
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
}
