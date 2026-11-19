-- 0059: **atıf kontrolü hükmü** (citation_checks) kanıtsız yazılamaz ve
-- alıntının kendisi satırda durur.
--
-- Neden: satır «bu alıntı şu emsalle şöyle eşleşir» iddiasıdır ve iki hüküm
-- sütunu okurun gördüğü yerde durur:
--   * `Precedents.tsx` liste rozetleri (`badgeIdentity`/`badgeQuote`) doğrudan
--     bu sütunları basar — «Eşleşti» yazan rozet hukukçunun dosyasındaki atıf
--     tablosunun hükmüdür;
--   * `citation_context` alıntının çevresini **yeniden doğrular**
--     (`verify_precedent_quote`): aynı satır için ikinci bir hüküm üretir.
-- v46 satırın **öznesini ve hükmünü dondurdu** (yazıldıktan sonra
-- `identity_match`/`quote_match`/`extracted_citation`/`candidate_precedent_id`
-- değişmez) ama **doğuşta kanıt istemedi**. Ölçüldü (düzeltme öncesi, gerçek
-- SQLite v58; sonda `scratch/dim-v59-probe/measure_v59.py`, log
-- `measure_before.log`): 7 kusur yazımının **7'si KABUL**, 7 meşru yazma
-- KABUL:
--   (b01/b02) **aday yokken** künye hükmü (`match`/`mismatch`) — karşılaştırma
--             konusu olmayan hüküm,
--   (b03) **numarasız** (eksik künyeli) emsale künye `match` — eşleştirilecek
--         esas/karar numarası yokken «künye tuttu» denemez,
--   (b04/b05) **tam metni olmayan** emsale alıntı hükmü (`match`/`mismatch`) —
--             ölçümün dayanağı (metin) yok; ekran «alıntı eşleşti» derken
--             `citation_context` «no_full_text» diyor,
--   (b06/b07) **aday yokken** alıntı hükmü — aynı çelişki.
-- Ayrıca çekirdek `record_citation_check` hükmü **çağırandan alıyordu**
-- (`NewCitationCheck.quote_match`); arayüzdeki serbest açılır liste ile
-- doğrulama düğmesi birbirine bağlı değildi. Daha kötüsü: satırda **alıntının
-- kendisi yoktu** — `quote_match` hangi pasajın ölçüldüğünü söylemiyordu
-- (`extracted_citation` atıf künyesidir, alıntı değil).
--
-- Yazılan kural (yeni sütun + 1 yeni tetikleyici + BU genişletmesi):
--   * `quote_text` — doğrulanan **pasajın kendisi**. NULL olabilir (eski
--     satırlar, yalnız künye kontrolü); doluysa kırpılmış, ≤ 4000 karakter.
--   * BI (`citation_check_quote_evidence_guard_bi`):
--       - `quote_text` doluysa kırpılmış ve 1..4000 karakter;
--       - alıntı hükmü (`match`/`mismatch`) **alıntı metni** ister (hangi
--         pasajın ölçüldüğü kayıtta yoksa hüküm kanıtsızdır);
--       - alıntı hükmü adayın **en az bir tam metinli sürümü** olmasını ister
--         (ölçümün dayanağı);
--       - künye hükmü (`match`/`mismatch`) **aday ister**;
--       - künye `match` ayrıca adayın en az bir numarası (esas/karar) dolu
--         olmasını ister — numarasız emsale «kimlik tuttu» iddiası boştur.
--     `not_checked`/`unknown` her zaman serbesttir (ölçmeyi reddetmek kanıt
--     istemez).
--   * BU (`citation_check_subject_guard_bu` DROP+CREATE) — `quote_text` de
--     **donar**: hüküm sabitken alıntının değiştirilmesi, rozetin başka bir
--     pasajın ölçümünü göstermesi olurdu.
--
-- Kapsam dışı (bilinçli): hükmün **doğruluğu** (pasajın gerçekten o metinde
-- bulunduğu) SQL'de hesaplanamaz — katmanlı normalizasyon (küçük harf, boşluk
-- katlama, tire birleştirme) çekirdektedir. Çekirdek `quote_match`i bu yüzden
-- **kendisi türetir** (`verify_precedent_quote(quote_text)`): bulunan `match`,
-- metin varken bulunamayan `mismatch`, metinsiz/kısa alıntı `not_checked`.
-- Şema kapısı çekirdeğin **kanıt tabanını** (alıntı + aday + tam metin)
-- doğrular.
--
-- v59 öncesi satırlar **silinmez/düzeltilmez**; `quote_text` NULL kalır ve
-- `citation_context` eski davranışla `extracted_citation`a düşer (dürüst
-- sınır). Kapı yalnız yeni yazmayı durdurur; onarım yolu **yeni kontrol
-- satırıdır**. Ret `DIM_INTEGRITY:citation_checks` → `E_DB_INTEGRITY`.
-- Tablo `case_id` taşır ve zaten taranıyor → `GUARDED_TABLES` **25 kalır**.
ALTER TABLE citation_checks ADD COLUMN quote_text TEXT;

CREATE TRIGGER citation_check_quote_evidence_guard_bi
BEFORE INSERT ON citation_checks
WHEN
  -- (1) alıntı metni verildiyse kırpılmış ve makul uzunlukta olmalı
  (NEW.quote_text IS NOT NULL
   AND (NEW.quote_text IS NOT trim(NEW.quote_text)
        OR length(NEW.quote_text) = 0
        OR length(NEW.quote_text) > 4000))
  -- (2) alıntı hükmü **hangi pasajın** ölçüldüğünü ister
  OR (NEW.quote_match IN ('match', 'mismatch')
      AND (NEW.quote_text IS NULL
           OR length(trim(NEW.quote_text)) = 0))
  -- (3) alıntı hükmü ölçümün dayanağını (tam metin) ister
  OR (NEW.quote_match IN ('match', 'mismatch')
      AND NOT EXISTS (
        SELECT 1 FROM precedent_versions v
         WHERE v.precedent_id = NEW.candidate_precedent_id
           AND v.full_text IS NOT NULL
           AND length(trim(v.full_text)) > 0))
  -- (4) künye hükmü karşılaştırma konusu ister
  OR (NEW.identity_match IN ('match', 'mismatch')
      AND (NEW.candidate_precedent_id IS NULL
           OR length(trim(NEW.candidate_precedent_id)) = 0))
  -- (5) «kimlik tuttu» iddiası adayın numarasıyla ayakta durur
  OR (NEW.identity_match = 'match'
      AND NOT EXISTS (
        SELECT 1 FROM precedents p
         WHERE p.id = NEW.candidate_precedent_id
           AND ((p.docket_no IS NOT NULL AND length(trim(p.docket_no)) > 0)
                OR (p.decision_no IS NOT NULL
                    AND length(trim(p.decision_no)) > 0))))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:citation_checks');
END;

-- v46'nın özne dondurması `quote_text`i de kapsar: hüküm sabitken alıntı
-- değişirse rozet başka bir pasajın ölçümünü gösterirdi.
DROP TRIGGER citation_check_subject_guard_bu;
CREATE TRIGGER citation_check_subject_guard_bu
BEFORE UPDATE OF id, extracted_citation, quote_text, candidate_precedent_id,
                 identity_match, quote_match, reason_codes, created_at
ON citation_checks
WHEN NEW.id IS NOT OLD.id
  OR NEW.extracted_citation IS NOT OLD.extracted_citation
  OR NEW.quote_text IS NOT OLD.quote_text
  OR NEW.candidate_precedent_id IS NOT OLD.candidate_precedent_id
  OR NEW.identity_match IS NOT OLD.identity_match
  OR NEW.quote_match IS NOT OLD.quote_match
  OR NEW.reason_codes IS NOT OLD.reason_codes
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:citation_checks');
END;
