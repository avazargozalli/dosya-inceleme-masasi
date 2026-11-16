-- 0046: **atıf kontrolü sicili kendi hükmünü kanıtlamalı** (INV-05; F12 alıntı
-- doğrulama zincirinin yazma tarafı).
--
-- Neden: `citation_checks` bir avukatın «bu alıntı şu emsalle eşleşiyor»
-- iddiasıdır ve satır üç olguyu birlikte taşır:
--   * **konu** — hangi belge sürümü (`input_document_version_id`), hangi alıntı
--     metni (`extracted_citation`), hangi aday emsal (`candidate_precedent_id`);
--   * **hüküm** — `identity_match` / `quote_match`;
--   * **anlam incelemesi** — `meaning_review` / `meaning_note` /
--     `meaning_reviewed_at` (0015: «boş bırakmak incelenmedi demektir»).
-- Arayüz rozetleri (`Precedents.tsx`: `badgeIdentity`, `badgeQuote`,
-- `badgeMeaning`) doğrudan bu sütunları basar; bağlam paneli
-- (`citation_context`) alıntıyı **bağlı emsalin** tam metninde arar.
-- 0037 yalnız **kapsamı** kapıladı (satır kendi dosyasının sürümünü gösterir).
-- Konu, hüküm ve anlam incelemesi hiçbir tetikleyici taşımıyordu; çekirdek
-- (`record_citation_check`, `review_citation_meaning`) doğru yazıyor ama
-- doğrudan SQL yolu hiçbir kurala takılmıyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v45; sonda
-- `~/.hermes/cache/scratch/dim-v46-probe/measure_v46.py`, log
-- `measure_before.log`): **22 kusur yazımının 21'i KABUL** edildi (tek RED:
-- var olan çapraz sürüme taşıma — 0037'nin kapsam kapısı):
--   (1) var **olmayan** emsal kimliği yazıldı (bağlam paneli çözülemez);
--   (2) yalnız boşluktan oluşan alıntı;
--   (3) bozuk `reason_codes` JSON'u (okuyucu sessizce boş liste yapar);
--   (4) `reason_codes` dizi değil (nesne);
--   (5) yeni kontrol «incelendi» damgasıyla doğdu — `meaning_reviewed_at` yok;
--   (6) anlam hükmü kapalı kümede değil (`'yanlis'`; rozet ham belirteci basar);
--   (7) `identity_match` UPDATE ile `match`→`mismatch` çevrildi;
--   (8) `quote_match` UPDATE ile çevrildi;
--   (9) alıntı metni değiştirildi, hüküm sabit kaldı;
--   (10) emsal bağı p1→p2 çevrildi: hüküm p1'in ölçümü, bağlam p2'nin metni;
--   (11) emsal bağı NULL'a çekildi; (12) var olmayan emsal bağı yazıldı;
--   (13) satır **aynı dosyada başka sürüme** taşındı — kontrol başka belgenin
--        listesinde göründü (`list_citation_checks('c1','v2')` → 1 satır);
--   (14) `reason_codes` değiştirildi; (15) `id` değiştirildi;
--   (16) `created_at` değiştirildi;
--   (17) `'misleading'` damgası **gerekçesiz** yazıldı (çekirdek istiyor, SQL
--        yolu istemiyordu);
--   (18) `'accurate'` damgası `meaning_reviewed_at` NULL iken yazıldı;
--   (19) anlam hükmü kapalı küme dışına taşındı;
--   (20) `'not_reviewed'` + gerekçe dolu (hükmü olmayan gerekçe);
--   (21) sürümü **var olmayan** satırda `case_id` başka dosyaya taşındı
--        (0037 kapısı yalnız «var olan çapraz sürüm»ü reddediyordu).
--   v45 taraması bu 8 satırın **hiçbirini** saymıyordu (`citation_checks` = 0).
--
-- Yazılan kural:
--   * `scope_guard_citation_checks_bu` (0037) **genişletildi** (DROP+CREATE, tek
--     tanım): `case_id` ve `input_document_version_id` **değişmez**. Böylece
--     aynı dosyada sürüm taşıması ve sürümü silinmiş satırın dosya taşıması da
--     durur. Ret öneki **değişmedi** (`DIM_SCOPE:citation_checks`): aynı sütun
--     kümesini iki tetikleyici paylaşırsa hangisinin koştuğu belirsiz kalırdı,
--     v37'nin mevcut kapı testleri de kırılırdı.
--   * `citation_check_subject_guard_bi` — yeni satır: alıntı boşluk değil,
--     dolu emsal bağı **var olan** bir emsali gösterir, `reason_codes` metin
--     dizisi, `meaning_review='not_reviewed'` + boş gerekçe + damgasız
--     (yeni kontrol **doğuştan incelenmiş olamaz**; inceleme ayrı avukat
--     eylemidir, `review_citation_meaning`).
--   * `citation_check_subject_guard_bu` — `id`, `extracted_citation`,
--     `candidate_precedent_id`, `identity_match`, `quote_match`,
--     `reason_codes`, `created_at` **değişmez**. Satır hangi alıntıyı hangi
--     emsalle ölçtüyse öyle kalır; yeniden ölçüm **yeni satır** ister.
--   * `citation_check_meaning_guard_bu` — anlam incelemesi kendi içinde
--     tutarlı: kapalı küme (`not_reviewed|accurate|misleading|off_context`),
--     `not_reviewed` ⇒ boş gerekçe + damgasız, incelenmiş ⇒ **damgalı**,
--     `misleading`/`off_context` ⇒ **gerekçeli** (çekirdeğin kuralı şemada).
--   Diğer sütunlar (mevcut kod hiç güncellemese de ileride bir alan) takılmaz —
--   v35 dersi: `UPDATE OF` listesi yalnız sayılan sütunları kapsar.
--
-- Bilinçli sınırlar (dürüst):
--   (a) `precedents` satırları hiçbir yoldan silinmez (silme komutu yok, FK
--       `ON DELETE CASCADE` yalnız sürüm/not için) → «var olmayan emsal»
--       denetim izi değil **kanıtlanabilir yalan** sayıldı (v45 ile aynı
--       fail-closed karar). Var olmayan **sürüm** kimliği 0037'de olduğu gibi
--       serbesttir (belge sonradan silinebilir; kapı onu ölçemez).
--   (b) Hükmün **doğruluğu** yeniden hesaplanmaz: kapı «hüküm sonradan
--       değişmedi» der, «hüküm doğru» demez (alıntı yeniden taranmaz).
--   (c) SQLite `trim()` yalnız verilen ASCII karakterleri kırpar; yalnız
--       U+00A0 gibi boşluklardan oluşan alıntı bu kapıdan geçer (çekirdek
--       `str::trim` ile reddeder — asıl yol kapalı, doğrudan SQL artığı kalır).
--   (d) v46 öncesi tutarsız satırlar **silinmez**; `scope_violations()`
--       sayımı bu sınıfı da kapsayacak biçimde genişletildi (aşağıdaki kod
--       değişikliği) ama **onarılmaz** — immutability onarımı da engeller
--       (temizlik ürün kararı).
--   (e) Tetikleyicileri anahtarı olan saldırgan düşürebilir (uygulama hatasına
--       karşı).
--
-- Ret `RAISE(ABORT,'DIM_INTEGRITY:citation_checks')` → `StorageError::Integrity`
-- → **`E_DB_INTEGRITY`** (0044/0045 ile aynı önek).

CREATE TRIGGER citation_check_subject_guard_bi
BEFORE INSERT ON citation_checks
WHEN length(trim(NEW.extracted_citation, ' ' || char(9) || char(10) || char(13))) = 0
   OR (NEW.candidate_precedent_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM precedents p WHERE p.id = NEW.candidate_precedent_id))
   OR NOT json_valid(NEW.reason_codes)
   OR json_type(CASE WHEN json_valid(NEW.reason_codes)
                     THEN NEW.reason_codes ELSE '[]' END) <> 'array'
   OR EXISTS (
        SELECT 1 FROM json_each(
               CASE WHEN json_valid(NEW.reason_codes)
                     AND json_type(NEW.reason_codes) = 'array'
                    THEN NEW.reason_codes ELSE '[]' END) j
         WHERE j.type <> 'text')
   OR NEW.meaning_review <> 'not_reviewed'
   OR length(trim(NEW.meaning_note, ' ' || char(9) || char(10) || char(13))) > 0
   OR NEW.meaning_reviewed_at IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:citation_checks');
END;

CREATE TRIGGER citation_check_subject_guard_bu
BEFORE UPDATE OF id, extracted_citation, candidate_precedent_id,
                 identity_match, quote_match, reason_codes, created_at
ON citation_checks
WHEN NEW.id IS NOT OLD.id
  OR NEW.extracted_citation IS NOT OLD.extracted_citation
  OR NEW.candidate_precedent_id IS NOT OLD.candidate_precedent_id
  OR NEW.identity_match IS NOT OLD.identity_match
  OR NEW.quote_match IS NOT OLD.quote_match
  OR NEW.reason_codes IS NOT OLD.reason_codes
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:citation_checks');
END;

CREATE TRIGGER citation_check_meaning_guard_bu
BEFORE UPDATE OF meaning_review, meaning_note, meaning_reviewed_at
ON citation_checks
WHEN NEW.meaning_review NOT IN ('not_reviewed', 'accurate', 'misleading', 'off_context')
   OR (NEW.meaning_review = 'not_reviewed'
       AND (length(trim(NEW.meaning_note, ' ' || char(9) || char(10) || char(13))) > 0
            OR NEW.meaning_reviewed_at IS NOT NULL))
   OR (NEW.meaning_review <> 'not_reviewed' AND NEW.meaning_reviewed_at IS NULL)
   OR (NEW.meaning_review IN ('misleading', 'off_context')
       AND length(trim(NEW.meaning_note, ' ' || char(9) || char(10) || char(13))) = 0)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:citation_checks');
END;

-- 0037'nin kapsam kapısı **aynı sütunlar** üzerinde genişletilir: satırın
-- dosyası ve gösterdiği sürüm yazıldıktan sonra değişmez. Kapsam kuralı
-- (var olan çapraz sürüm reddi) aynen korunur.
DROP TRIGGER scope_guard_citation_checks_bu;

CREATE TRIGGER scope_guard_citation_checks_bu
BEFORE UPDATE OF input_document_version_id, case_id ON citation_checks
WHEN NEW.case_id IS NULL
   OR NEW.case_id IS NOT OLD.case_id
   OR NEW.input_document_version_id IS NOT OLD.input_document_version_id
   OR EXISTS (
        SELECT 1
          FROM document_versions v
          JOIN documents d ON d.id = v.document_id
         WHERE v.id = NEW.input_document_version_id
           AND d.case_id <> NEW.case_id
   )
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:citation_checks');
END;
