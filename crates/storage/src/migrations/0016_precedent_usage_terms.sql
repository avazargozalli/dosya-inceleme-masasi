-- F12-T11: kaynak kullanım koşulu.
--
-- Kütüphaneye alınan her kararın **kullanım dayanağı** ve **yeniden dağıtım
-- izni** ayrı sütunlarda tutulur. Hiçbir varsayılan "izin var" demek değildir:
-- yeni ve eski kayıtlar `unknown` ile başlar; kullanıcı dayanağı yazmadan
-- "izinli" işaretleyemez (kural Rust tarafında, `precedent_terms.rs`).
--
-- `terms_note` ve `terms_url` yazılı dayanağı taşır. `terms_recorded_at`
-- kaydın **alındığı** zamandır; hukuki inceleme tarihi diye sunulmaz.
ALTER TABLE precedents ADD COLUMN usage_basis TEXT NOT NULL DEFAULT 'unknown'
  CHECK (usage_basis IN ('official_public', 'licence', 'permission', 'internal_only', 'unknown'));
ALTER TABLE precedents ADD COLUMN redistribution TEXT NOT NULL DEFAULT 'unknown'
  CHECK (redistribution IN ('allowed', 'not_allowed', 'unknown'));
ALTER TABLE precedents ADD COLUMN terms_url TEXT;
ALTER TABLE precedents ADD COLUMN terms_note TEXT NOT NULL DEFAULT '';
ALTER TABLE precedents ADD COLUMN terms_recorded_at TEXT;

CREATE INDEX precedents_by_redistribution ON precedents(redistribution);
