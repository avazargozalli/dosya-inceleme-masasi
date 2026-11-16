-- 0043: **çalıştırmanın yasal dayanağı kendi dosyasında** (INV-05, P0).
--
-- Neden: `ai_runs.consent_id` "bu gönderim hangi onayla yapıldı" sorusunun tek
-- kaydıdır (şartname §13: aktarım onayı). Çekirdek `start_ai_run` onayın
-- `case_scope`unu ve belge listesini denetliyor, ama **şemada kapı yoktu**:
-- sütun serbest metin, `scope_violations()` bu tabloyu hiç saymıyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v42; sonda
-- `scratch/dim-v43-probe/measure_v43.py`, log `dim-v43-measure.log`):
--   (1) A dosyasının çalıştırmasına doğrudan SQL ile **B dosyasının onayı**
--       yazıldı (`INSERT … consent_id='tc9'` → KABUL, rows=1);
--   (2) var olan çalıştırmanın onayı UPDATE ile B'nin onayına çevrildi
--       (`UPDATE ai_runs SET consent_id='tc9'` → KABUL);
--   (3) çalıştırma başka dosyaya taşındı (`UPDATE ai_runs SET case_id='c2'`);
--       gönderim sicili A'nın sürümünü göstermeye devam etti;
--       `list_ai_runs('c2')` satırı B'nin çalıştırması diye döndürdü ve v34
--       taraması o satırın transfer segmentini **1** aykırı saydı — yani var
--       olan sicil tek UPDATE ile sessizce çaprazlaştı;
--   (4) v42 taraması `ai_runs`ı hiç saymıyordu (alan yok → 0);
--   (5) var olmayan onay kimliği yazılabildi (aşağıdaki bilinçli gevşeklik).
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT: `consent_id` doluysa ve o kimlikte bir onay **varsa**, onayın
--     `case_scope`u çalıştırmanın `case_id`si olmalı; başka dosyanın onayı
--     yazılmaz (fail-closed).
--   * UPDATE OF case_id, consent_id: iki sütun **değişmez** (`IS NOT`).
--     Bir çalıştırmanın dosyası ve yasal dayanağı kurulduktan sonra taşınmaz:
--     mevcut kod yalnız `status`/`usage_report`/`completed_at` günceller.
--     Durum güncellemesi takılmaz — tetikleyici yalnız bu iki sütunu hedefler
--     (v35 dersi: boşta kalmış referans başka alanın güncellemesini engellemez).
--
-- Kasıtlı gevşeklik (0033 `note_drafts` / 0034 `ai_context_items` / 0038
-- `transfer_segments` / 0039 `transfer_consents` ile aynı): **var olmayan**
-- onay kimliği ne kapıya takılır ne taranır — kayıt denetim izidir, silinmiş
-- bir satırı kanıtlayan uyuşmazlık yoktur. Onay satırları üründe silinmez
-- (iptal `revoked_at` ile yazılır); yine de eski/elle yazılmış satır bu
-- biçimde olabilir. Sınır belgede yazılı: kapı **yanlış dayanak** yazmayı
-- durdurur, "dayanak yok" (NULL) satırı durdurmaz.
--
-- Kural 0031–0042 ile aynı: reddetme `RAISE(ABORT,'DIM_SCOPE:ai_runs')` →
-- `StorageError::ScopeViolation` → **`E_DB_SCOPE`**.

CREATE TRIGGER scope_guard_ai_runs_bi
BEFORE INSERT ON ai_runs
WHEN NEW.consent_id IS NOT NULL
  AND EXISTS (
       SELECT 1 FROM transfer_consents c
        WHERE c.id = NEW.consent_id
          AND c.case_scope <> NEW.case_id)
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:ai_runs');
END;

CREATE TRIGGER scope_guard_ai_runs_bu
BEFORE UPDATE OF case_id, consent_id ON ai_runs
WHEN NEW.case_id IS NOT OLD.case_id
  OR NEW.consent_id IS NOT OLD.consent_id
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:ai_runs');
END;
