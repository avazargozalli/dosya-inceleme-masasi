-- 0001_init: çekirdek şema (şartname §06).
-- Kimlikler UUIDv7 metni; teknik zamanlar UTC ISO-8601 metni; hukuki tarih ayrı
-- alanlarda ve `date_precision` ile taşınır. Para alanları ondalık metindir.

CREATE TABLE cases (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 200),
  case_kind TEXT NOT NULL CHECK (case_kind IN ('review', 'reference_archive')),
  reference_code TEXT,
  jurisdiction_label TEXT,
  practice_area TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'archived', 'deleted')),
  sensitivity TEXT NOT NULL CHECK (sensitivity IN ('normal', 'high', 'client_confidential')),
  default_ai_policy TEXT NOT NULL CHECK (default_ai_policy IN ('off', 'local_only', 'cli_allowed', 'api_allowed')),
  archived_at TEXT,
  retention_hold INTEGER NOT NULL DEFAULT 0 CHECK (retention_hold IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1 CHECK (row_version >= 1),
  created_by_local_actor_id TEXT,
  deleted_at TEXT
);

CREATE TABLE actors (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('person', 'organization', 'unknown')),
  normalized_names TEXT NOT NULL DEFAULT '[]',
  identifiers_encrypted BLOB,
  review_state TEXT NOT NULL DEFAULT 'pending' CHECK (review_state IN ('pending', 'accepted', 'rejected', 'merged')),
  merged_into_actor_id TEXT REFERENCES actors(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1,
  created_by_local_actor_id TEXT,
  deleted_at TEXT
);

CREATE TABLE case_participants (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  actor_id TEXT NOT NULL REFERENCES actors(id) ON DELETE RESTRICT,
  role TEXT NOT NULL,
  role_note TEXT,
  valid_from TEXT,
  valid_to TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1,
  created_by_local_actor_id TEXT,
  deleted_at TEXT,
  UNIQUE (case_id, actor_id, role)
);

CREATE TABLE blobs (
  id TEXT PRIMARY KEY,
  storage_id TEXT NOT NULL UNIQUE,
  cipher_version INTEGER NOT NULL CHECK (cipher_version = 1),
  encrypted_size INTEGER NOT NULL CHECK (encrypted_size >= 0),
  wrapped_key_ref TEXT NOT NULL,
  reference_count INTEGER NOT NULL DEFAULT 0 CHECK (reference_count >= 0),
  integrity_status TEXT NOT NULL DEFAULT 'unverified' CHECK (integrity_status IN ('unverified', 'verified', 'corrupt')),
  created_at TEXT NOT NULL
);

CREATE TABLE documents (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  display_name TEXT NOT NULL,
  document_type TEXT NOT NULL DEFAULT 'unknown',
  declared_document_date TEXT,
  declared_date_precision TEXT NOT NULL DEFAULT 'unknown'
    CHECK (declared_date_precision IN ('day', 'month', 'year', 'range', 'approximate', 'unknown')),
  source_origin TEXT NOT NULL DEFAULT 'import' CHECK (source_origin IN ('import', 'copy', 'reference', 'generated')),
  current_version_id TEXT,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived', 'deleted', 'quarantined')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1,
  created_by_local_actor_id TEXT,
  deleted_at TEXT
);

CREATE TABLE document_versions (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  version_no INTEGER NOT NULL CHECK (version_no >= 1),
  blob_id TEXT NOT NULL REFERENCES blobs(id) ON DELETE RESTRICT,
  original_sha256 TEXT NOT NULL CHECK (length(original_sha256) = 64),
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  detected_mime TEXT NOT NULL,
  imported_at TEXT NOT NULL,
  original_filename_encrypted BLOB,
  parent_version_id TEXT REFERENCES document_versions(id) ON DELETE SET NULL,
  is_original INTEGER NOT NULL DEFAULT 1 CHECK (is_original IN (0, 1)),
  UNIQUE (document_id, version_no)
);

CREATE TABLE parse_runs (
  id TEXT PRIMARY KEY,
  document_version_id TEXT NOT NULL REFERENCES document_versions(id) ON DELETE CASCADE,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  engine TEXT NOT NULL,
  engine_version TEXT NOT NULL,
  settings_hash TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'partial')),
  pages_expected INTEGER,
  pages_processed INTEGER NOT NULL DEFAULT 0,
  warnings TEXT NOT NULL DEFAULT '[]',
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE pages (
  id TEXT PRIMARY KEY,
  parse_run_id TEXT NOT NULL REFERENCES parse_runs(id) ON DELETE CASCADE,
  page_index INTEGER NOT NULL CHECK (page_index >= 0),
  page_label TEXT,
  width REAL,
  height REAL,
  rotation INTEGER NOT NULL DEFAULT 0,
  text_origin TEXT NOT NULL CHECK (text_origin IN ('native', 'ocr', 'mixed', 'none')),
  quality_flags TEXT NOT NULL DEFAULT '[]',
  derivative_blob_id TEXT REFERENCES blobs(id) ON DELETE SET NULL,
  UNIQUE (parse_run_id, page_index)
);

CREATE TABLE text_segments (
  rowid INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT NOT NULL UNIQUE,
  parse_run_id TEXT NOT NULL REFERENCES parse_runs(id) ON DELETE CASCADE,
  page_id TEXT REFERENCES pages(id) ON DELETE CASCADE,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  structural_path TEXT NOT NULL DEFAULT '',
  raw_text TEXT NOT NULL,
  normalized_text TEXT NOT NULL,
  offset_map TEXT NOT NULL DEFAULT '[]',
  bbox_list TEXT NOT NULL DEFAULT '[]',
  language TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE source_anchors (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  document_version_id TEXT NOT NULL REFERENCES document_versions(id) ON DELETE CASCADE,
  parse_run_id TEXT NOT NULL REFERENCES parse_runs(id) ON DELETE CASCADE,
  page_id TEXT REFERENCES pages(id) ON DELETE SET NULL,
  segment_id TEXT NOT NULL,
  start_offset INTEGER NOT NULL CHECK (start_offset >= 0),
  end_offset INTEGER NOT NULL CHECK (end_offset >= start_offset),
  exact_quote TEXT NOT NULL,
  prefix TEXT NOT NULL DEFAULT '',
  suffix TEXT NOT NULL DEFAULT '',
  quote_hash TEXT NOT NULL CHECK (length(quote_hash) = 64),
  anchor_status TEXT NOT NULL DEFAULT 'active'
    CHECK (anchor_status IN ('active', 'ambiguous', 'unresolved', 'rebound')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  title TEXT NOT NULL DEFAULT '',
  body_ast TEXT NOT NULL,
  author_kind TEXT NOT NULL CHECK (author_kind IN ('user', 'system', 'ai_candidate')),
  visibility_label TEXT NOT NULL DEFAULT 'case_private',
  source_anchor_ids TEXT NOT NULL DEFAULT '[]',
  approved_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1,
  created_by_local_actor_id TEXT,
  deleted_at TEXT
);

CREATE TABLE events (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  event_kind TEXT NOT NULL,
  date_value TEXT,
  date_precision TEXT NOT NULL CHECK (date_precision IN ('day', 'month', 'year', 'range', 'approximate', 'unknown')),
  date_role TEXT NOT NULL CHECK (date_role IN ('event', 'document', 'notification', 'contract', 'hearing', 'deadline_candidate', 'unknown')),
  date_raw TEXT,
  sortable_date TEXT,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  assertion_kind TEXT NOT NULL DEFAULT 'user' CHECK (assertion_kind IN ('user', 'party_claim', 'document', 'system_inference')),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'confirmed', 'disputed', 'archived')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1,
  created_by_local_actor_id TEXT,
  deleted_at TEXT,
  CHECK (date_precision <> 'unknown' OR sortable_date IS NULL)
);

CREATE TABLE event_sources (
  id TEXT PRIMARY KEY,
  event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  source_anchor_id TEXT NOT NULL REFERENCES source_anchors(id) ON DELETE CASCADE,
  relation TEXT NOT NULL CHECK (relation IN ('states', 'supports', 'contradicts', 'context')),
  user_confirmed INTEGER NOT NULL DEFAULT 0 CHECK (user_confirmed IN (0, 1)),
  UNIQUE (event_id, source_anchor_id, relation)
);

CREATE TABLE claims (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  claimant_actor_id TEXT REFERENCES actors(id) ON DELETE SET NULL,
  claim_text TEXT NOT NULL,
  claim_kind TEXT NOT NULL CHECK (claim_kind IN ('allegation', 'defense', 'admission', 'court_finding', 'unknown')),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'asserted', 'responded', 'decided', 'archived')),
  source_anchor_id TEXT REFERENCES source_anchors(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1,
  created_by_local_actor_id TEXT,
  deleted_at TEXT
);

CREATE TABLE evidence_links (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  claim_id TEXT NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
  source_anchor_id TEXT NOT NULL REFERENCES source_anchors(id) ON DELETE CASCADE,
  relation TEXT NOT NULL CHECK (relation IN ('submitted_for', 'response_to', 'supports_candidate', 'contradicts_candidate', 'context')),
  rationale TEXT NOT NULL DEFAULT '',
  review_state TEXT NOT NULL DEFAULT 'pending' CHECK (review_state IN ('pending', 'accepted', 'rejected', 'edited', 'needs_more_evidence')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done', 'cancelled')),
  user_due_date TEXT,
  date_timezone TEXT,
  date_origin TEXT NOT NULL DEFAULT 'user_entered'
    CHECK (date_origin IN ('user_entered', 'document', 'imported', 'unknown')),
  assignee_label TEXT,
  source_anchor_id TEXT REFERENCES source_anchors(id) ON DELETE SET NULL,
  reminder_policy TEXT NOT NULL DEFAULT 'none' CHECK (reminder_policy IN ('none', 'on_day', 'day_before')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1,
  deleted_at TEXT
);

CREATE TABLE review_items (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  item_kind TEXT NOT NULL,
  payload_schema_version INTEGER NOT NULL DEFAULT 1,
  payload TEXT NOT NULL,
  source_anchor_ids TEXT NOT NULL DEFAULT '[]',
  ai_run_id TEXT,
  state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'accepted', 'rejected', 'edited', 'needs_more_evidence')),
  target_entity_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE review_decisions (
  id TEXT PRIMARY KEY,
  review_item_id TEXT NOT NULL REFERENCES review_items(id) ON DELETE CASCADE,
  decision TEXT NOT NULL CHECK (decision IN ('accept', 'reject', 'edit', 'request_evidence')),
  edited_payload TEXT,
  reviewer_label TEXT NOT NULL,
  rationale TEXT NOT NULL DEFAULT '',
  decided_at TEXT NOT NULL
);

CREATE TABLE checklist_templates (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  practice_area TEXT,
  version INTEGER NOT NULL,
  author TEXT NOT NULL,
  review_date TEXT,
  license_id TEXT,
  content_hash TEXT NOT NULL,
  is_community INTEGER NOT NULL DEFAULT 0 CHECK (is_community IN (0, 1)),
  created_at TEXT NOT NULL
);

CREATE TABLE case_checklist_items (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  template_version_id TEXT REFERENCES checklist_templates(id) ON DELETE SET NULL,
  item_text TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'present', 'missing', 'not_applicable')),
  linked_document_ids TEXT NOT NULL DEFAULT '[]',
  note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE provider_profiles (
  id TEXT PRIMARY KEY,
  provider_type TEXT NOT NULL CHECK (provider_type IN ('local_model', 'cli', 'api', 'off')),
  adapter_id TEXT NOT NULL,
  version_range TEXT,
  executable_ref TEXT,
  execution_environment TEXT NOT NULL DEFAULT 'local',
  auth_mode_requested TEXT,
  auth_mode_verified TEXT,
  capabilities TEXT NOT NULL DEFAULT '{}',
  policy_version TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  row_version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE model_profiles (
  id TEXT PRIMARY KEY,
  provider_profile_id TEXT NOT NULL REFERENCES provider_profiles(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL,
  revision TEXT NOT NULL,
  context_limit INTEGER,
  capabilities TEXT NOT NULL DEFAULT '{}',
  license_url TEXT,
  local_file_hash TEXT,
  hardware_profile TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (provider_profile_id, model_id, revision)
);

CREATE TABLE ai_runs (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  task_type TEXT NOT NULL,
  provider_id TEXT REFERENCES provider_profiles(id) ON DELETE SET NULL,
  model_snapshot TEXT NOT NULL,
  input_manifest_hash TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  output_schema_version TEXT NOT NULL,
  consent_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'partial')),
  timings TEXT NOT NULL DEFAULT '{}',
  usage_report TEXT NOT NULL DEFAULT '{"billing":"unknown"}',
  created_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE TABLE ai_context_items (
  id TEXT PRIMARY KEY,
  ai_run_id TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
  document_version_id TEXT NOT NULL,
  source_anchor_id TEXT,
  segment_ids TEXT NOT NULL DEFAULT '[]',
  inclusion_reason TEXT NOT NULL,
  bytes_sent INTEGER NOT NULL CHECK (bytes_sent >= 0)
);

CREATE TABLE transfer_segments (
  id TEXT PRIMARY KEY,
  ai_run_id TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
  outbound_segment_id TEXT NOT NULL,
  source_segment_id TEXT,
  outbound_text TEXT NOT NULL,
  outbound_hash TEXT NOT NULL,
  transform_manifest_id TEXT,
  source_range_map TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE ai_messages (
  id TEXT PRIMARY KEY,
  ai_run_id TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
  visible_content TEXT NOT NULL,
  source_refs TEXT NOT NULL DEFAULT '[]',
  state TEXT NOT NULL DEFAULT 'visible'
);

CREATE TABLE ai_candidates (
  id TEXT PRIMARY KEY,
  ai_run_id TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
  type TEXT NOT NULL,
  structured_payload TEXT NOT NULL,
  validation_state TEXT NOT NULL CHECK (validation_state IN ('valid', 'invalid', 'partial')),
  validation_errors TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE transfer_consents (
  id TEXT PRIMARY KEY,
  provider_snapshot TEXT NOT NULL,
  case_scope TEXT NOT NULL,
  purpose TEXT NOT NULL,
  selected_document_ids TEXT NOT NULL DEFAULT '[]',
  expires_at TEXT,
  consent_text_version TEXT NOT NULL,
  approved_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE TABLE transfer_receipts (
  id TEXT PRIMARY KEY,
  ai_run_id TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
  destination_label TEXT NOT NULL,
  outbound_hash TEXT NOT NULL,
  item_count INTEGER NOT NULL,
  byte_count INTEGER NOT NULL,
  status TEXT NOT NULL,
  dispatched_at TEXT NOT NULL
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  type TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('vault', 'case')),
  case_id TEXT REFERENCES cases(id) ON DELETE CASCADE,
  input_fingerprint TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  dependency_ids TEXT NOT NULL DEFAULT '[]',
  state TEXT NOT NULL DEFAULT 'queued'
    CHECK (state IN ('queued', 'leased', 'running', 'succeeded', 'failed', 'cancelled', 'retry_wait')),
  progress REAL NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 1),
  checkpoint TEXT,
  retry_policy TEXT NOT NULL DEFAULT '{"maxAttempts":3,"baseDelayMs":2000}',
  attempt_count INTEGER NOT NULL DEFAULT 0,
  next_run_at TEXT,
  lease_owner TEXT,
  lease_until TEXT,
  cancel_requested_at TEXT,
  error_code TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (scope = 'case' OR case_id IS NULL)
);

CREATE TABLE job_attempts (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  attempt_no INTEGER NOT NULL,
  engine_version TEXT,
  error_code TEXT,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  retry_after TEXT,
  UNIQUE (job_id, attempt_no)
);

CREATE TABLE audit_events (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT NOT NULL UNIQUE,
  category TEXT NOT NULL,
  entity_id TEXT,
  operation TEXT NOT NULL,
  actor_label TEXT NOT NULL,
  metadata_allowlist TEXT NOT NULL DEFAULT '{}',
  prior_hash TEXT,
  event_hash TEXT NOT NULL,
  timestamp TEXT NOT NULL
);

CREATE TABLE outbox_events (
  id TEXT PRIMARY KEY,
  aggregate_id TEXT NOT NULL,
  case_id TEXT,
  event_type TEXT NOT NULL,
  payload_version INTEGER NOT NULL,
  payload TEXT NOT NULL,
  created_at TEXT NOT NULL,
  delivered_at TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE embeddings (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  segment_id TEXT NOT NULL,
  model_profile_id TEXT NOT NULL,
  model_revision TEXT NOT NULL,
  dimensions INTEGER NOT NULL CHECK (dimensions > 0),
  vector_bytes BLOB NOT NULL,
  normalized INTEGER NOT NULL DEFAULT 1 CHECK (normalized IN (0, 1)),
  input_hash TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE ai_cache_entries (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  cache_key TEXT NOT NULL,
  ai_run_id TEXT REFERENCES ai_runs(id) ON DELETE SET NULL,
  output_blob_id TEXT,
  validation_version TEXT NOT NULL,
  expires_at TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (case_id, cache_key)
);

CREATE TABLE precedents (
  id TEXT PRIMARY KEY,
  court TEXT NOT NULL,
  chamber TEXT,
  docket_no TEXT,
  decision_no TEXT,
  application_no TEXT,
  decision_date TEXT,
  title TEXT,
  source_url TEXT,
  user_tags TEXT NOT NULL DEFAULT '[]',
  legal_issue TEXT,
  completeness TEXT NOT NULL DEFAULT 'incomplete' CHECK (completeness IN ('complete', 'incomplete')),
  created_at TEXT NOT NULL
);

CREATE TABLE precedent_versions (
  id TEXT PRIMARY KEY,
  precedent_id TEXT NOT NULL REFERENCES precedents(id) ON DELETE CASCADE,
  document_version_id TEXT,
  acquired_at TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  body_hash TEXT NOT NULL,
  verification_state TEXT NOT NULL DEFAULT 'unverified'
    CHECK (verification_state IN ('unverified', 'metadata_verified', 'text_verified', 'failed'))
);

CREATE TABLE precedent_notes (
  id TEXT PRIMARY KEY,
  precedent_id TEXT NOT NULL REFERENCES precedents(id) ON DELETE CASCADE,
  case_id_optional TEXT REFERENCES cases(id) ON DELETE CASCADE,
  useful_passage_anchor TEXT,
  similarity_note TEXT NOT NULL DEFAULT '',
  distinguishing_note TEXT NOT NULL DEFAULT '',
  user_conclusion TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE TABLE citation_checks (
  id TEXT PRIMARY KEY,
  input_document_version_id TEXT NOT NULL,
  extracted_citation TEXT NOT NULL,
  candidate_precedent_id TEXT,
  identity_match TEXT NOT NULL DEFAULT 'unknown' CHECK (identity_match IN ('match', 'mismatch', 'unknown')),
  quote_match TEXT NOT NULL DEFAULT 'unknown' CHECK (quote_match IN ('match', 'mismatch', 'not_checked', 'unknown')),
  meaning_review TEXT NOT NULL DEFAULT 'not_reviewed',
  reason_codes TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL
);

CREATE TABLE render_derivatives (
  id TEXT PRIMARY KEY,
  document_version_id TEXT NOT NULL,
  parse_run_id TEXT,
  blob_id TEXT NOT NULL,
  renderer_version TEXT NOT NULL,
  settings_hash TEXT NOT NULL,
  font_manifest_hash TEXT,
  page_manifest TEXT NOT NULL DEFAULT '[]',
  content_hash TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE redaction_plans (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  document_version_id TEXT NOT NULL,
  parse_run_id TEXT,
  render_derivative_id TEXT,
  render_hash TEXT NOT NULL,
  transform_manifest_id TEXT,
  coordinate_space TEXT NOT NULL,
  plan_version INTEGER NOT NULL DEFAULT 1,
  state TEXT NOT NULL CHECK (state IN ('draft', 'reviewing', 'approved', 'applied', 'failed')),
  reviewer TEXT,
  output_profile TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE redaction_regions (
  id TEXT PRIMARY KEY,
  plan_id TEXT NOT NULL REFERENCES redaction_plans(id) ON DELETE CASCADE,
  page_index INTEGER NOT NULL,
  quads TEXT NOT NULL,
  coordinate_space TEXT NOT NULL,
  category TEXT NOT NULL,
  detected_text_hash TEXT,
  user_decision TEXT NOT NULL DEFAULT 'pending'
    CHECK (user_decision IN ('pending', 'redact', 'keep', 'needs_review'))
);

CREATE TABLE exports (
  id TEXT PRIMARY KEY,
  case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  type TEXT NOT NULL,
  included_ids TEXT NOT NULL DEFAULT '[]',
  template_version TEXT,
  status TEXT NOT NULL CHECK (status IN ('draft', 'building', 'verifying', 'ready', 'failed', 'blocked')),
  destination_kind TEXT NOT NULL,
  output_hash TEXT,
  externalization_consent_id TEXT,
  created_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE TABLE export_items (
  id TEXT PRIMARY KEY,
  export_id TEXT NOT NULL REFERENCES exports(id) ON DELETE CASCADE,
  source_version_id TEXT NOT NULL,
  output_page_range TEXT,
  citation_label TEXT,
  redaction_state TEXT NOT NULL DEFAULT 'none'
);

CREATE TABLE backup_runs (
  id TEXT PRIMARY KEY,
  snapshot_id TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  manifest_hash TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'verified')),
  verification_result TEXT,
  destination_label TEXT NOT NULL,
  created_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE TABLE settings (
  scope TEXT NOT NULL CHECK (scope IN ('vault', 'device', 'ui')),
  key TEXT NOT NULL,
  typed_value TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (scope, key)
);

CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at TEXT NOT NULL
);

-- Sorgu yolları (06.5)
CREATE INDEX idx_cases_status ON cases(status, updated_at);
CREATE INDEX idx_documents_case_status ON documents(case_id, status, updated_at);
CREATE INDEX idx_versions_document ON document_versions(document_id, version_no);
CREATE INDEX idx_versions_case ON document_versions(case_id);
CREATE INDEX idx_parse_runs_version ON parse_runs(document_version_id, created_at);
CREATE INDEX idx_text_segments_case ON text_segments(case_id, parse_run_id);
CREATE INDEX idx_anchors_version ON source_anchors(document_version_id, anchor_status);
CREATE INDEX idx_events_case_date ON events(case_id, sortable_date);
CREATE INDEX idx_review_items_state ON review_items(case_id, state, created_at);
CREATE INDEX idx_jobs_state_next ON jobs(state, next_run_at);
CREATE INDEX idx_jobs_lease ON jobs(lease_until);
CREATE INDEX idx_audit_entity ON audit_events(entity_id, sequence);
CREATE INDEX idx_outbox_undelivered ON outbox_events(delivered_at, created_at);
CREATE INDEX idx_embeddings_case_model ON embeddings(case_id, model_profile_id, segment_id);

-- Anahtar-değer ayarlarında yalnız bilinen kapsamlar; tip dönüşümü uygulamada.
