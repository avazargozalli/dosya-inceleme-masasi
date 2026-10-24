-- F12-T06: toplu içe alma ve künye çakışması.
--
-- Aynı künye (mahkeme + daire + esas/karar) ile **farklı metin** gelirse
-- sessiz birleştirme yapılmaz: yeni nüsha ayrı sürüm olarak eklenir ve
-- `identity_conflict=1` ile incelemeye sunulur. `conflict_with_version_id`
-- hangi sürümle çakıştığını söyler (kullanıcı iki metni karşılaştırabilsin).
ALTER TABLE precedent_versions ADD COLUMN identity_conflict INTEGER NOT NULL DEFAULT 0;
ALTER TABLE precedent_versions ADD COLUMN conflict_with_version_id TEXT;

-- Aynı nüshanın tekrar içe alınması hash ile yakalanır.
CREATE INDEX precedent_versions_by_body_hash
  ON precedent_versions(precedent_id, body_hash);
