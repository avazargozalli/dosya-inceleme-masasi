//! Kalıcı iş kuyruğu: sahiplenme (lease), doğrulama, tamamlama, hata, engel,
//! iptal ve yeniden başlatma kurtarması.
//!
//! Şartname §17.2'nin durum makinesi **tek kaynakta** (`crates/job_state_machine.tsv`)
//! durur; `dim-jobs` (kural crate'i) ve bu modül aynı dosyayı okur. Her geçiş
//! SQL'den önce `job_state::allowed` ile denetlenir: tabloda olmayan bir ok
//! `E_JOB_STATE` ile reddedilir, sessizce yazılmaz.
//!
//! Geçişler (özet):
//! - sahiplenme: `queued|retry_wait → running` (tek işlemde; `retry_wait`
//!   durumunda `retry_wait → queued` adımı da aynı işlemde tüketilir),
//! - doğrulama: `running → validating` (`begin_validation`),
//! - sonuç: `validating → completed` / `→ partial` / `→ failed`,
//! - engel: `running → blocked` (`blocked_reason` yazılır) ve `blocked → queued`,
//! - iptal: `queued|retry_wait|blocked → canceled`, `running|validating →
//!   canceling`; yürütücü bitince `canceling → canceled`,
//! - kesinti: `running|validating|canceling → interrupted`, ardından
//!   `interrupted → queued` (yeniden sıraya) ya da `→ failed` (bütçe bitti),
//! - kullanıcı yeniden denemesi: `failed → queued` (yeni attempt açar).
//!
//! Dürüst sınırlar:
//! - Kuyruk yürütücüsü tek süreçlidir: kasa başına tek `Database` bağlantısı
//!   (tek yazıcı) kuralı burada da geçerli. Çok süreçli yarış yalnız
//!   `lease_until` ile korunur.
//! - `partial` bu satır için terminaldir; şartnamenin "yeni alt iş" yolu ayrı
//!   bir iş satırıdır (aynı satırın geçişi değil).
//! - `interrupted → queued` "yeniden doğrulama" adımı bugün **girdi parmak
//!   izinin değişmediği** varsayımıyla yapılır (yerel deterministik iş;
//!   `parse_document` girdisi kasadaki blob + ayarlardır).
//! - Ölçüm: `docs/releases/F15/BENCHMARK.md` (kuyruk gecikmesi p50/p95).

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::job_state;
use crate::{now_iso, Database, StorageError};

/// Belge ayrıştırma işi (içe aktarma sonrası).
pub const JOB_PARSE_DOCUMENT: &str = "parse_document";

/// Varsayılan kira süresi: bir ayrıştırma işi 5 dakikada bitmezse sahiplik
/// düşer. Uzun belgelerde çağıran daha büyük değer verir.
pub const DEFAULT_LEASE_SECONDS: i64 = 300;

/// Bir işin durumu (tablodaki CHECK kümesiyle birebir).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub id: String,
    pub job_type: String,
    pub scope: String,
    pub case_id: Option<String>,
    pub state: String,
    pub priority: i64,
    pub attempt_count: i64,
    pub max_attempts: i64,
    pub progress: f64,
    pub next_run_at: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_until: Option<String>,
    pub cancel_requested_at: Option<String>,
    pub error_code: Option<String>,
    /// `running → blocked` geçişinde yazılan engel nedeni (parola, kota, izin,
    /// kasa kilidi). Hata kodu değildir.
    pub blocked_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStateCount {
    pub state: String,
    pub count: i64,
}

/// Kuyruk özeti: arayüz yalnız bunu gösterir (iş kimlikleri değil).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobQueueSummary {
    pub total: i64,
    pub by_state: Vec<JobStateCount>,
    /// Şu an sahiplenilebilir iş sayısı (durum `queued`, zamanı gelmiş).
    pub due_now: i64,
    /// En eski bekleyen işin oluşturulma zamanı; yoksa `None`.
    pub oldest_queued_at: Option<String>,
    /// Bağımlılığı bitmiş/iptal olmuş ya da kaybolmuş işler: sessizce
    /// beklemezler, sahiplenme sırasında `failed` işaretlenirler ve burada
    /// sayılırlar.
    pub blocked_by_dependency: i64,
}

/// Kurtarma raporu (açılışta ya da elle). §17.6 iki adımlıdır: süresi dolmuş
/// iş **önce** `interrupted` olur, sonra yeniden doğrulama sonrası sıraya ya da
/// hataya düşer. Sayımlar bu iki adımı ayrı gösterir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecovery {
    /// Kesintiye uğradığı tespit edilen iş sayısı (`→ interrupted`).
    pub interrupted: usize,
    /// Yeniden sıraya alınan iş sayısı (`interrupted → queued`).
    pub requeued: usize,
    /// Tekrar bütçesi bittiği için kapatılan iş sayısı (`interrupted → failed`).
    pub failed: usize,
    /// İptal isteği gelmiş ve yarıda kalmış, kapatılan iş sayısı.
    pub canceled: usize,
    /// Yeniden sıraya alınan işlerin kimlikleri (içerik taşımaz).
    pub interrupted_job_ids: Vec<String>,
}

/// İptal sonucu: `cancelled` iş bitti, `requested` yürüyen işe işaret kondu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobCancelResult {
    pub outcome: String,
    pub job: JobRow,
}

const JOB_COLUMNS: &str = "id,type,scope,case_id,state,priority,attempt_count,retry_policy,progress,next_run_at,lease_owner,lease_until,cancel_requested_at,error_code,created_at,updated_at,completed_at,blocked_reason";

fn map_job(row: &Row<'_>) -> rusqlite::Result<JobRow> {
    let retry_policy: String = row.get(7)?;
    let max_attempts = max_attempts_of(&retry_policy);
    Ok(JobRow {
        id: row.get(0)?,
        job_type: row.get(1)?,
        scope: row.get(2)?,
        case_id: row.get(3)?,
        state: row.get(4)?,
        priority: row.get(5)?,
        attempt_count: row.get(6)?,
        max_attempts,
        progress: row.get(8)?,
        next_run_at: row.get(9)?,
        lease_owner: row.get(10)?,
        lease_until: row.get(11)?,
        cancel_requested_at: row.get(12)?,
        error_code: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        completed_at: row.get(16)?,
        blocked_reason: row.get(17)?,
    })
}

/// Durum makinesi kapısı: tabloda olmayan ok `E_JOB_STATE` ile reddedilir.
/// SQL'deki `WHERE state IN (...)` yarışa karşı ikinci kapıdır; kural tek yerde.
fn ensure_transition(from: &str, to: &str) -> Result<(), StorageError> {
    if job_state::allowed(from, to) {
        Ok(())
    } else {
        Err(StorageError::JobState(format!(
            "{from} durumundan {to} durumuna geçiş şartname §17.2'de yok"
        )))
    }
}

/// Sahiplenme kapısı: `queued → running` tek ok; `retry_wait` ise
/// `retry_wait → queued → running` iki okun **aynı işlemde** tüketilmesidir
/// (tabloda doğrudan `retry_wait → running` yok).
fn ensure_claimable(from: &str) -> Result<(), StorageError> {
    if from == "queued" {
        return ensure_transition("queued", "running");
    }
    ensure_transition(from, "queued")?;
    ensure_transition("queued", "running")
}

/// Yeniden deneme bütçesi ve artan bekleme.
///
/// `dim-jobs::RetryPolicy` ile **aynı** formül (2 s tabanlı üstel, üst sınırlı);
/// burada yineleniyor çünkü depolama katmanı yalnız şemasını bildiği `jobs`
/// satırını okuyor ve `dim-jobs`'a crate bağımlılığı eklemek `Cargo.lock`
/// üzerinde başka ajanların bekleyen değişiklikleriyle çakışıyordu. İki
/// uygulama ayrışırsa `retry_backoff_matches_the_documented_policy` testi
/// yakalar (sabitler ve formül testte yazılı).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RetryBudget {
    max_attempts: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 2_000,
            max_delay_ms: 300_000,
        }
    }
}

impl RetryBudget {
    fn should_retry(self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }

    fn delay_for(self, attempt: u32) -> std::time::Duration {
        if attempt == 0 {
            return std::time::Duration::ZERO;
        }
        let exponent = attempt.saturating_sub(1).min(16);
        let scaled = self.base_delay_ms.saturating_mul(1u64 << exponent);
        std::time::Duration::from_millis(scaled.min(self.max_delay_ms))
    }
}

/// `retry_policy` JSON'u; eksik alanlar varsayılana düşer (eski satırlar
/// `maxDelayMs` taşımıyor).
fn retry_policy_of(raw: &str) -> RetryBudget {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap_or_default();
    let mut policy = RetryBudget::default();
    if let Some(v) = value.get("maxAttempts").and_then(serde_json::Value::as_u64) {
        policy.max_attempts = u32::try_from(v).unwrap_or(policy.max_attempts);
    }
    if let Some(v) = value.get("baseDelayMs").and_then(serde_json::Value::as_u64) {
        policy.base_delay_ms = v;
    }
    if let Some(v) = value.get("maxDelayMs").and_then(serde_json::Value::as_u64) {
        policy.max_delay_ms = v;
    }
    policy
}

fn max_attempts_of(raw: &str) -> i64 {
    i64::from(retry_policy_of(raw).max_attempts)
}

/// Çağıranın saat enjekte etmesi gerekmediğinde kullanılan "şimdi".
pub fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn iso(at: OffsetDateTime) -> String {
    at.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn parse_ts(text: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(text, &Rfc3339).ok()
}

fn lease_expired(until: Option<&str>, now: OffsetDateTime) -> bool {
    match until {
        // Kira zamanı okunamıyorsa "dolmuş" sayılır: başka süreç devralabilir,
        // sessizce sahiplenilmiş görünmez.
        Some(text) => parse_ts(text).is_none_or(|at| now >= at),
        None => false,
    }
}

impl Database {
    /// İşi kuyruğa ekler; `priority` yüksek olan önce sahiplenilir.
    pub fn enqueue_job_with_priority(
        &self,
        job_type: &str,
        case_id: Option<&str>,
        input_fingerprint: &str,
        idempotency_key: &str,
        priority: i64,
    ) -> Result<String, StorageError> {
        if let Some(id) = self.job_id_by_key(idempotency_key)? {
            return Ok(id);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        let scope = if case_id.is_some() { "case" } else { "vault" };
        self.conn().execute(
            "INSERT INTO jobs (id,type,scope,case_id,input_fingerprint,idempotency_key,state,priority,progress,retry_policy,attempt_count,next_run_at,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,'queued',?7,0,'{\"maxAttempts\":3,\"baseDelayMs\":2000,\"maxDelayMs\":300000}',0,?8,?8,?8)",
            params![id, job_type, scope, case_id, input_fingerprint, idempotency_key, priority, now],
        )?;
        Ok(id)
    }

    pub fn job_id_by_key(&self, idempotency_key: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id FROM jobs WHERE idempotency_key=?1",
                [idempotency_key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Option<JobRow>, StorageError> {
        let mut stmt = self
            .conn()
            .prepare(&format!("SELECT {JOB_COLUMNS} FROM jobs WHERE id=?1"))?;
        let mut rows = stmt.query([job_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(map_job(row)?)),
            None => Ok(None),
        }
    }

    pub fn job_by_key(&self, idempotency_key: &str) -> Result<Option<JobRow>, StorageError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE idempotency_key=?1"
        ))?;
        let mut rows = stmt.query([idempotency_key])?;
        match rows.next()? {
            Some(row) => Ok(Some(map_job(row)?)),
            None => Ok(None),
        }
    }

    /// Zamanı gelmiş işleri **tek işlemde** sahiplenir: durum `running`, kira
    /// yazılır, `attempt_count` artar, `job_attempts` satırı açılır.
    ///
    /// Sıra: öncelik (yüksek önce) → oluşturulma (eski önce). Bağımlılığı
    /// bitmemiş iş `blocked` işaretlenir (şartname §17.2: "bağımlılık bekliyor");
    /// bağımlılığı **başarısız/iptal/kayıp** olan iş `canceled` olur — hiç
    /// çalışmayacak iş `failed` diye yazılmaz, "yarım kaldı" görüntüsü vermez.
    /// Bu tur başında bağımlılığı çözülmüş `blocked` işler yeniden `queued` olur.
    pub fn claim_jobs(
        &self,
        owner: &str,
        limit: usize,
        lease_seconds: i64,
        job_types: &[&str],
        now: OffsetDateTime,
    ) -> Result<Vec<JobRow>, StorageError> {
        if limit == 0 || owner.trim().is_empty() {
            return Ok(Vec::new());
        }
        let now_text = iso(now);
        let until_text = iso(now + time::Duration::seconds(lease_seconds.max(1)));
        let scan_limit = i64::try_from(limit.saturating_mul(4).min(400)).unwrap_or(400);
        let tx = self.conn().unchecked_transaction()?;
        release_dependency_blocked(&tx, &now_text)?;
        let mut claimed: Vec<JobRow> = Vec::new();
        {
            // Durum başına tek eşitlik: `state IN (...)` yazıldığında SQLite
            // sıralamayı indeksten karşılayamıyor ve her sahiplenmede geçici
            // B-tree kuruyordu (plan testi bunu tutuyor). İki sorgunun sonucu
            // Rust'ta (öncelik, yaş) sırasına dizilir.
            let mut candidates: Vec<(JobRow, String)> = Vec::new();
            for state in job_state::CLAIMABLE {
                let mut stmt = tx.prepare(&format!(
                    "SELECT {JOB_COLUMNS},dependency_ids FROM jobs INDEXED BY idx_jobs_claim
                     WHERE state=?1 AND next_run_at<=?2
                     ORDER BY priority DESC, created_at ASC
                     LIMIT ?3"
                ))?;
                let rows = stmt
                    .query_map(params![state, now_text, scan_limit], |row| {
                        let job = map_job(row)?;
                        let deps: String = row.get(18)?;
                        Ok((job, deps))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                candidates.extend(rows);
            }
            candidates.sort_by(|a, b| {
                b.0.priority
                    .cmp(&a.0.priority)
                    .then_with(|| a.0.created_at.cmp(&b.0.created_at))
            });

            for (job, deps_raw) in candidates {
                if claimed.len() >= limit {
                    break;
                }
                if !job_types.is_empty() && !job_types.iter().any(|t| *t == job.job_type) {
                    continue;
                }
                ensure_transition(&job.state, "canceled")?;
                if job.cancel_requested_at.is_some() {
                    tx.execute(
                        "UPDATE jobs SET state='canceled', error_code='E_JOB_CANCELLED', completed_at=?2, updated_at=?2
                         WHERE id=?1 AND state IN ('queued','retry_wait')",
                        params![job.id, now_text],
                    )?;
                    continue;
                }
                match dependency_state(&tx, &deps_raw)? {
                    DependencyState::Ready => {}
                    DependencyState::Waiting => {
                        // `queued → blocked`: bağımlılık bekliyor. `claim_jobs`
                        // başında çözülenler geri açılır; sonsuz bekleme yok.
                        ensure_transition(&job.state, "blocked")?;
                        tx.execute(
                            "UPDATE jobs SET state='blocked', blocked_reason='dependency',
                                    next_run_at=?2, updated_at=?2, error_code='E_JOB_DEPENDENCY_WAIT'
                             WHERE id=?1 AND state IN ('queued','retry_wait')",
                            params![job.id, now_text],
                        )?;
                        continue;
                    }
                    DependencyState::Aborted(code) => {
                        ensure_transition(&job.state, "canceled")?;
                        tx.execute(
                            "UPDATE jobs SET state='canceled', error_code=?2, completed_at=?3, updated_at=?3
                             WHERE id=?1 AND state IN ('queued','retry_wait')",
                            params![job.id, code, now_text],
                        )?;
                        continue;
                    }
                }
                ensure_claimable(&job.state)?;
                let changed = tx.execute(
                    "UPDATE jobs SET state='running', lease_owner=?2, lease_until=?3,
                            attempt_count=attempt_count+1, updated_at=?4
                     WHERE id=?1 AND state IN ('queued','retry_wait')",
                    params![job.id, owner, until_text, now_text],
                )?;
                if changed != 1 {
                    continue;
                }
                let attempt_no = job.attempt_count + 1;
                tx.execute(
                    "INSERT INTO job_attempts (id, job_id, attempt_no, started_at)
                     VALUES (?1,?2,(SELECT COALESCE(MAX(attempt_no),0)+1 FROM job_attempts WHERE job_id=?2),?3)",
                    params![uuid::Uuid::now_v7().to_string(), job.id, now_text],
                )?;
                claimed.push(JobRow {
                    state: "running".to_string(),
                    attempt_count: attempt_no,
                    lease_owner: Some(owner.to_string()),
                    lease_until: Some(until_text.clone()),
                    updated_at: now_text.clone(),
                    ..job
                });
            }
        }
        tx.commit()?;
        Ok(claimed)
    }

    /// Tek bir işi anahtarına göre sahiplenir (belge ayrıştırma yolu).
    /// İş hazır değilse `None` döner; hata değildir.
    pub fn claim_job_by_key(
        &self,
        idempotency_key: &str,
        owner: &str,
        lease_seconds: i64,
        now: OffsetDateTime,
    ) -> Result<Option<JobRow>, StorageError> {
        let Some(job) = self.job_by_key(idempotency_key)? else {
            return Ok(None);
        };
        if !job_state::CLAIMABLE.contains(&job.state.as_str()) {
            return Ok(None);
        }
        if let Some(next) = job.next_run_at.as_deref().and_then(parse_ts) {
            if next > now {
                return Ok(None);
            }
        }
        let now_text = iso(now);
        let until_text = iso(now + time::Duration::seconds(lease_seconds.max(1)));
        let tx = self.conn().unchecked_transaction()?;
        release_dependency_blocked(&tx, &now_text)?;
        ensure_claimable(&job.state)?;
        let changed = tx.execute(
            "UPDATE jobs SET state='running', lease_owner=?2, lease_until=?3,
                    attempt_count=attempt_count+1, updated_at=?4
             WHERE id=?1 AND state IN ('queued','retry_wait')",
            params![job.id, owner, until_text, now_text],
        )?;
        if changed != 1 {
            return Ok(None);
        }
        tx.execute(
            "INSERT INTO job_attempts (id, job_id, attempt_no, started_at)
             VALUES (?1,?2,(SELECT COALESCE(MAX(attempt_no),0)+1 FROM job_attempts WHERE job_id=?2),?3)",
            params![uuid::Uuid::now_v7().to_string(), job.id, now_text],
        )?;
        tx.commit()?;
        self.get_job(&job.id)
    }

    /// `running → validating`: çıktı şema/hash/kapsam denetimine alındı.
    ///
    /// İptal isteği varsa yürüyen iş doğrudan `canceling` olur (şartname §17.2):
    /// yeni bir doğrulama başlatılmaz.
    pub fn begin_validation(
        &self,
        job_id: &str,
        owner: &str,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        let job = self.require_leased(job_id, owner, now)?;
        let now_text = iso(now);
        let target = if job.cancel_requested_at.is_some() {
            "canceling"
        } else {
            "validating"
        };
        ensure_transition(&job.state, target)?;
        let changed = self.conn().execute(
            "UPDATE jobs SET state=?2, updated_at=?3 WHERE id=?1 AND state='running'",
            params![job_id, target, now_text],
        )?;
        if changed != 1 {
            return Err(StorageError::JobState(
                "iş bu arada durum değiştirdi; doğrulama başlatılamadı".to_string(),
            ));
        }
        self.get_job(job_id)?.ok_or(StorageError::NotFound)
    }

    /// Kira sahibi + süre denetimi (bitirme yollarının ortak kapısı).
    fn require_leased(
        &self,
        job_id: &str,
        owner: &str,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        let job = self.get_job(job_id)?.ok_or(StorageError::NotFound)?;
        if job.lease_owner.as_deref() != Some(owner) {
            return Err(StorageError::JobLease(
                "kira sahibi bu süreç değil".to_string(),
            ));
        }
        if lease_expired(job.lease_until.as_deref(), now) {
            return Err(StorageError::JobLease("kira süresi dolmuş".to_string()));
        }
        Ok(job)
    }

    /// İşi başarıyla bitirir. Kira sahibi ve süresi denetlenir; iptal isteği
    /// varsa sonuç `completed` değil `canceled` olur (istek sessizce yutulmaz).
    ///
    /// `running` durumundan çağrıldığında `running → validating → completed`
    /// iki geçişi **tek işlemde** yazar (şartname §17.2'de `running → completed`
    /// yoktur). Çağıran uzun bir doğrulama yapacaksa önce `begin_validation`
    /// çağırır; sonuç yine bu yolla yazılır.
    pub fn complete_job(
        &self,
        job_id: &str,
        owner: &str,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        self.finish_job(job_id, owner, now, None, false)
    }

    /// Doğrulama sonucu **kısmi**: bazı sayfalar/alt işler tamam, açık eksik
    /// listesi var. `validating → partial` (terminal). `completed` demek
    /// "her şey doğrulandı" demek olurdu; yarım çıktı tam sayılmaz.
    pub fn complete_job_partial(
        &self,
        job_id: &str,
        owner: &str,
        error_code: &str,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        self.apply_finish(job_id, owner, now, FinishKind::Partial(error_code), false)
    }

    /// İşi hatayla bitirir. `retryable` ve deneme bütçesi uygunsa iş
    /// `retry_wait` + artan bekleme ile yeniden sıraya girer; değilse `failed`.
    /// Doğrulama aşamasındaki hata **tekrar denenmez** (şartname §17.2:
    /// `validating` yalnız `failed`e gider), çünkü çıktı üretilmiştir ve aynı
    /// girdiyle tekrar denenmesi yeni bir sonuç doğurmaz.
    pub fn fail_job(
        &self,
        job_id: &str,
        owner: &str,
        error_code: &str,
        retryable: bool,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        self.finish_job(job_id, owner, now, Some(error_code), retryable)
    }

    fn finish_job(
        &self,
        job_id: &str,
        owner: &str,
        now: OffsetDateTime,
        error_code: Option<&str>,
        retryable: bool,
    ) -> Result<JobRow, StorageError> {
        let kind = match error_code {
            Some(code) => FinishKind::Failure(code),
            None => FinishKind::Success,
        };
        self.apply_finish(job_id, owner, now, kind, retryable)
    }

    fn apply_finish(
        &self,
        job_id: &str,
        owner: &str,
        now: OffsetDateTime,
        kind: FinishKind<'_>,
        retryable: bool,
    ) -> Result<JobRow, StorageError> {
        let job = self.require_leased(job_id, owner, now)?;
        if !matches!(job.state.as_str(), "running" | "validating" | "canceling") {
            return Err(StorageError::JobState(format!(
                "iş {} durumunda; yalnız yürüyen/doğrulanan iş bitirilebilir",
                job.state
            )));
        }
        let now_text = iso(now);
        let policy = retry_policy_of(
            &self
                .conn()
                .query_row("SELECT retry_policy FROM jobs WHERE id=?1", [job_id], |r| {
                    r.get::<_, String>(0)
                })
                .optional()?
                .unwrap_or_default(),
        );

        // İptal isteği her sonucun önüne geçer: `canceling → canceled`.
        let cancelled = job.cancel_requested_at.is_some() || job.state == "canceling";
        let (state, code, next_run_at) = if cancelled {
            ("canceled", Some("E_JOB_CANCELLED"), None)
        } else {
            match kind {
                FinishKind::Success => ("completed", None, None),
                FinishKind::Partial(code) => ("partial", Some(code), None),
                FinishKind::Failure(code) => {
                    let attempt = u32::try_from(job.attempt_count).unwrap_or(u32::MAX);
                    // `validating → retry_wait` tabloda yok: doğrulama hatası
                    // terminaldir (çıktı üretilmiştir, aynı girdi aynı sonucu verir).
                    if retryable && job.state == "running" && policy.should_retry(attempt) {
                        let delay = policy.delay_for(attempt);
                        let ms = i64::try_from(delay.as_millis()).unwrap_or(i64::MAX);
                        (
                            "retry_wait",
                            Some(code),
                            Some(iso(now + time::Duration::milliseconds(ms))),
                        )
                    } else {
                        ("failed", Some(code), None)
                    }
                }
            }
        };
        // Yol denetimi: atlanan ara adımlar da şartnamede olmalı. `running`den
        // başarı/kısmilik `validating` üzerinden gider; iptal `canceling`
        // üzerinden. Diğer oklar tabloda doğrudan var.
        let hops: &[&str] = match (job.state.as_str(), state) {
            ("running", "completed") => &["validating", "completed"],
            ("running", "partial") => &["validating", "partial"],
            ("running", "canceled") => &["canceling", "canceled"],
            ("validating", "canceled") => &["canceling", "canceled"],
            ("running", to) | ("validating", to) | ("canceling", to) => &[to],
            _ => &[],
        };
        let mut from = job.state.clone();
        for hop in hops {
            ensure_transition(&from, hop)?;
            from = (*hop).to_string();
        }
        let completed_at = next_run_at.is_none().then(|| now_text.clone());
        let progress = if state == "completed" {
            1.0
        } else {
            job.progress
        };
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE jobs SET state=?2, error_code=NULLIF(?3,''), next_run_at=?4, completed_at=?5,
                    progress=?6, lease_owner=NULL, lease_until=NULL, updated_at=?7
             WHERE id=?1 AND state IN ('running','validating','canceling')",
            params![
                job_id,
                state,
                code.unwrap_or(""),
                next_run_at,
                completed_at,
                progress,
                now_text
            ],
        )?;
        tx.execute(
            "UPDATE job_attempts SET ended_at=?2, error_code=NULLIF(?3,'')
             WHERE job_id=?1 AND ended_at IS NULL",
            params![job_id, now_text, code.unwrap_or("")],
        )?;
        tx.commit()?;
        self.get_job(job_id)?.ok_or(StorageError::NotFound)
    }

    /// `running → blocked`: iş engellendi (parola, kota, izin, kasa kilidi).
    /// Kira bırakılır, neden `blocked_reason`a yazılır; iş yeniden `queued`
    /// olana kadar sahiplenilmez.
    pub fn block_job(
        &self,
        job_id: &str,
        owner: &str,
        reason: &str,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        let job = self.require_leased(job_id, owner, now)?;
        ensure_transition(&job.state, "blocked")?;
        let now_text = iso(now);
        let changed = self.conn().execute(
            "UPDATE jobs SET state='blocked', blocked_reason=?2, lease_owner=NULL,
                    lease_until=NULL, updated_at=?3
             WHERE id=?1 AND state='running'",
            params![job_id, reason, now_text],
        )?;
        if changed != 1 {
            return Err(StorageError::JobState(
                "iş bu arada durum değiştirdi; engellenemedi".to_string(),
            ));
        }
        self.get_job(job_id)?.ok_or(StorageError::NotFound)
    }

    /// `blocked → queued`: engel kalktı (parola girildi, kota açıldı, kasa
    /// açıldı). İş hemen sahiplenilebilir olur; engel nedeni kayıt için kalır.
    pub fn unblock_job(&self, job_id: &str, now: OffsetDateTime) -> Result<JobRow, StorageError> {
        let job = self.get_job(job_id)?.ok_or(StorageError::NotFound)?;
        ensure_transition(&job.state, "queued")?;
        let now_text = iso(now);
        let changed = self.conn().execute(
            "UPDATE jobs SET state='queued', error_code=NULL, next_run_at=?2, updated_at=?2
             WHERE id=?1 AND state='blocked'",
            params![job_id, now_text],
        )?;
        if changed != 1 {
            return Err(StorageError::JobState(
                "iş bu arada durum değiştirdi; engel kaldırılamadı".to_string(),
            ));
        }
        self.get_job(job_id)?.ok_or(StorageError::NotFound)
    }

    /// `failed → queued`: kullanıcı yeniden denedi. Deneme sayacı **sıfırlanır**
    /// (yeni bütçe); `job_attempts` satırları silinmez, numaralandırma
    /// `MAX(attempt_no)+1` ile sürer — geçmiş kaybolmaz.
    pub fn retry_failed_job(
        &self,
        job_id: &str,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        let job = self.get_job(job_id)?.ok_or(StorageError::NotFound)?;
        ensure_transition(&job.state, "queued")?;
        let now_text = iso(now);
        let changed = self.conn().execute(
            "UPDATE jobs SET state='queued', error_code=NULL, blocked_reason=NULL,
                    attempt_count=0, next_run_at=?2, completed_at=NULL, updated_at=?2
             WHERE id=?1 AND state='failed'",
            params![job_id, now_text],
        )?;
        if changed != 1 {
            return Err(StorageError::JobState(
                "iş bu arada durum değiştirdi; yeniden denenemedi".to_string(),
            ));
        }
        self.get_job(job_id)?.ok_or(StorageError::NotFound)
    }

    /// İptal isteği. Sırada bekleyen/engellenen iş hemen `canceled` olur;
    /// yürüyen ya da doğrulanan iş `canceling` işaretine geçer
    /// (`cancelRequestedAt` yazılır) ve yürütücü bitirince `canceled` olur —
    /// süreç gerçekten durmadan "iptal tamam" denmez (şartname §17.5).
    pub fn cancel_job(
        &self,
        job_id: &str,
        now: OffsetDateTime,
    ) -> Result<JobCancelResult, StorageError> {
        let job = self.get_job(job_id)?.ok_or(StorageError::NotFound)?;
        let now_text = iso(now);
        match job.state.as_str() {
            "queued" | "retry_wait" | "blocked" => {
                ensure_transition(&job.state, "canceled")?;
                self.conn().execute(
                    "UPDATE jobs SET state='canceled', error_code='E_JOB_CANCELLED',
                            completed_at=?2, lease_owner=NULL, lease_until=NULL, updated_at=?2
                     WHERE id=?1 AND state IN ('queued','retry_wait','blocked')",
                    params![job_id, now_text],
                )?;
                Ok(JobCancelResult {
                    outcome: "canceled".to_string(),
                    job: self.get_job(job_id)?.ok_or(StorageError::NotFound)?,
                })
            }
            "running" | "validating" => {
                ensure_transition(&job.state, "canceling")?;
                self.conn().execute(
                    "UPDATE jobs SET state='canceling',
                            cancel_requested_at=COALESCE(cancel_requested_at,?2), updated_at=?2
                     WHERE id=?1 AND state IN ('running','validating')",
                    params![job_id, now_text],
                )?;
                Ok(JobCancelResult {
                    outcome: "requested".to_string(),
                    job: self.get_job(job_id)?.ok_or(StorageError::NotFound)?,
                })
            }
            // Zaten iptal istenmiş yürüyen iş: tekrar istek zararsız.
            "canceling" => Ok(JobCancelResult {
                outcome: "requested".to_string(),
                job,
            }),
            other => Err(StorageError::JobState(format!(
                "iş {other} durumunda; iptal edilemez"
            ))),
        }
    }

    /// Kirası dolmuş işleri kurtarır (şartname §17.6 iki adım):
    ///
    /// 1. `running|validating|canceling` + süresi dolmuş → **önce** `interrupted`,
    /// 2. yeniden doğrulama: iptal isteği olan `canceled`, tekrar bütçesi kalan
    ///    `queued` (yeniden sıraya), bütçesi biten `failed`.
    ///
    /// Deneme sayısı artmaz (kesinti başarısız deneme değildir) ve bütçe
    /// sıfırlanmaz. "Yeniden doğrulama" bugün **girdi parmak izinin değişmediği**
    /// varsayımıdır: yerel deterministik iş (ayrıştırma girdisi kasadaki blob +
    /// ayarlardır). Uzak (AI) işlerinde otomatik yeniden gönderim yoktur.
    pub fn recover_stale_jobs(&self, now: OffsetDateTime) -> Result<JobRecovery, StorageError> {
        let now_text = iso(now);
        let tx = self.conn().unchecked_transaction()?;
        let stale_states = "('running','validating','canceling')";
        let mut stmt = tx.prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE state IN {stale_states}"
        ))?;
        let jobs: Vec<JobRow> = stmt
            .query_map([], map_job)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut interrupted_ids = Vec::new();
        let mut requeued = 0usize;
        let mut failed = 0usize;
        let mut canceled = 0usize;
        for job in jobs {
            if !lease_expired(job.lease_until.as_deref(), now) {
                continue;
            }
            // İptal istenmiş iş kesintiye uğramış sayılmaz: `canceling → canceled`.
            // (`interrupted → canceled` tabloda yok; iptal yolu kendi yolundan gider.)
            if job.cancel_requested_at.is_some() {
                ensure_transition(
                    &job.state,
                    if job.state == "canceling" {
                        "canceled"
                    } else {
                        "canceling"
                    },
                )?;
                ensure_transition("canceling", "canceled")?;
                tx.execute(
                    "UPDATE jobs SET state='canceled', error_code='E_JOB_CANCELLED',
                            completed_at=?2, lease_owner=NULL, lease_until=NULL, updated_at=?2
                     WHERE id=?1",
                    params![job.id, now_text],
                )?;
                tx.execute(
                    "UPDATE job_attempts SET ended_at=?2, error_code='E_JOB_CANCELLED'
                     WHERE job_id=?1 AND ended_at IS NULL",
                    params![job.id, now_text],
                )?;
                canceled += 1;
                continue;
            }
            // 1) kesinti: `→ interrupted`
            ensure_transition(&job.state, "interrupted")?;
            tx.execute(
                "UPDATE jobs SET state='interrupted', error_code='E_JOB_INTERRUPTED',
                        lease_owner=NULL, lease_until=NULL, updated_at=?2
                 WHERE id=?1",
                params![job.id, now_text],
            )?;
            tx.execute(
                "UPDATE job_attempts SET ended_at=?2, error_code='E_JOB_INTERRUPTED'
                 WHERE job_id=?1 AND ended_at IS NULL",
                params![job.id, now_text],
            )?;
            interrupted_ids.push(job.id.clone());

            // 2) yeniden doğrulama sonrası karar: bütçe varsa sıraya, yoksa hata.
            let policy = retry_policy_of(&self.retry_policy_raw(&job.id)?);
            let attempt = u32::try_from(job.attempt_count).unwrap_or(u32::MAX);
            if policy.should_retry(attempt) {
                ensure_transition("interrupted", "queued")?;
                tx.execute(
                    "UPDATE jobs SET state='queued', error_code=NULL, next_run_at=?2, updated_at=?2
                     WHERE id=?1 AND state='interrupted'",
                    params![job.id, now_text],
                )?;
                requeued += 1;
            } else {
                ensure_transition("interrupted", "failed")?;
                tx.execute(
                    "UPDATE jobs SET state='failed', error_code='E_JOB_RETRIES',
                            completed_at=?2, updated_at=?2
                     WHERE id=?1 AND state='interrupted'",
                    params![job.id, now_text],
                )?;
                failed += 1;
            }
        }
        tx.commit()?;
        Ok(JobRecovery {
            interrupted: interrupted_ids.len(),
            requeued,
            failed,
            canceled,
            interrupted_job_ids: interrupted_ids,
        })
    }

    fn retry_policy_raw(&self, job_id: &str) -> Result<String, StorageError> {
        Ok(self
            .conn()
            .query_row("SELECT retry_policy FROM jobs WHERE id=?1", [job_id], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .unwrap_or_default())
    }

    /// Kuyruk özeti. Sayımlar tablodan; hiçbir alan tahmin edilmez.
    pub fn job_queue_summary(&self) -> Result<JobQueueSummary, StorageError> {
        let now_text = now_iso();
        let mut stmt = self
            .conn()
            .prepare("SELECT state, COUNT(*) FROM jobs GROUP BY state ORDER BY state")?;
        let by_state: Vec<JobStateCount> = stmt
            .query_map([], |row| {
                Ok(JobStateCount {
                    state: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        let total = by_state.iter().map(|s| s.count).sum();
        // Sahiplenilebilir durumlar tablodan gelir (tek kaynak), sorgu durum
        // başına tek eşitlik kullanır: `state IN (...)` indeksi kullanmıyordu.
        let mut due_now = 0i64;
        for state in job_state::CLAIMABLE {
            due_now += self.conn().query_row(
                "SELECT COUNT(*) FROM jobs WHERE state=?1 AND (next_run_at IS NULL OR next_run_at<=?2)",
                params![state, &now_text],
                |r| r.get::<_, i64>(0),
            )?;
        }
        let oldest_queued_at: Option<String> = self
            .conn()
            .query_row(
                "SELECT MIN(created_at) FROM jobs WHERE state IN ('queued','retry_wait')",
                [],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        // Bağımlılığı düşen iş artık `canceled` (hiç çalışmadı); sayı yine
        // görünür olsun diye hata kodu üzerinden sayılır.
        let blocked_by_dependency: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM jobs WHERE state='canceled' AND error_code IN ('E_JOB_DEPENDENCY_ABORTED','E_JOB_DEPENDENCY_MISSING')",
            [],
            |r| r.get(0),
        )?;
        Ok(JobQueueSummary {
            total,
            by_state,
            due_now,
            oldest_queued_at,
            blocked_by_dependency,
        })
    }

    /// Arayüz listesi: en yeni işler. Dosya adı/metin taşımaz.
    pub fn list_jobs(&self, limit: usize) -> Result<Vec<JobRow>, StorageError> {
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(20);
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs ORDER BY created_at DESC, id DESC LIMIT ?1"
        ))?;
        let rows = stmt
            .query_map([limit], map_job)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

enum DependencyState {
    Ready,
    Waiting,
    Aborted(&'static str),
}

/// Bitirme türü: başarı, kısmi başarı (açık eksik listesi), hata.
#[derive(Debug, Clone, Copy)]
enum FinishKind<'a> {
    Success,
    Partial(&'a str),
    Failure(&'a str),
}

/// Bağımlılığı çözülmüş `blocked` işleri açar, bağımlılığı düşmüş olanları
/// kapatır. `blocked_reason='dependency'` olanlar taranır: engel nedeni başka
/// olan iş (parola/kota) kendiliğinden açılmaz — kullanıcı `unblock_job` çağırır.
fn release_dependency_blocked(
    tx: &rusqlite::Transaction<'_>,
    now_text: &str,
) -> Result<(), StorageError> {
    let mut stmt = tx.prepare(&format!(
        "SELECT {JOB_COLUMNS},dependency_ids FROM jobs WHERE state='blocked' AND blocked_reason='dependency'"
    ))?;
    let rows: Vec<(JobRow, String)> = stmt
        .query_map([], |row| {
            let job = map_job(row)?;
            let deps: String = row.get(18)?;
            Ok((job, deps))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for (job, deps) in rows {
        match dependency_state(tx, &deps)? {
            DependencyState::Waiting => {}
            DependencyState::Ready => {
                ensure_transition("blocked", "queued")?;
                tx.execute(
                    "UPDATE jobs SET state='queued', error_code=NULL, next_run_at=?2, updated_at=?2
                     WHERE id=?1 AND state='blocked'",
                    params![job.id, now_text],
                )?;
            }
            DependencyState::Aborted(code) => {
                ensure_transition("blocked", "canceled")?;
                tx.execute(
                    "UPDATE jobs SET state='canceled', error_code=?2, completed_at=?3, updated_at=?3
                     WHERE id=?1 AND state='blocked'",
                    params![job.id, code, now_text],
                )?;
            }
        }
    }
    Ok(())
}

fn dependency_state(
    tx: &rusqlite::Transaction<'_>,
    raw: &str,
) -> Result<DependencyState, StorageError> {
    let ids: Vec<String> = serde_json::from_str(raw).unwrap_or_default();
    if ids.is_empty() {
        return Ok(DependencyState::Ready);
    }
    let mut waiting = false;
    for id in ids {
        let state: Option<String> = tx
            .query_row("SELECT state FROM jobs WHERE id=?1", [&id], |r| r.get(0))
            .optional()?;
        match state.as_deref() {
            // `partial` çıktı üretir (eksik listesiyle) — bağımlı iş çalışabilir.
            Some("completed" | "partial") => {}
            Some("failed" | "canceled") => {
                return Ok(DependencyState::Aborted("E_JOB_DEPENDENCY_ABORTED"))
            }
            Some(_) => waiting = true,
            None => return Ok(DependencyState::Aborted("E_JOB_DEPENDENCY_MISSING")),
        }
    }
    Ok(if waiting {
        DependencyState::Waiting
    } else {
        DependencyState::Ready
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{AiPolicy, DatabaseKey, Sensitivity};

    fn db() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([7u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kuyruk".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: AiPolicy::Off,
            })
            .expect("case");
        (db, case.id)
    }

    fn enqueue(db: &Database, case_id: &str, key: &str, priority: i64) -> String {
        db.enqueue_job_with_priority(
            JOB_PARSE_DOCUMENT,
            Some(case_id),
            &format!("fp-{key}"),
            key,
            priority,
        )
        .expect("iş")
    }

    /// Test zamanı gerçek "şimdi"ye göre: `enqueue` işi gerçek saatle
    /// damgalıyor, sabit bir geçmiş an kullanmak işi "gelecekte" bırakırdı.
    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::now_utc() + time::Duration::seconds(seconds)
    }

    fn set_lease(db: &Database, job_id: &str, until: Option<OffsetDateTime>) {
        db.conn()
            .execute(
                "UPDATE jobs SET lease_until=?2 WHERE id=?1",
                params![job_id, until.map(iso)],
            )
            .expect("kira");
    }

    #[test]
    fn enqueue_is_idempotent_and_claim_advances_state() {
        let (db, case_id) = db();
        let first = enqueue(&db, &case_id, "parse:v1", 0);
        let second = enqueue(&db, &case_id, "parse:v1", 0);
        assert_eq!(first, second, "aynı anahtar ikinci satır açmamalı");
        assert_eq!(db.list_jobs(10).expect("liste").len(), 1);

        let claimed = db
            .claim_jobs("worker-a", 5, 60, &[JOB_PARSE_DOCUMENT], at(0))
            .expect("sahiplen");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].state, "running");
        assert_eq!(claimed[0].attempt_count, 1);
        assert_eq!(claimed[0].lease_owner.as_deref(), Some("worker-a"));
        // Aynı iş ikinci kez sahiplenilmez.
        assert!(db
            .claim_jobs("worker-b", 5, 60, &[], at(0))
            .expect("tekrar")
            .is_empty());

        let done = db.complete_job(&first, "worker-a", at(1)).expect("bitir");
        assert_eq!(done.state, "completed");
        assert_eq!(done.progress, 1.0);
        assert!(done.completed_at.is_some());
        assert!(done.lease_owner.is_none());

        let attempts: (i64, i64, Option<String>) = db
            .conn()
            .query_row(
                "SELECT COUNT(*), MAX(attempt_no), MAX(ended_at) FROM job_attempts WHERE job_id=?1",
                [&first],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("denemeler");
        assert_eq!(attempts.0, 1);
        assert_eq!(attempts.1, 1);
        assert!(attempts.2.is_some(), "deneme kaydı kapanmalı");
    }

    #[test]
    fn higher_priority_is_claimed_first_even_if_older_job_waits() {
        let (db, case_id) = db();
        let low = enqueue(&db, &case_id, "low", 0);
        let high = enqueue(&db, &case_id, "high", 50);
        let claimed = db.claim_jobs("w", 10, 60, &[], at(0)).expect("sahiplen");
        assert_eq!(claimed[0].id, high);
        assert_eq!(claimed[1].id, low);
    }

    #[test]
    fn dependency_waits_then_releases_and_aborts_when_parent_fails() {
        let (db, case_id) = db();
        let parent = enqueue(&db, &case_id, "parent", 0);
        let child = enqueue(&db, &case_id, "child", 0);
        let other = enqueue(&db, &case_id, "other", 0);
        db.conn()
            .execute(
                "UPDATE jobs SET dependency_ids=?2 WHERE id=?1",
                params![child, serde_json::json!([parent]).to_string()],
            )
            .expect("bağımlılık");
        db.conn()
            .execute(
                "UPDATE jobs SET dependency_ids=?2 WHERE id=?1",
                params![other, serde_json::json!([parent]).to_string()],
            )
            .expect("bağımlılık");

        // Ebeveyn sırada beklerken çocuk sahiplenilmez.
        let first = db.claim_jobs("w", 1, 60, &[], at(0)).expect("sahiplen");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, parent, "önce bağımlılık sahiplenilmeli");
        db.fail_job(&parent, "w", "E_PARSE", false, at(1))
            .expect("başarısız");

        // Ebeveyn başarısız olunca bağımlı işler beklemez, `canceled` olur
        // (hiç çalışmadılar: "failed" demek yarım kalmış iş görüntüsü verirdi).
        let claimed = db.claim_jobs("w", 10, 60, &[], at(2)).expect("sahiplen");
        assert!(claimed.is_empty());
        let child_row = db.get_job(&child).expect("oku").expect("var");
        assert_eq!(child_row.state, "canceled");
        assert_eq!(
            child_row.error_code.as_deref(),
            Some("E_JOB_DEPENDENCY_ABORTED")
        );
        let other_row = db.get_job(&other).expect("oku").expect("var");
        assert_eq!(other_row.state, "canceled");
        let summary = db.job_queue_summary().expect("özet");
        assert_eq!(summary.blocked_by_dependency, 2);
    }

    #[test]
    fn dependency_that_succeeds_releases_the_child() {
        let (db, case_id) = db();
        let parent = enqueue(&db, &case_id, "parent", 0);
        let child = enqueue(&db, &case_id, "child", 0);
        db.conn()
            .execute(
                "UPDATE jobs SET dependency_ids=?2 WHERE id=?1",
                params![child, serde_json::json!([parent]).to_string()],
            )
            .expect("bağımlılık");
        let claimed = db.claim_jobs("w", 10, 60, &[], at(0)).expect("sahiplen");
        assert_eq!(claimed.len(), 1);
        db.complete_job(&parent, "w", at(1)).expect("bitir");
        let claimed = db.claim_jobs("w", 10, 60, &[], at(2)).expect("sahiplen");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, child);
    }

    #[test]
    fn lease_owner_and_expiry_are_enforced() {
        let (db, case_id) = db();
        let job = enqueue(&db, &case_id, "j", 0);
        db.claim_jobs("worker-a", 1, 60, &[], at(0))
            .expect("sahiplen");
        let err = db
            .complete_job(&job, "worker-b", at(1))
            .expect_err("yabancı sahip bitirememeli");
        assert_eq!(err.code(), "E_JOB_LEASE");

        set_lease(&db, &job, Some(at(-5)));
        let err = db
            .complete_job(&job, "worker-a", at(0))
            .expect_err("dolmuş kira ile bitirilemez");
        assert_eq!(err.code(), "E_JOB_LEASE");
        // Kira dolduğunda iş hâlâ yürüyor görünür; kurtarma onu geri alır.
        assert_eq!(
            db.get_job(&job).expect("oku").expect("var").state,
            "running"
        );
    }

    #[test]
    fn cancel_is_immediate_when_queued_and_requested_when_running() {
        let (db, case_id) = db();
        let queued = enqueue(&db, &case_id, "q", 0);
        let result = db.cancel_job(&queued, at(0)).expect("iptal");
        assert_eq!(result.outcome, "canceled");
        assert_eq!(result.job.state, "canceled");
        assert!(result.job.completed_at.is_some());

        let running = enqueue(&db, &case_id, "r", 0);
        db.claim_jobs("w", 1, 60, &[], at(1)).expect("sahiplen");
        let result = db.cancel_job(&running, at(2)).expect("iptal isteği");
        assert_eq!(result.outcome, "requested");
        // `running → canceling`: süreç gerçekten durmadan "iptal tamam" denmez.
        assert_eq!(result.job.state, "canceling");
        assert!(result.job.cancel_requested_at.is_some());

        // Yürütücü bitirince sonuç `completed` değil `canceled` olur.
        let finished = db.complete_job(&running, "w", at(3)).expect("bitir");
        assert_eq!(finished.state, "canceled");
        assert_eq!(finished.error_code.as_deref(), Some("E_JOB_CANCELLED"));

        let err = db.cancel_job(&running, at(4)).expect_err("terminal iş");
        assert_eq!(err.code(), "E_JOB_STATE");
    }

    /// İptal edilen ayrıştırma `failed` değil `canceled` biter ve yeniden
    /// sıraya girmez: Tauri iptal yolunda `E_WORKER_CANCELED` ile `fail_job`
    /// çağırır (F04-T07); kullanıcının gördüğü durum bu yoldan gelir.
    #[test]
    fn failing_a_canceling_job_ends_canceled_without_retry() {
        let (db, case_id) = db();
        let job = enqueue(&db, &case_id, "parse", 0);
        db.claim_jobs("w", 1, 60, &[], at(0)).expect("sahiplen");
        db.cancel_job(&job, at(1)).expect("iptal isteği");
        let row = db
            .fail_job(&job, "w", "E_WORKER_CANCELED", false, at(2))
            .expect("iptal sonucu");
        assert_eq!(row.state, "canceled");
        assert_eq!(row.error_code.as_deref(), Some("E_JOB_CANCELLED"));
        assert!(
            row.next_run_at.is_none(),
            "iptal yeniden sıraya girmez (§17.4)"
        );
        assert!(row.completed_at.is_some());
    }

    #[test]
    fn recovery_requeues_interrupted_and_cancels_requested() {
        let (db, case_id) = db();
        let interrupted = enqueue(&db, &case_id, "i", 0);
        let cancelled = enqueue(&db, &case_id, "c", 0);
        let claimed = db.claim_jobs("w", 2, 60, &[], at(0)).expect("sahiplen");
        assert_eq!(claimed.len(), 2);
        set_lease(&db, &interrupted, Some(at(-10)));
        set_lease(&db, &cancelled, Some(at(-10)));
        db.conn()
            .execute(
                "UPDATE jobs SET cancel_requested_at=?2 WHERE id=?1",
                params![cancelled, iso(at(-1))],
            )
            .expect("iptal işareti");

        let report = db.recover_stale_jobs(at(0)).expect("kurtar");
        // §17.6 iki adım: önce `interrupted`, sonra yeniden doğrulama.
        assert_eq!(report.interrupted, 1);
        assert_eq!(report.requeued, 1);
        assert_eq!(report.failed, 0);
        assert_eq!(report.canceled, 1);
        assert_eq!(report.interrupted_job_ids, vec![interrupted.clone()]);

        let row = db.get_job(&interrupted).expect("oku").expect("var");
        // Bütçe kaldığı için yeniden sıraya alındı (`interrupted → queued`).
        assert_eq!(row.state, "queued");
        assert_eq!(
            row.error_code, None,
            "kesinti kodu başarısızlık gibi kalmaz"
        );
        assert!(row.lease_owner.is_none());
        // Kesinti deneme sayısını artırmaz, bütçeyi de sıfırlamaz.
        assert_eq!(row.attempt_count, 1);
        assert_eq!(
            db.get_job(&cancelled).expect("oku").expect("var").state,
            "canceled"
        );

        // Kurtarılan iş hemen yeniden sahiplenilebilir; numara sürer.
        let claimed = db
            .claim_jobs("w2", 1, 60, &[], at(1))
            .expect("yeniden sahiplen");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, interrupted);
        assert_eq!(claimed[0].attempt_count, 2);
        let attempt_nos: Vec<i64> = db
            .conn()
            .prepare("SELECT attempt_no FROM job_attempts WHERE job_id=?1 ORDER BY attempt_no")
            .expect("hazırla")
            .query_map([&interrupted], |r| r.get(0))
            .expect("sorgu")
            .collect::<Result<_, _>>()
            .expect("satır");
        assert_eq!(attempt_nos, vec![1, 2]);
    }

    #[test]
    fn recovery_of_exhausted_budget_ends_in_failed() {
        let (db, case_id) = db();
        let job = enqueue(&db, &case_id, "b", 0);
        db.claim_jobs("w", 1, 60, &[], at(0)).expect("sahiplen");
        // Bütçe bitti (3/3): kesinti yeniden sıraya almaz, `failed` yazar.
        db.conn()
            .execute("UPDATE jobs SET attempt_count=3 WHERE id=?1", [&job])
            .expect("sayaç");
        set_lease(&db, &job, Some(at(-10)));
        let report = db.recover_stale_jobs(at(0)).expect("kurtar");
        assert_eq!(report.interrupted, 1);
        assert_eq!(report.requeued, 0);
        assert_eq!(report.failed, 1);
        let row = db.get_job(&job).expect("oku").expect("var");
        assert_eq!(row.state, "failed");
        assert_eq!(row.error_code.as_deref(), Some("E_JOB_RETRIES"));
    }

    #[test]
    fn retry_budget_ends_in_failed() {
        let (db, case_id) = db();
        let job = enqueue(&db, &case_id, "retry", 0);
        for attempt in 1..=3 {
            let claimed = db
                .claim_jobs("w", 1, 60, &[], at(attempt * 10))
                .expect("sahiplen");
            assert_eq!(claimed.len(), 1, "{attempt}. deneme sahiplenilmeli");
            let row = db
                .fail_job(&job, "w", "E_PARSE", true, at(attempt * 10 + 1))
                .expect("hata");
            if attempt < 3 {
                assert_eq!(row.state, "retry_wait");
                assert!(row.next_run_at.is_some(), "bekleme zamanı yazılmalı");
                // Bekleme süresi dolmadan sahiplenilmez.
                assert!(db
                    .claim_jobs("w", 1, 60, &[], at(attempt * 10 + 2))
                    .expect("erken")
                    .is_empty());
                db.conn()
                    .execute(
                        "UPDATE jobs SET next_run_at=?2 WHERE id=?1",
                        params![job, iso(at(-1))],
                    )
                    .expect("bekleme");
            } else {
                assert_eq!(row.state, "failed");
                assert!(row.completed_at.is_some());
            }
        }
        let summary = db.job_queue_summary().expect("özet");
        assert_eq!(summary.total, 1);
        assert_eq!(summary.due_now, 0);
    }

    #[test]
    fn claim_respects_job_type_filter_and_parse_key() {
        let (db, case_id) = db();
        let parse = enqueue(&db, &case_id, "parse:v9", 0);
        db.enqueue_job_with_priority("index", Some(&case_id), "fp", "index:v9", 0)
            .expect("iş");
        let claimed = db
            .claim_jobs("w", 5, 60, &[JOB_PARSE_DOCUMENT], at(0))
            .expect("sahiplen");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, parse);

        // Anahtarla tekil sahiplenme: ikinci çağrı `None` döner.
        let again = db
            .claim_job_by_key("parse:v9", "w2", 60, at(1))
            .expect("sahiplen");
        assert!(again.is_none(), "zaten yürüyen iş yeniden sahiplenilmez");

        let other = enqueue(&db, &case_id, "parse:v10", 0);
        let claimed = db
            .claim_job_by_key("parse:v10", "w3", 60, at(2))
            .expect("sahiplen")
            .expect("sahiplenilmeli");
        assert_eq!(claimed.id, other);
        assert_eq!(claimed.state, "running");
    }

    #[test]
    fn summary_counts_states_and_oldest_wait() {
        let (db, case_id) = db();
        assert_eq!(db.job_queue_summary().expect("boş").total, 0);
        let a = enqueue(&db, &case_id, "a", 0);
        enqueue(&db, &case_id, "b", 0);
        let due = db.job_queue_summary().expect("özet");
        assert_eq!(due.total, 2);
        assert_eq!(due.due_now, 2);
        assert!(due.oldest_queued_at.is_some());
        db.claim_jobs("w", 1, 60, &[], at(0)).expect("sahiplen");
        db.complete_job(&a, "w", at(1)).expect("bitir");
        let after = db.job_queue_summary().expect("özet");
        assert_eq!(after.total, 2);
        assert_eq!(after.due_now, 1);
        let running = after
            .by_state
            .iter()
            .find(|s| s.state == "completed")
            .map(|s| s.count);
        assert_eq!(running, Some(1));
    }

    /// Geri çekilme formülü sözleşmede yazılı olanla aynı olmalı: 2 sn taban,
    /// her denemede iki katı, 300 sn üst sınır. Ayrışırsa kullanıcıya vaat
    /// edilen "3 deneme / artan bekleme" davranışı sessizce değişir.
    #[test]
    fn retry_backoff_matches_the_documented_policy() {
        let policy =
            retry_policy_of("{\"maxAttempts\":3,\"baseDelayMs\":2000,\"maxDelayMs\":300000}");
        assert_eq!(policy.delay_for(0), std::time::Duration::ZERO);
        assert_eq!(policy.delay_for(1), std::time::Duration::from_millis(2_000));
        assert_eq!(policy.delay_for(2), std::time::Duration::from_millis(4_000));
        assert_eq!(policy.delay_for(3), std::time::Duration::from_millis(8_000));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));

        // Eski satırlar `maxDelayMs` taşımaz: varsayılan üst sınır uygulanır.
        let legacy = retry_policy_of("{\"maxAttempts\":3,\"baseDelayMs\":2000}");
        assert_eq!(legacy.max_delay_ms, 300_000);
        assert_eq!(
            legacy.delay_for(9),
            std::time::Duration::from_millis(300_000)
        );
    }

    /// Sahiplenme sorgusu `next_run_at` üzerinde karşılaştırma yapar; NULL
    /// taşıyan satır bir daha sahiplenilmez. Bu yüzden her yolda (ekleme,
    /// hata/tekrar, kurtarma) alan dolu kalmalı.
    #[test]
    fn next_run_at_is_never_left_null() {
        let (db, case_id) = db();
        let a = enqueue(&db, &case_id, "a", 0);
        let b = enqueue(&db, &case_id, "b", 0);
        db.claim_jobs("w", 2, 60, &[], at(0)).expect("sahiplen");
        db.fail_job(&a, "w", "E_PARSE", true, at(1)).expect("hata");
        db.complete_job(&b, "w", at(1)).expect("bitir");
        set_lease(&db, &a, Some(at(-30)));
        db.recover_stale_jobs(at(2)).expect("kurtar");
        let nulls: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM jobs
                 WHERE next_run_at IS NULL AND state IN ('queued','retry_wait')",
                [],
                |r| r.get(0),
            )
            .expect("sayım");
        assert_eq!(nulls, 0);
    }

    /// Sahiplenme sorgusu tam tablo taramasına düşmemeli: `jobs` büyüdükçe
    /// her sahiplenme SQLCipher'ın bütün satırları çözmesi demek olurdu.
    #[test]
    fn claim_query_uses_an_index_not_a_full_scan() {
        let (db, _case) = db();
        let mut stmt = db
            .conn()
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT id FROM jobs INDEXED BY idx_jobs_claim
                 WHERE state=?1 AND next_run_at<=?2
                 ORDER BY priority DESC, created_at ASC
                 LIMIT 4",
            )
            .expect("plan");
        let plan: Vec<String> = stmt
            .query_map(params!["queued", "2026-01-01T00:00:00Z"], |row| {
                row.get::<_, String>(3)
            })
            .expect("plan")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan");
        let text = plan.join(" | ");
        assert!(
            text.contains("idx_jobs_claim"),
            "sahiplenme indeksi kullanılmalı: {text}"
        );
        assert!(
            !text.contains("TEMP B-TREE"),
            "sıralama indeksten gelmeli, geçici B-tree kurulmamalı: {text}"
        );
    }

    #[test]
    fn future_jobs_are_not_due() {
        let (db, case_id) = db();
        enqueue(&db, &case_id, "later", 0);
        db.conn()
            .execute("UPDATE jobs SET next_run_at=?1", [iso(at(600))])
            .expect("zaman");
        assert!(db
            .claim_jobs("w", 5, 60, &[], at(0))
            .expect("sahiplen")
            .is_empty());
        assert_eq!(db.job_queue_summary().expect("özet").due_now, 0);
        assert_eq!(
            db.claim_jobs("w", 5, 60, &[], at(601))
                .expect("sahiplen")
                .len(),
            1
        );
    }

    /// Şema kapısı: tabloda (tek kaynakta) olan her durum `jobs.state` CHECK
    /// kümesinde yazılabilir olmalı. Tablo ile şema ayrışırsa bu test düşer —
    /// bir turda tam bu ayrışma yaşandı (CHECK kümesi 7 eski adı taşıyordu).
    #[test]
    fn schema_check_covers_every_spec_state() {
        let (db, _case) = db();
        let schema: String = db
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='jobs'",
                [],
                |r| r.get(0),
            )
            .expect("şema");
        for (state, _) in job_state::all_states() {
            assert!(
                schema.contains(&format!("'{state}'")),
                "{state} CHECK kümesinde yok: {schema}"
            );
        }
        for legacy in ["'leased'", "'succeeded'", "'cancelled'"] {
            assert!(
                !schema.contains(legacy),
                "eski ad {legacy} hâlâ şemada: yeni satırlar yazılamaz"
            );
        }
        // Tablodaki her geçiş gerçekten yazılabiliyor mu: en azından hedef adlar
        // CHECK kümesinde olmalı (yukarıdaki döngü kapsıyor), kaynak adlar da.
        for (_, nexts) in job_state::all_states() {
            for next in *nexts {
                assert!(schema.contains(&format!("'{next}'")), "{next} eksik");
            }
        }
    }

    /// `running → blocked → queued`: engel nedeni yazılır, kira bırakılır ve
    /// engel kalkınca iş yeniden sahiplenilebilir (şartname §17.2).
    #[test]
    fn blocked_job_keeps_reason_and_can_be_released() {
        let (db, case_id) = db();
        let job = enqueue(&db, &case_id, "p", 0);
        db.claim_jobs("w", 1, 60, &[], at(0)).expect("sahiplen");
        let blocked = db.block_job(&job, "w", "password", at(1)).expect("engelle");
        assert_eq!(blocked.state, "blocked");
        assert_eq!(blocked.blocked_reason.as_deref(), Some("password"));
        assert!(blocked.lease_owner.is_none(), "kira bırakılmalı");
        // Engelli iş sahiplenilmez.
        assert!(db
            .claim_jobs("w2", 5, 60, &[], at(2))
            .expect("sahiplen")
            .is_empty());
        let released = db.unblock_job(&job, at(3)).expect("engeli kaldır");
        assert_eq!(released.state, "queued");
        assert_eq!(
            released.blocked_reason.as_deref(),
            Some("password"),
            "neden kayıt olarak kalır"
        );
        let claimed = db.claim_jobs("w3", 5, 60, &[], at(4)).expect("sahiplen");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, job);
    }

    /// Doğrulama yolu: `running → validating` açık adım, sonuç `partial`.
    /// `begun` çağrısı kira sahibini denetler.
    #[test]
    fn validation_step_and_partial_result_are_recorded() {
        let (db, case_id) = db();
        let job = enqueue(&db, &case_id, "v", 0);
        db.claim_jobs("w", 1, 60, &[], at(0)).expect("sahiplen");
        let validating = db.begin_validation(&job, "w", at(1)).expect("doğrula");
        assert_eq!(validating.state, "validating");
        let err = db
            .begin_validation(&job, "other", at(2))
            .expect_err("yabancı sahip doğrulayamaz");
        assert_eq!(err.code(), "E_JOB_LEASE");

        let partial = db
            .complete_job_partial(&job, "w", "E_PARSE_PARTIAL", at(3))
            .expect("kısmi");
        assert_eq!(partial.state, "partial");
        assert_eq!(partial.error_code.as_deref(), Some("E_PARSE_PARTIAL"));
        assert!(partial.completed_at.is_some());
        assert_ne!(partial.progress, 1.0, "kısmi çıktı tam sayılmaz");

        // Doğrulamadaki hata tekrar denenmez (validating → failed).
        let second = enqueue(&db, &case_id, "v2", 0);
        db.claim_jobs("w", 1, 60, &[], at(10)).expect("sahiplen");
        db.begin_validation(&second, "w", at(11)).expect("doğrula");
        let failed = db
            .fail_job(&second, "w", "E_VERIFY", true, at(12))
            .expect("hata");
        assert_eq!(failed.state, "failed");
        assert_eq!(
            failed.next_run_at, None,
            "doğrulama hatası yeniden beklemez"
        );
    }

    /// Kullanıcı yeniden denemesi: `failed → queued`, bütçe sıfırlanır ama
    /// `job_attempts` numaralandırması sürer (geçmiş silinmez).
    #[test]
    fn user_retry_after_failure_restarts_budget_and_keeps_history() {
        let (db, case_id) = db();
        let job = enqueue(&db, &case_id, "f", 0);
        db.claim_jobs("w", 1, 60, &[], at(0)).expect("sahiplen");
        db.fail_job(&job, "w", "E_PARSE", false, at(1))
            .expect("hata");
        assert_eq!(db.get_job(&job).expect("oku").expect("var").state, "failed");
        let err = db.cancel_job(&job, at(2)).expect_err("terminal iş");
        assert_eq!(err.code(), "E_JOB_STATE");

        let retried = db.retry_failed_job(&job, at(3)).expect("yeniden dene");
        assert_eq!(retried.state, "queued");
        assert_eq!(retried.attempt_count, 0, "yeni bütçe");
        assert_eq!(retried.error_code, None);
        assert_eq!(retried.completed_at, None);

        let claimed = db.claim_jobs("w2", 1, 60, &[], at(4)).expect("sahiplen");
        assert_eq!(claimed.len(), 1);
        let attempt_nos: Vec<i64> = db
            .conn()
            .prepare("SELECT attempt_no FROM job_attempts WHERE job_id=?1 ORDER BY attempt_no")
            .expect("hazırla")
            .query_map([&job], |r| r.get(0))
            .expect("sorgu")
            .collect::<Result<_, _>>()
            .expect("satır");
        assert_eq!(attempt_nos, vec![1, 2], "numara artar, sıfırdan başlamaz");
    }

    /// Bağımlılığı bekleyen iş `blocked` (queued değil) ve ebeveyn bitince
    /// kendiliğinden açılır: kuyruk "sessizce bekleyen" satır bırakmaz.
    #[test]
    fn waiting_child_is_blocked_and_released_by_parent_completion() {
        let (db, case_id) = db();
        let parent = enqueue(&db, &case_id, "parent", 0);
        let child = enqueue(&db, &case_id, "child", 0);
        db.conn()
            .execute(
                "UPDATE jobs SET dependency_ids=?2 WHERE id=?1",
                params![child, serde_json::json!([parent]).to_string()],
            )
            .expect("bağımlılık");
        let claimed = db.claim_jobs("w", 10, 60, &[], at(0)).expect("sahiplen");
        assert_eq!(claimed.len(), 1, "yalnız ebeveyn sahiplenilir");
        let blocked = db.get_job(&child).expect("oku").expect("var");
        assert_eq!(blocked.state, "blocked");
        assert_eq!(blocked.blocked_reason.as_deref(), Some("dependency"));
        assert!(blocked.next_run_at.is_some());

        db.complete_job(&parent, "w", at(1)).expect("bitir");
        let claimed = db.claim_jobs("w", 10, 60, &[], at(2)).expect("sahiplen");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, child);
    }

    /// Durum makinesi kapısı: tabloda olmayan ok `E_JOB_STATE` ile reddedilir
    /// (SQL'e hiç gitmeden).
    #[test]
    fn illegal_transitions_are_rejected_by_the_machine() {
        assert!(!job_state::allowed("validating", "retry_wait"));
        assert!(!job_state::allowed("completed", "queued"));
        assert!(
            !job_state::allowed("running", "completed"),
            "ara ad validating olmalı"
        );
        assert!(!job_state::allowed("interrupted", "canceled"));
        assert!(
            !job_state::allowed("blocked", "running"),
            "önce queued gerekir"
        );
        // ve gerçekten var olanlar
        assert!(job_state::allowed("running", "validating"));
        assert!(job_state::allowed("canceling", "canceled"));
        assert!(job_state::allowed("failed", "queued"));
    }
}
