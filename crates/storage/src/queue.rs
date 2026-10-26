//! Kalıcı iş kuyruğu: sahiplenme (lease), tamamlama, hata, iptal ve yeniden
//! başlatma kurtarması.
//!
//! Şartname §17 iş türlerini ve durum makinesini tanımlar. Bu modül kuyruğun
//! **veritabanı ayağını** uygular; durum geçiş kuralları `dim-jobs`'ta tek
//! yerde durur (kural burada kopyalanmaz). `jobs` tablosu 0001'den beri vardı
//! ama hiçbir katman satırı sahiplenmiyordu: her içe aktarma `queued` bir iş
//! bırakıyor, iş sonsuza kadar "bekliyor" görünüyordu. Bu modül o boşluğu
//! kapatır.
//!
//! Dürüst sınırlar:
//! - `jobs.state` CHECK kümesi `queued|leased|running|succeeded|failed|cancelled|
//!   retry_wait`. Şartnamenin `blocked|validating|canceling|interrupted|partial|
//!   completed` adları **ayrı durum olarak yok**; `interrupted` ve `canceling`
//!   burada *koşul* olarak taşınır (`error_code='E_JOB_INTERRUPTED'`,
//!   `cancel_requested_at` dolu). Yeni durum eklemek tablo yeniden yazımı
//!   gerektirir (CHECK değişmez); bilinçli olarak yapılmadı.
//! - Kuyruk yürütücüsü tek süreçlidir: kasa başına tek `Database` bağlantısı
//!   (tek yazıcı) kuralı burada da geçerli. Çok süreçli yarış yalnız
//!   `lease_until` ile korunur.
//! - Ölçüm: `docs/releases/F15/BENCHMARK.md` (kuyruk gecikmesi p50/p95).

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

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

/// Kurtarma raporu (açılışta ya da elle).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecovery {
    /// Kirası dolmuş, yeniden sıraya alınan iş sayısı.
    pub interrupted: usize,
    /// İptal isteği gelmiş ve yarıda kalmış, doğrudan iptal edilen iş sayısı.
    pub cancelled: usize,
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

const JOB_COLUMNS: &str = "id,type,scope,case_id,state,priority,attempt_count,retry_policy,progress,next_run_at,lease_owner,lease_until,cancel_requested_at,error_code,created_at,updated_at,completed_at";

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
    })
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
    /// bitmemiş iş atlanır; bağımlılığı **başarısız/iptal/kayıp** olan iş
    /// sonsuza kadar beklemez, `failed` işaretlenir ve bir sonraki tura geçilir.
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
        let mut claimed: Vec<JobRow> = Vec::new();
        {
            // Durum başına tek eşitlik: `state IN (...)` yazıldığında SQLite
            // sıralamayı indeksten karşılayamıyor ve her sahiplenmede geçici
            // B-tree kuruyordu (plan testi bunu tutuyor). İki sorgunun sonucu
            // Rust'ta (öncelik, yaş) sırasına dizilir.
            let mut candidates: Vec<(JobRow, String)> = Vec::new();
            for state in ["queued", "retry_wait"] {
                let mut stmt = tx.prepare(&format!(
                    "SELECT {JOB_COLUMNS},dependency_ids FROM jobs INDEXED BY idx_jobs_claim
                     WHERE state=?1 AND next_run_at<=?2
                     ORDER BY priority DESC, created_at ASC
                     LIMIT ?3"
                ))?;
                let rows = stmt
                    .query_map(params![state, now_text, scan_limit], |row| {
                        let job = map_job(row)?;
                        let deps: String = row.get(17)?;
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
                if job.cancel_requested_at.is_some() {
                    tx.execute(
                        "UPDATE jobs SET state='cancelled', error_code='E_JOB_CANCELLED', completed_at=?2, updated_at=?2
                         WHERE id=?1 AND state IN ('queued','retry_wait')",
                        params![job.id, now_text],
                    )?;
                    continue;
                }
                match dependency_state(&tx, &deps_raw)? {
                    DependencyState::Ready => {}
                    DependencyState::Waiting => continue,
                    DependencyState::Aborted(code) => {
                        tx.execute(
                            "UPDATE jobs SET state='failed', error_code=?2, completed_at=?3, updated_at=?3
                             WHERE id=?1 AND state IN ('queued','retry_wait')",
                            params![job.id, code, now_text],
                        )?;
                        continue;
                    }
                }
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
                    "INSERT INTO job_attempts (id, job_id, attempt_no, started_at) VALUES (?1,?2,?3,?4)",
                    params![uuid::Uuid::now_v7().to_string(), job.id, attempt_no, now_text],
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
        if job.state != "queued" && job.state != "retry_wait" {
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
        let changed = tx.execute(
            "UPDATE jobs SET state='running', lease_owner=?2, lease_until=?3,
                    attempt_count=attempt_count+1, updated_at=?4
             WHERE id=?1 AND state IN ('queued','retry_wait')",
            params![job.id, owner, until_text, now_text],
        )?;
        if changed != 1 {
            return Ok(None);
        }
        let attempt_no = job.attempt_count + 1;
        tx.execute(
            "INSERT INTO job_attempts (id, job_id, attempt_no, started_at) VALUES (?1,?2,?3,?4)",
            params![
                uuid::Uuid::now_v7().to_string(),
                job.id,
                attempt_no,
                now_text
            ],
        )?;
        tx.commit()?;
        self.get_job(&job.id)
    }

    /// İşi başarıyla bitirir. Kira sahibi ve süresi denetlenir; iptal isteği
    /// varsa sonuç `succeeded` değil `cancelled` olur (istek sessizce yutulmaz).
    pub fn complete_job(
        &self,
        job_id: &str,
        owner: &str,
        now: OffsetDateTime,
    ) -> Result<JobRow, StorageError> {
        self.finish_job(job_id, owner, now, None, false)
    }

    /// İşi hatayla bitirir. `retryable` ve deneme bütçesi uygunsa iş
    /// `retry_wait` + artan bekleme ile yeniden sıraya girer; değilse `failed`.
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
        let job = self.get_job(job_id)?.ok_or(StorageError::NotFound)?;
        if job.state != "running" && job.state != "leased" {
            return Err(StorageError::JobState(format!(
                "iş {} durumunda; yalnız yürüyen iş bitirilebilir",
                job.state
            )));
        }
        if job.lease_owner.as_deref() != Some(owner) {
            return Err(StorageError::JobLease(
                "kira sahibi bu süreç değil".to_string(),
            ));
        }
        if lease_expired(job.lease_until.as_deref(), now) {
            return Err(StorageError::JobLease("kira süresi dolmuş".to_string()));
        }
        let now_text = iso(now);
        let raw_policy: String = self
            .conn()
            .query_row("SELECT retry_policy FROM jobs WHERE id=?1", [job_id], |r| {
                r.get(0)
            })
            .optional()?
            .unwrap_or_default();
        let policy = retry_policy_of(&raw_policy);

        let mut effective_code = error_code;
        let (state, next_run_at, completed_at) = if job.cancel_requested_at.is_some() {
            effective_code = Some("E_JOB_CANCELLED");
            ("cancelled", None, Some(now_text.clone()))
        } else if let Some(_code) = error_code {
            let attempt = u32::try_from(job.attempt_count).unwrap_or(u32::MAX);
            if retryable && policy.should_retry(attempt) {
                let delay = policy.delay_for(attempt);
                let ms = i64::try_from(delay.as_millis()).unwrap_or(i64::MAX);
                (
                    "retry_wait",
                    Some(iso(now + time::Duration::milliseconds(ms))),
                    None,
                )
            } else {
                ("failed", None, Some(now_text.clone()))
            }
        } else {
            ("succeeded", None, Some(now_text.clone()))
        };
        let progress = if state == "succeeded" {
            1.0
        } else {
            job.progress
        };
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE jobs SET state=?2, error_code=NULLIF(?3,''), next_run_at=?4, completed_at=?5,
                    progress=?6, lease_owner=NULL, lease_until=NULL, updated_at=?7
             WHERE id=?1 AND state IN ('running','leased')",
            params![
                job_id,
                state,
                effective_code.unwrap_or(""),
                next_run_at,
                completed_at,
                progress,
                now_text
            ],
        )?;
        tx.execute(
            "UPDATE job_attempts SET ended_at=?2, error_code=NULLIF(?3,'')
             WHERE job_id=?1 AND ended_at IS NULL",
            params![job_id, now_text, effective_code.unwrap_or("")],
        )?;
        tx.commit()?;
        self.get_job(job_id)?.ok_or(StorageError::NotFound)
    }

    /// İptal isteği. Sırada bekleyen iş hemen `cancelled` olur; yürüyen işe
    /// işaret konur (`cancelRequestedAt`) ve yürütücü bitirince iptal olur.
    pub fn cancel_job(
        &self,
        job_id: &str,
        now: OffsetDateTime,
    ) -> Result<JobCancelResult, StorageError> {
        let job = self.get_job(job_id)?.ok_or(StorageError::NotFound)?;
        let now_text = iso(now);
        match job.state.as_str() {
            "queued" | "retry_wait" => {
                self.conn().execute(
                    "UPDATE jobs SET state='cancelled', error_code='E_JOB_CANCELLED',
                            completed_at=?2, updated_at=?2
                     WHERE id=?1 AND state IN ('queued','retry_wait')",
                    params![job_id, now_text],
                )?;
                Ok(JobCancelResult {
                    outcome: "cancelled".to_string(),
                    job: self.get_job(job_id)?.ok_or(StorageError::NotFound)?,
                })
            }
            "running" | "leased" => {
                self.conn().execute(
                    "UPDATE jobs SET cancel_requested_at=COALESCE(cancel_requested_at,?2), updated_at=?2
                     WHERE id=?1 AND state IN ('running','leased')",
                    params![job_id, now_text],
                )?;
                Ok(JobCancelResult {
                    outcome: "requested".to_string(),
                    job: self.get_job(job_id)?.ok_or(StorageError::NotFound)?,
                })
            }
            other => Err(StorageError::JobState(format!(
                "iş {other} durumunda; iptal edilemez"
            ))),
        }
    }

    /// Kirası dolmuş yürüyen işleri kurtarır: iptal isteği olanı iptal eder,
    /// olmayanı `retry_wait` + `E_JOB_INTERRUPTED` ile hemen sıraya alır.
    /// Deneme sayısı **artmaz** (kesinti başarısız deneme değildir) ama
    /// bütçe de sıfırlanmaz.
    pub fn recover_stale_jobs(&self, now: OffsetDateTime) -> Result<JobRecovery, StorageError> {
        let now_text = iso(now);
        let tx = self.conn().unchecked_transaction()?;
        let mut stmt = tx.prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE state IN ('running','leased')"
        ))?;
        let jobs: Vec<JobRow> = stmt
            .query_map([], map_job)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut interrupted = Vec::new();
        let mut cancelled = 0usize;
        for job in jobs {
            if !lease_expired(job.lease_until.as_deref(), now) {
                continue;
            }
            if job.cancel_requested_at.is_some() {
                tx.execute(
                    "UPDATE jobs SET state='cancelled', error_code='E_JOB_CANCELLED',
                            completed_at=?2, lease_owner=NULL, lease_until=NULL, updated_at=?2
                     WHERE id=?1",
                    params![job.id, now_text],
                )?;
                tx.execute(
                    "UPDATE job_attempts SET ended_at=?2, error_code='E_JOB_CANCELLED'
                     WHERE job_id=?1 AND ended_at IS NULL",
                    params![job.id, now_text],
                )?;
                cancelled += 1;
                continue;
            }
            tx.execute(
                "UPDATE jobs SET state='retry_wait', error_code='E_JOB_INTERRUPTED',
                        next_run_at=?2, lease_owner=NULL, lease_until=NULL, updated_at=?2
                 WHERE id=?1",
                params![job.id, now_text],
            )?;
            tx.execute(
                "UPDATE job_attempts SET ended_at=?2, error_code='E_JOB_INTERRUPTED'
                 WHERE job_id=?1 AND ended_at IS NULL",
                params![job.id, now_text],
            )?;
            interrupted.push(job.id);
        }
        tx.commit()?;
        Ok(JobRecovery {
            interrupted: interrupted.len(),
            cancelled,
            interrupted_job_ids: interrupted,
        })
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
        let due_now: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM jobs WHERE state='queued' AND (next_run_at IS NULL OR next_run_at<=?1)",
            [&now_text],
            |r| r.get(0),
        )?;
        let oldest_queued_at: Option<String> = self
            .conn()
            .query_row(
                "SELECT MIN(created_at) FROM jobs WHERE state IN ('queued','retry_wait')",
                [],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let blocked_by_dependency: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM jobs WHERE state='failed' AND error_code IN ('E_JOB_DEPENDENCY_ABORTED','E_JOB_DEPENDENCY_MISSING')",
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
            Some("succeeded") => {}
            Some("failed" | "cancelled") => {
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
        assert_eq!(done.state, "succeeded");
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

        // Ebeveyn başarısız olunca bağımlı işler beklemez, `failed` olur.
        let claimed = db.claim_jobs("w", 10, 60, &[], at(2)).expect("sahiplen");
        assert!(claimed.is_empty());
        let child_row = db.get_job(&child).expect("oku").expect("var");
        assert_eq!(child_row.state, "failed");
        assert_eq!(
            child_row.error_code.as_deref(),
            Some("E_JOB_DEPENDENCY_ABORTED")
        );
        let other_row = db.get_job(&other).expect("oku").expect("var");
        assert_eq!(other_row.state, "failed");
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
        assert_eq!(result.outcome, "cancelled");
        assert_eq!(result.job.state, "cancelled");
        assert!(result.job.completed_at.is_some());

        let running = enqueue(&db, &case_id, "r", 0);
        db.claim_jobs("w", 1, 60, &[], at(1)).expect("sahiplen");
        let result = db.cancel_job(&running, at(2)).expect("iptal isteği");
        assert_eq!(result.outcome, "requested");
        assert_eq!(result.job.state, "running");
        assert!(result.job.cancel_requested_at.is_some());

        // Yürütücü bitirince sonuç `succeeded` değil `cancelled` olur.
        let finished = db.complete_job(&running, "w", at(3)).expect("bitir");
        assert_eq!(finished.state, "cancelled");
        assert_eq!(finished.error_code.as_deref(), Some("E_JOB_CANCELLED"));

        let err = db.cancel_job(&running, at(4)).expect_err("terminal iş");
        assert_eq!(err.code(), "E_JOB_STATE");
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
        assert_eq!(report.interrupted, 1);
        assert_eq!(report.cancelled, 1);
        assert_eq!(report.interrupted_job_ids, vec![interrupted.clone()]);

        let row = db.get_job(&interrupted).expect("oku").expect("var");
        assert_eq!(row.state, "retry_wait");
        assert_eq!(row.error_code.as_deref(), Some("E_JOB_INTERRUPTED"));
        assert!(row.lease_owner.is_none());
        // Kesinti deneme sayısını artırmaz, bütçeyi de sıfırlamaz.
        assert_eq!(row.attempt_count, 1);

        // Kurtarılan iş hemen yeniden sahiplenilebilir.
        let claimed = db
            .claim_jobs("w2", 1, 60, &[], at(1))
            .expect("yeniden sahiplen");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, interrupted);
        assert_eq!(claimed[0].attempt_count, 2);
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
            .find(|s| s.state == "succeeded")
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
}
