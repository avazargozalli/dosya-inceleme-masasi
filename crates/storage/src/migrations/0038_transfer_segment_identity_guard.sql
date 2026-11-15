-- 0038: **giden segment sicili kendi segmentine bağlıdır** (INV-05, P0).
--
-- Neden: 0034 giden sicil satırının gösterdiği **sürümün** kendi dosyasından
-- olmasını kapıladı, ama satırın gösterdiği **segment** hiç denetlenmedi.
-- `transfer_segments` giden metnin kendisini (maskelenmiş hâli) ve o metnin
-- hangi segmentten geldiğini (`outbound_segment_id` = `source_segment_id`)
-- birlikte taşır: satır "V sürümünün S segmenti gönderildi" der. Şema bu iki
-- alanın birbirini tutmasını istemiyordu; `scope_violations()` de bu sınıfı
-- **hiç saymıyordu** (kör).
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v37; geçici sonda sonra silindi):
-- doğrudan SQL ile A dosyasının çalıştırmasına (`run1`, `c1`) **A'nın sürümü**
-- (`v1`) + **B dosyasının segmenti** (`s9`, `v9`) yazıldı; satır kabul edildi ve
-- `list_transfer_segments('run1')` A dosyasının gönderim sicilinde B dosyasının
-- segment kimliğini gösterdi. Sicil "A'nın belgesinden çıkan metin" derken
-- gösterdiği parça B dosyasının parçasıdır: denetim izi yanlış belgeyi yazar
-- (şartname §16 bağlamı: cihazdan **ne** çıktığının kaydı).
-- Çekirdek bu kapıyı zaten taşıyordu (`record_outbound_segments`: segment
-- `s.id=?1 AND pr.document_version_id=?2 AND dv.case_id=?3`), yani kusur
-- yalnız **doğrudan SQL** yolunda açıktı — 0031–0037 ile aynı sınıf.
--
-- Yazılan (2 tetikleyici, 0034'ünkiler **genişletilerek** yeniden yazıldı):
--
--   * v34 kuralı korunur: gösterilen sürüm başka dosyada **var ise** reddedilir.
--   * Yeni kural: satırın **iki segment kolonundan** biri
--     (`outbound_segment_id` — ekranın gösterdiği alan — ve `source_segment_id`)
--     **var olan** bir segment ise gösterilen sürümün segmenti olmalı; sürüm
--     `NULL` ise (0026 öncesi biçim) segment çalıştırmanın dosyasında olmalı.
--     İki kolon birden denetlenir: üretim yolu ikisini aynı değerle yazar
--     (`record_outbound_segments`), ama ayrışırlarsa **gösterilen** alan da
--     kapıdan geçmek zorundadır.
--
-- Kasıtlı gevşeklik (0033 `note_drafts` / 0034 `ai_context_items` / 0037 ile
-- aynı): **var olmayan** segment kimliği sicilde kalabilir — `transfer_segments`
-- bir denetim izidir; belge/sürüm sonradan silinince segmenti de gider, satır
-- yaşamalıdır (bu tabloda segment yabancı anahtar değildir). Kapı yalnız
-- **var olan ama başka sürüme/dosyaya ait** segmenti reddeder; tarama da yalnız
-- kanıtlanabilir uyuşmazlığı sayar. Kendi sürümü silinmiş eski satır yanlış
-- veri değil "kaynağı artık kasada yok" demektir.
--
-- Kural 0031–0037 ile aynı: hedef **kendi sürümünde** değilse satır yazılmaz
-- (fail-closed). Reddetme `RAISE(ABORT, 'DIM_SCOPE:<tablo>')` →
-- `StorageError::ScopeViolation` → **`E_DB_SCOPE`**.

DROP TRIGGER scope_guard_transfer_segments_bi;
DROP TRIGGER scope_guard_transfer_segments_bu;

CREATE TRIGGER scope_guard_transfer_segments_bi
BEFORE INSERT ON transfer_segments
WHEN (NEW.document_version_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM document_versions dv
         WHERE dv.id = NEW.document_version_id
           AND dv.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
      ))
   OR (NEW.document_version_id IS NOT NULL AND EXISTS (
        SELECT 1
          FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id IN (NEW.outbound_segment_id, NEW.source_segment_id)
           AND pr.document_version_id IS NOT NEW.document_version_id
      ))
   OR (NEW.document_version_id IS NULL AND EXISTS (
        SELECT 1
          FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id IN (NEW.outbound_segment_id, NEW.source_segment_id)
           AND pr.case_id IS NOT (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
      ))
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:transfer_segments');
END;

CREATE TRIGGER scope_guard_transfer_segments_bu
BEFORE UPDATE OF ai_run_id, document_version_id, outbound_segment_id, source_segment_id
ON transfer_segments
WHEN (NEW.document_version_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM document_versions dv
         WHERE dv.id = NEW.document_version_id
           AND dv.case_id <> (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
      ))
   OR (NEW.document_version_id IS NOT NULL AND EXISTS (
        SELECT 1
          FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id IN (NEW.outbound_segment_id, NEW.source_segment_id)
           AND pr.document_version_id IS NOT NEW.document_version_id
      ))
   OR (NEW.document_version_id IS NULL AND EXISTS (
        SELECT 1
          FROM text_segments s
          JOIN parse_runs pr ON pr.id = s.parse_run_id
         WHERE s.id IN (NEW.outbound_segment_id, NEW.source_segment_id)
           AND pr.case_id IS NOT (SELECT r.case_id FROM ai_runs r WHERE r.id = NEW.ai_run_id)
      ))
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:transfer_segments');
END;
