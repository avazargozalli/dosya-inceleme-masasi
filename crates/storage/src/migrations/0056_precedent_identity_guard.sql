-- 0056: **emsal künyesi** (`precedents`) doğuşta tutarlı olmalı ve kimlik
-- sütunları sonradan değişmemeli.
--
-- Neden: bu satır «hangi karar bu» iddiasıdır — mahkeme, daire,
-- esas/karar/başvuru numarası, karar tarihi, başlık, kaynak bağlantısı ve
-- `completeness` rozeti. Okuyucular bu satıra yaslanır:
--   * `screens/Precedents.tsx` her satırda `completeness` rozetini (`ok`/`warn`)
--     ve `decisionDate`i **ham** basar;
--   * `precedent_bibliography` kopyalanabilir hukuki künye metnini bu
--     sütunlardan kurar: tarih ISO değilse «olduğu gibi» yazılır, eksik alan
--     `missing` listesine girer — yani rozet ile `missing` **aynı ekranda**
--     çelişebilir («complete» rozet + «esas numarası» eksik uyarısı);
--   * `find_precedent_by_identity` aynı kararın nüshalarını mahkeme + daire +
--     numaralarla eşler; künye çevrilebilirse yazılmış nüsha (`precedent_versions`)
--     sessizce **başka karara** bağlanır;
--   * `search_precedents` künye numaralarında arar.
-- Şemada bu tablonun 0001'den beri **hiçbir** tetikleyicisi yoktu; yalnız
-- `completeness` ve 0016'nın iki kapalı kümesi CHECK'ti.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v55; sonda
-- `scratch/dim-v56-probe/measure_v56.py`, log `measure_before.log`):
-- 46 yazımın **46'sı KABUL** — 36 kusur + 10 meşru yol:
--   doğum (23): boş kimlik; boş / yalnız boşluk / kırpılmamış mahkeme;
--     **rozet yalanı** (`complete` + iki numara da NULL; `complete` + karar no
--     yok); **rozet eksik-beyanı** (`incomplete` + iki numara dolu);
--     tarih `dün`, `02.05.2025`, `2026-02-30`, `2026-1-5`, ISO + saat;
--     daire yalnız boşluk / kırpılmamış; etiket bozuk JSON / nesne / sayı
--     dizisi (okuyucu `unwrap_or_default` ile **sessizce boşaltır**);
--     boş `created_at`, `created_at='dün'`; konu yalnız boşluk; başlık ve
--     başvuru no kırpılmamış; kaynak url kırpılmamış.
--   UPDATE (13): mahkeme, esas no, karar no, karar tarihi, başlık, kaynak url,
--     etiketler, konu, `created_at` çevrildi; daire NULL'a çekildi; başvuru no
--     eklendi; rozet sonradan çevrildi; **kimlik takası** (`id`).
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — `id` kırpılmış-dolu; `court` kırpılmış ve dolu; `completeness`
--     **kanıtla tutarlı** (`complete` ⇔ kırpılmış esas **ve** karar no dolu —
--     çekirdek zaten böyle hesaplar); `decision_date` NULL ya da **gerçek
--     takvim günü** `YYYY-AA-GG` (`date()` eşitliği: `2026-02-30`,
--     `2023-02-29`, `2026-1-5`, `02.05.2025`, `dün` ve saat taşıyan damga
--     reddedilir; hukuki tarih **gün** taşır); isteğe bağlı metin sütunları
--     (`chamber`, `docket_no`, `decision_no`, `application_no`, `title`,
--     `source_url`, `legal_issue`) NULL ya da **kırpılmış ve boşluk değil**;
--     `user_tags` **metin dizisi** (JSON dizi, her eleman `text`);
--     `created_at` ISO-8601 damgası (0052 ile aynı desen).
--   * UPDATE — on üç **kimlik** sütunu (`id`, `court`, `chamber`,
--     `docket_no`, `decision_no`, `application_no`, `decision_date`, `title`,
--     `source_url`, `user_tags`, `legal_issue`, `completeness`, `created_at`)
--     **donar**: satır doğduğu künyeyle yaşar (çekirdekte bu sütunlara UPDATE
--     yolu yok; tek meşru yazma yeni emsal doğurmaktır). **Kullanım koşulu**
--     sütunları (`usage_basis`, `redistribution`, `terms_url`, `terms_note`,
--     `terms_recorded_at`) bu kapının **dışında** — `set_precedent_usage_terms`
--     onları meşru olarak günceller. Değişmeyen yazma takılmaz.
-- Ret `DIM_INTEGRITY:precedents` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Çekirdek hizası: `create_precedent` karar tarihini artık **ISO güne**
-- çevirir (`GG.AA.YYYY`, `GG/AA/YYYY`, `YYYY/AA/GG` → `YYYY-AA-GG`; sıra
-- tahmini yapılmaz). Çözülemeyen değer olduğu gibi bırakılır ve kapı reddeder
-- (fail-closed) — böylece toplu künye içe alma (`parse_manifest` tarihi
-- **ham** geçiriyordu) Türkçe tarih yazımıyla çalışmaya devam eder.
--
-- Kapsam taraması: tablo `case_id` taşımaz → yeni sınıf eklenmedi,
-- `GUARDED_TABLES` **25 kalır** (0044–0055 ile aynı sınıf — kural dosya
-- kapsamı değil **künye bütünlüğü**dür).
--
-- Dürüst sınırlar: (a) v56 öncesi tutarsız künyeler **silinmez** ve artık
-- **onarılamaz** (kimlik dondu) — temizlik ürün kararı, onarım yolu yeni kayıt.
-- (b) Kullanım koşulu iddiası (`allowed` ⇒ dayanak + yazılı dayanak, damga)
-- bu turda **kapsam dışı** — Rust doğrular, şema doğrulamaz (sıradaki aday).
-- (c) `source_url` yalnız kırpılmış-dolu denetlenir; URL **biçimi** (şema,
-- kontrol karakteri) doğrulanmaz. (d) Tarih **takvimde var mı** denetlenir,
-- gelecekte mi diye bakılmaz. (e) SQLite `trim()` yalnız ASCII boşluk kırpar
-- (U+00A0 artığı geçer). (f) Etiketlerin **içeriği** (boş metin, yinelenen)
-- denetlenmez. (g) Tetikleyicileri anahtarı olan saldırgan düşürebilir.
-- (h) Arayüz davranışı değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

CREATE TRIGGER precedent_identity_guard_bi
BEFORE INSERT ON precedents
WHEN length(trim(NEW.id)) = 0
  OR length(trim(NEW.court)) = 0
  OR NEW.court <> trim(NEW.court)
  OR (NEW.completeness = 'complete')
     <> (length(trim(COALESCE(NEW.docket_no, ''))) > 0
         AND length(trim(COALESCE(NEW.decision_no, ''))) > 0)
  OR (NEW.decision_date IS NOT NULL
      AND (date(NEW.decision_date) IS NULL
           OR date(NEW.decision_date) <> NEW.decision_date))
  OR (NEW.chamber IS NOT NULL
      AND (NEW.chamber <> trim(NEW.chamber)
           OR length(trim(NEW.chamber)) = 0))
  OR (NEW.docket_no IS NOT NULL
      AND (NEW.docket_no <> trim(NEW.docket_no)
           OR length(trim(NEW.docket_no)) = 0))
  OR (NEW.decision_no IS NOT NULL
      AND (NEW.decision_no <> trim(NEW.decision_no)
           OR length(trim(NEW.decision_no)) = 0))
  OR (NEW.application_no IS NOT NULL
      AND (NEW.application_no <> trim(NEW.application_no)
           OR length(trim(NEW.application_no)) = 0))
  OR (NEW.title IS NOT NULL
      AND (NEW.title <> trim(NEW.title)
           OR length(trim(NEW.title)) = 0))
  OR (NEW.source_url IS NOT NULL
      AND (NEW.source_url <> trim(NEW.source_url)
           OR length(trim(NEW.source_url)) = 0))
  OR (NEW.legal_issue IS NOT NULL
      AND (NEW.legal_issue <> trim(NEW.legal_issue)
           OR length(trim(NEW.legal_issue)) = 0))
  OR NOT json_valid(NEW.user_tags)
  OR json_type(CASE WHEN json_valid(NEW.user_tags)
                    THEN NEW.user_tags ELSE '[]' END) <> 'array'
  OR EXISTS (
       SELECT 1 FROM json_each(
              CASE WHEN json_valid(NEW.user_tags)
                    AND json_type(NEW.user_tags) = 'array'
                    THEN NEW.user_tags ELSE '[]' END) j
        WHERE typeof(j.value) <> 'text')
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedents');
END;

CREATE TRIGGER precedent_identity_guard_bu
BEFORE UPDATE ON precedents
WHEN NEW.id IS NOT OLD.id
  OR NEW.court IS NOT OLD.court
  OR NEW.chamber IS NOT OLD.chamber
  OR NEW.docket_no IS NOT OLD.docket_no
  OR NEW.decision_no IS NOT OLD.decision_no
  OR NEW.application_no IS NOT OLD.application_no
  OR NEW.decision_date IS NOT OLD.decision_date
  OR NEW.title IS NOT OLD.title
  OR NEW.source_url IS NOT OLD.source_url
  OR NEW.user_tags IS NOT OLD.user_tags
  OR NEW.legal_issue IS NOT OLD.legal_issue
  OR NEW.completeness IS NOT OLD.completeness
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedents');
END;
