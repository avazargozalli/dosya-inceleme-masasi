-- 0021_job_states: `jobs.state` şartname §17.2'nin tamamını taşır.
--
-- Neden tablo yeniden yazılıyor: SQLite CHECK kısıtını `ALTER TABLE` ile
-- değiştirmiyor. 0001'in kümesi `queued|leased|running|succeeded|failed|
-- cancelled|retry_wait` idi; şartname `blocked|validating|canceling|
-- interrupted|partial|completed|failed|canceled` adlarını istiyor. Eski
-- adlar veri kaybı olmadan eşlenir:
--   leased → running · succeeded → completed · cancelled → canceled
--
-- `blocked_reason`: `running → blocked` geçişinde engel nedeni yazılır
-- (şartname §17.2 "güvenli checkpoint ve engel nedeni yazılır"). Hata kodu
-- değildir; bu yüzden `error_code` yeniden kullanılmadı.
--
-- DİKKAT (sıra bağımlılığı): `DROP TABLE jobs` yabancı anahtar denetimi AÇIK
-- iken `job_attempts` satırlarını CASCADE ile siler. Migrasyon çalıştırıcısı
-- (`Database::migrate`) bu yüzden migrasyonları `PRAGMA foreign_keys=OFF` ile
-- koşar ve commit öncesi `PRAGMA foreign_key_check` ile denetler; bu dosya
-- o kurala güvenir (kanıt: `upgrade_tests::job_state_rewrite_keeps_attempts`).
CREATE TABLE jobs_new (
  id TEXT PRIMARY KEY,
  type TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('vault', 'case')),
  case_id TEXT REFERENCES cases(id) ON DELETE CASCADE,
  input_fingerprint TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  dependency_ids TEXT NOT NULL DEFAULT '[]',
  state TEXT NOT NULL DEFAULT 'queued'
    CHECK (state IN ('queued', 'blocked', 'running', 'validating', 'retry_wait',
                     'canceling', 'interrupted', 'partial', 'completed', 'failed',
                     'canceled')),
  progress REAL NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 1),
  checkpoint TEXT,
  retry_policy TEXT NOT NULL DEFAULT '{"maxAttempts":3,"baseDelayMs":2000}',
  attempt_count INTEGER NOT NULL DEFAULT 0,
  next_run_at TEXT,
  lease_owner TEXT,
  lease_until TEXT,
  cancel_requested_at TEXT,
  error_code TEXT,
  blocked_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  completed_at TEXT,
  CHECK (scope = 'case' OR case_id IS NULL)
);

INSERT INTO jobs_new (id, type, scope, case_id, input_fingerprint, idempotency_key,
                      dependency_ids, state, progress, checkpoint, retry_policy,
                      attempt_count, next_run_at, lease_owner, lease_until,
                      cancel_requested_at, error_code, blocked_reason, created_at,
                      updated_at, priority, completed_at)
SELECT id, type, scope, case_id, input_fingerprint, idempotency_key,
       dependency_ids,
       CASE state
         WHEN 'leased' THEN 'running'
         WHEN 'succeeded' THEN 'completed'
         WHEN 'cancelled' THEN 'canceled'
         ELSE state
       END,
       progress, checkpoint, retry_policy, attempt_count, next_run_at, lease_owner,
       lease_until, cancel_requested_at, error_code, NULL, created_at, updated_at,
       priority, completed_at
FROM jobs;

DROP TABLE jobs;

ALTER TABLE jobs_new RENAME TO jobs;

-- Tablo yeniden yazıldığı için 0001/0020 indeksleri düştü; aynı tanımlarla
-- geri kurulur (sahiplenme planı `idx_jobs_claim`e bağlı, plan testi tutuyor).
CREATE INDEX idx_jobs_state_next ON jobs(state, next_run_at);
CREATE INDEX idx_jobs_lease ON jobs(lease_until);
CREATE INDEX idx_jobs_claim ON jobs(state, priority DESC, created_at, next_run_at);
