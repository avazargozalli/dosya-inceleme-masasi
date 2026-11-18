-- 0054: **maskeleme bölgesi sayfa sınırı** — bölge, planın maskede kullanılacak
-- ayrıştırma koşusunda **var olan** bir sayfayı göstermeli ve dikdörtgeni o
-- sayfanın kırpma kutusunun içinde kalmalı.
--
-- Neden: 0050 bölgenin biçimini ve değişmezliğini kapıladı, ama kapanış sınırı
-- (c) açıkça şunu bıraktı: «`quads` koordinatlarının sayfa sınırları içinde
-- olduğu veya sayfa sayısıyla tutarlılığı doğrulanmaz». Oysa bölge satırı
-- **maskenin kendisidir**:
--   * ekran plan kartında «N bölge» ve sayfa kırılımını («Sayfa 5: 2 bölge»)
--     doğrudan bu satırlardan basar (`screens/Redaction.tsx`);
--   * `outbound_terms_from_plan` bölgenin `page_index`ini planın ayrıştırma
--     koşusundaki sayfalarda arar — sayfa yoksa maskeleme **komple** reddedilir;
--   * `block_terms` yalnız bölgeyle kesişen bloklardan terim üretir; sayfa
--     dışına taşan kutu hiçbir metinle kesişmez ve «kesişen metin bloğu yok»
--     hatası verir; ters yönde, sayfayı aşan bir kutu sayfadaki **her** bloğu
--     kapsayıp gereğinden fazla maskeleme üretir.
-- Yani kusur sessiz sızıntı değil **kanıt yalanıdır**: plan «sayfa 5'i
-- maskeledim» der, oysa belge 2 sayfadır; ya da kutu sayfanın dışındadır ve
-- maske hiç uygulanamaz. Bu satırlar doğrudan SQL ile yazılabiliyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v53; sonda
-- `~/.hermes/cache/scratch/dim-v54-probe/measure_v54.py`, log
-- `measure_before.log`): 23 yazımın **12 kusuru 12/12 KABUL** (sayfa 3/9 —
-- koşuda yok; planın **bildirdiği** koşuda olmayan ama en son koşuda olan
-- sayfa; ters dikdörtgen; sıfır alanlı (x ve y); genişliği aşan; negatif x;
-- negatif y; yüksekliği aşan; iki dörtlüden biri taşan; 2 punto taşan; −1,5
-- punto negatif), 9 meşru yazma KABUL ve 2 yazım zaten RED (kapanmış plan,
-- 0050'nin UPDATE dondurması).
--
-- Yazılan kural (tek tetikleyici — 0050'nin INSERT kapısı **genişletildi**;
-- aynı sütun kümesinde ikinci bir BEFORE INSERT tetikleyicisi sıra
-- belirsizliği ve farklı hata sınıfı üretirdi): 0050'nin bütün cümleleri
-- aynen durur, üstüne üç cümle eklenir:
--   1. **Sayfa var olmalı:** bölge, planın maskede kullanılacak ayrıştırma
--      koşusunda (`plan.parse_run_id` bildirilmişse **o**, değilse çekirdeğin
--      seçtiği **en yeni sayfalı koşu**) var olan bir `page_index`
--      göstermeli. Koşuda hiç sayfa yoksa cümle boşta kalır (dürüst sınır a).
--   2. **Dikdörtgen pozitif alanlı olmalı:** her dörtlüde `x0 < x1` ve
--      `y0 < y1` (ters/çakışık kutu, `rects_overlap` min/max normalleştirmesi
--      yüzünden bugüne dek sessizce kabul ediliyordu).
--   3. **Kutu sayfanın içinde kalmalı:** sayfanın `width`/`height` biliniyorsa
--      her dörtlü `[-1, width+1] × [-1, height+1]` aralığında. **1 punto
--      yuvarlama payı bilinçli:** önizleme ölçeği `72/dpi` ve piksel
--      yuvarlaması, kullanıcı görüntü kenarına dayanan seçimde kutuyu bir
--      puana kadar taşırabilir; metin kutusu da glif taşmasıyla taşabilir.
--      Pay, gerçek taşmayı (≥2 punto) yakalamayı sürdürür.
-- Değişmeyen yazma takılmaz; UPDATE kapısı 0050'deki gibi **sekiz sütunu**
-- dondurmaya devam eder (bu göç ona dokunmaz).
-- Ret `DIM_INTEGRITY:redaction_regions` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam taraması: tablo `case_id` taşımaz → `GUARDED_TABLES` **25 kalır**
-- (0044–0053 ile aynı sınıf — kural dosya kapsamı değil **bölge kanıtı
-- bütünlüğü**dür).
--
-- Dürüst sınırlar: (a) ayrıştırılmış **hiç sayfası olmayan** ya da ayrıştırması
-- **tamamlanmamış** (`running`/`partial`/`failed`/`cancelled`) sürümde sayfa
-- varlığı denetlenmez: yarım kalmış koşuda eksik sayfa «yalan» sayılmaz (maske
-- PDF'i zaten maskeler; metin eşlemesi isteyen yol ayrıca reddeder). (b)
-- `width`/`height` NULL olan eksende sınır denetlenmez; kutu biçimi yine
-- denetlenir. (c) Kutu sayfadaki
-- metni **gerçekten** kapsıyor mu doğrulanmaz (kesişim `outbound_terms_from_plan`
-- ve maskeleme doğrulamasının işidir). (d) Rotasyon denetlenmez: döndürülmüş
-- sayfa zaten maskede reddedilir (`plan_space_matches_page`/`rotation % 360`),
-- bu kapı yalnız kırpma kutusu uzayını ölçtüğü için rotasyonlu sayfada
-- yazımı durdurmaz. (e) v54 öncesi sayfa dışı bölgeler **silinmez** ve
-- donmuş oldukları için onarılamaz — temizlik ürün kararı; onarım yolu yeni
-- plandır. (f) `parse_run_id` boş metinse (`''`) bildirilmemiş sayılır ve en
-- yeni sayfalı koşuya düşülür. (g) Tetikleyicileri anahtarı olan saldırgan
-- düşürebilir. (h) Arayüz davranışı değişmedi; gerçek pencere + görsel QA
-- **insan bekliyor**.

DROP TRIGGER IF EXISTS redaction_region_evidence_guard_bi;

CREATE TRIGGER redaction_region_evidence_guard_bi
BEFORE INSERT ON redaction_regions
WHEN length(trim(NEW.id)) = 0
  OR NOT EXISTS (SELECT 1 FROM redaction_plans p
                  WHERE p.id = NEW.plan_id AND p.state = 'draft')
  OR NEW.page_index < 0
  OR NOT json_valid(NEW.quads)
  OR (CASE WHEN json_valid(NEW.quads)
           THEN CASE WHEN json_type(NEW.quads) = 'array' THEN 0 ELSE 1 END
           ELSE 0 END) = 1
  OR (CASE WHEN json_valid(NEW.quads)
           THEN CASE WHEN json_type(NEW.quads) = 'array'
                     THEN CASE WHEN json_array_length(NEW.quads) = 0
                               THEN 1 ELSE 0 END
                     ELSE 0 END
           ELSE 0 END) = 1
  OR EXISTS (
       SELECT 1 FROM json_each(
         CASE WHEN json_valid(NEW.quads)
              THEN CASE WHEN json_type(NEW.quads) = 'array'
                        THEN NEW.quads ELSE '[]' END
              ELSE '[]' END) q
        WHERE q.type <> 'array'
           OR (CASE WHEN q.type = 'array'
                    THEN json_array_length(q.value) ELSE 4 END) <> 4
           OR EXISTS (
                SELECT 1 FROM json_each(
                  CASE WHEN q.type = 'array' THEN q.value ELSE '[]' END) e
                 WHERE e.type NOT IN ('integer', 'real')))
  -- v54/2: her dörtlü pozitif alanlı bir dikdörtgen olmalı.
  OR EXISTS (
       SELECT 1 FROM json_each(
         CASE WHEN json_valid(NEW.quads)
              THEN CASE WHEN json_type(NEW.quads) = 'array'
                        THEN NEW.quads ELSE '[]' END
              ELSE '[]' END) q
        WHERE q.type = 'array'
          AND json_array_length(q.value) = 4
          AND (CAST(json_extract(q.value, '$[0]') AS REAL)
                 >= CAST(json_extract(q.value, '$[2]') AS REAL)
            OR CAST(json_extract(q.value, '$[1]') AS REAL)
                 >= CAST(json_extract(q.value, '$[3]') AS REAL)))
  -- v54/1: bölge, maskede kullanılacak koşuda var olan bir sayfayı göstermeli.
  -- Sayfa kümesi yalnız **tamamlanmış** (`succeeded`) koşuda belgelenir; yarım
  -- kalmış koşuda eksik sayfa yalan sayılmaz (maske PDF'i zaten maskeler).
  OR (
       EXISTS (
         SELECT 1 FROM parse_runs r
          WHERE r.id = COALESCE(
                  NULLIF(TRIM((SELECT p.parse_run_id FROM redaction_plans p
                                WHERE p.id = NEW.plan_id)), ''),
                  (SELECT pr.id FROM parse_runs pr
                     JOIN redaction_plans p2 ON p2.id = NEW.plan_id
                    WHERE pr.document_version_id = p2.document_version_id
                      AND EXISTS (SELECT 1 FROM pages x
                                   WHERE x.parse_run_id = pr.id)
                    ORDER BY pr.created_at DESC, pr.id DESC
                    LIMIT 1))
            AND r.status = 'succeeded')
       AND EXISTS (
         SELECT 1 FROM pages pg
          WHERE pg.parse_run_id = COALESCE(
                  NULLIF(TRIM((SELECT p.parse_run_id FROM redaction_plans p
                                WHERE p.id = NEW.plan_id)), ''),
                  (SELECT pr.id FROM parse_runs pr
                     JOIN redaction_plans p2 ON p2.id = NEW.plan_id
                    WHERE pr.document_version_id = p2.document_version_id
                      AND EXISTS (SELECT 1 FROM pages x
                                   WHERE x.parse_run_id = pr.id)
                    ORDER BY pr.created_at DESC, pr.id DESC
                    LIMIT 1)))
       AND NOT EXISTS (
         SELECT 1 FROM pages pg
          WHERE pg.parse_run_id = COALESCE(
                  NULLIF(TRIM((SELECT p.parse_run_id FROM redaction_plans p
                                WHERE p.id = NEW.plan_id)), ''),
                  (SELECT pr.id FROM parse_runs pr
                     JOIN redaction_plans p2 ON p2.id = NEW.plan_id
                    WHERE pr.document_version_id = p2.document_version_id
                      AND EXISTS (SELECT 1 FROM pages x
                                   WHERE x.parse_run_id = pr.id)
                    ORDER BY pr.created_at DESC, pr.id DESC
                    LIMIT 1))
            AND pg.page_index = NEW.page_index)
     )
  -- v54/3: kutu sayfanın kırpma kutusunda kalmalı (±1 punto yuvarlama payı).
  -- Boyut NULL ise karşılaştırma NULL'a düşer ve o eksen **denetlenmez**;
  -- ayrı bir `IS NOT NULL` cümlesi yazılmadı (etkisiz olurdu).
  OR EXISTS (
       SELECT 1 FROM pages pg
        WHERE pg.parse_run_id = COALESCE(
                NULLIF(TRIM((SELECT p.parse_run_id FROM redaction_plans p
                              WHERE p.id = NEW.plan_id)), ''),
                (SELECT pr.id FROM parse_runs pr
                   JOIN redaction_plans p2 ON p2.id = NEW.plan_id
                  WHERE pr.document_version_id = p2.document_version_id
                    AND EXISTS (SELECT 1 FROM pages x
                                 WHERE x.parse_run_id = pr.id)
                  ORDER BY pr.created_at DESC, pr.id DESC
                  LIMIT 1))
          AND pg.page_index = NEW.page_index
          AND EXISTS (
               SELECT 1 FROM json_each(
                 CASE WHEN json_valid(NEW.quads)
                      THEN CASE WHEN json_type(NEW.quads) = 'array'
                                THEN NEW.quads ELSE '[]' END
                      ELSE '[]' END) q
                WHERE q.type = 'array'
                  AND json_array_length(q.value) = 4
                  AND (CAST(json_extract(q.value, '$[0]') AS REAL) < -1.0
                    OR CAST(json_extract(q.value, '$[1]') AS REAL) < -1.0
                    OR CAST(json_extract(q.value, '$[2]') AS REAL) > pg.width + 1.0
                    OR CAST(json_extract(q.value, '$[3]') AS REAL) > pg.height + 1.0)))
  OR NEW.coordinate_space <> 'pdf_page_points'
  OR length(trim(NEW.category)) = 0
  OR (NEW.detected_text_hash IS NOT NULL
      AND length(trim(NEW.detected_text_hash)) = 0)
  OR NEW.user_decision <> 'redact'
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:redaction_regions');
END;
