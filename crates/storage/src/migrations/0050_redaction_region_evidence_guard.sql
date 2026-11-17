-- 0050: **maskeleme bölgesi kanıtı** (redaction_regions) değişmez ve
-- doğuşta tutarlı olmalı.
--
-- Neden: bölge satırı **maskenin kendisidir** — hangi sayfada hangi
-- dikdörtgenin karartıldığı. Arayüz planın altında `regionCount`u («N bölge»),
-- açılınca sayfa kırılımını («Sayfa 3: 2 bölge») ve geometrisi okunamayan
-- bölge sayısını kırmızı uyarıyla basar (`screens/Redaction.tsx`;
-- `list_redaction_regions` → `rects: null`). Gönderim maskesi de terimlerini
-- bu satırlardan üretir: `outbound_terms_from_plan` yalnız
-- `user_decision='redact'` bölgelerin dikdörtgenleriyle kesişen bloklardan
-- metin çıkarır. Çekirdek bölgeyi **yalnız INSERT eder** (plan `draft`ken
-- `add_redaction_regions`; sonra plan `applied` olur) — tabloda düzeltme,
-- taşıma veya silme yolu yoktur. Şemada ise tablo **hiçbir tetikleyici
-- taşımıyordu** (0001'den beri): sayfa, geometri ve karar doğrudan SQL ile
-- serbestçe çevrilebiliyor, bölge başka plana (başka dosyaya) taşınabiliyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v49; sonda
-- `scratch/dim-v50-probe/measure_v50.py`, log `measure_before.log`): 26 kusur
-- yazımının **24'ü KABUL** (2 RED yalnız FK'nın kendisiydi):
--   (1) bölge **başka plana** taşındı (`plan_id='p2'` — başka dosyanın planı);
--   (2) sayfa numarası değiştirildi (2 → 0);
--   (3) dikdörtgen geometrisi değiştirildi (başka bölge taranır);
--   (4) karar `redact`→`keep` çevrildi (maske yolu o bölgeyi sessizce atlar);
--   (5) kategori boşaltıldı; (6) koordinat uzayı `image_pixels`e çevrildi;
--   (7) terim özeti değiştirildi; (8) kimlik değiştirildi;
--   (9) geometri bozuk JSON'a çevrildi (arayüz «okunamadı» der);
--   (10) negatif sayfalı bölge yazıldı;
--   (11) **kapanmış (`applied`) plana** yeni bölge eklendi — planın bölge
--        sayısı sonradan büyüdü;
--   (12) boş kimlik; (13) geçersiz JSON; (14) nesne; (15) boş dizi;
--   (16) düz sayı elemanı (`[1,2,3,4]`); (17) üçlü dikdörtgen;
--   (18) metinli koordinat (`[[1,2,3,"a"]]`); (19) yanlış koordinat uzayı;
--   (20) boş kategori; (21) boşluk kategori; (22) boş terim özeti;
--   (23) **doğuştan `keep`**; (24) **doğuştan `pending`**.
-- FK iki yazımı zaten reddediyordu (olmayan plan, boş plan kimliği); kapı
-- bunları **yedekler** (FK kapalı bağlantıda da doğru hüküm verir).
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — `id` dolu; plan **var ve `draft`** (bölge seti, plan
--     `applied` olmadan önce tamamlanır; kapanmış plana sonradan kanıt
--     eklenemez); `page_index` negatif değil; `quads` **geçerli JSON dizi**,
--     boş değil, her elemanı **tam dört sayı** (integer/real) — arayüzün
--     «geometri okunamadı» saydığı satır doğuşta yazılamaz; koordinat uzayı
--     `pdf_page_points` (çekirdek sabit yazar); kategori kırpılmış ve boş
--     değil; `detected_text_hash` NULL ya da kırpılmış-boş-değil;
--     `user_decision='redact'` (bu yolda doğan her bölge maskelenmiştir).
--   * UPDATE — sekiz sütunun **hepsi** değişmez: bölge doğduğu anda donar.
--     Değişmeyen yazma (`SET quads=quads`) takılmaz; düzeltme yolu yeni
--     plandır (çekirdekte de tek yol budur).
-- Ret `DIM_INTEGRITY:redaction_regions` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam taraması: tablo `case_id` taşımaz → `scope_violations()` alanı
-- **eklenmez** (`GUARDED_TABLES` **25 kalır**; 0044–0049 ile aynı sınıf —
-- kural dosya kapsamı değil **bölge kanıtı bütünlüğü**dür).
--
-- Dürüst sınırlar: (a) v50 öncesi tutarsız/bayat bölgeler **silinmez** ve
-- artık **onarılamaz** (donmuş) — temizlik ürün kararı. (b) Kapı bölgenin
-- **gerçekten maskelendiğini** doğrulamaz: yazan taraf bir kez yalan yazabilir
-- (plan `draft`ken sahte ama biçimce geçerli bir bölge eklenebilir) — kapı
-- sonraki değişikliği ve kapanmış plana eklemeyi durdurur. (c) `quads`
-- koordinatlarının sayfa sınırları içinde olduğu veya sayfa sayısıyla
-- tutarlılığı doğrulanmaz (sayfa sayısı planın bağlı olduğu ayrıştırmaya
-- bağlı, bölge satırı taşımıyor). (d) `detected_text_hash`in **içeriği**
-- (terimin gerçek özeti mi) doğrulanmaz — yalnız boş olamaz (SQLite'ta
-- SHA-256 yok). (e) `plan_id` dolu değilse zaten var olan plana çözülür;
-- ayrı bir "boş olamaz" cümlesi yazılmadı çünkü `EXISTS` onu kapsar.
-- (f) Tetikleyicileri anahtarı olan saldırgan düşürebilir. (g) Arayüz
-- davranışı değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

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
  OR NEW.coordinate_space <> 'pdf_page_points'
  OR length(trim(NEW.category)) = 0
  OR (NEW.detected_text_hash IS NOT NULL
      AND length(trim(NEW.detected_text_hash)) = 0)
  OR NEW.user_decision <> 'redact'
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:redaction_regions');
END;

CREATE TRIGGER redaction_region_evidence_guard_bu
BEFORE UPDATE OF id, plan_id, page_index, quads, coordinate_space, category,
                 detected_text_hash, user_decision
ON redaction_regions
WHEN NEW.id IS NOT OLD.id
  OR NEW.plan_id IS NOT OLD.plan_id
  OR NEW.page_index IS NOT OLD.page_index
  OR NEW.quads IS NOT OLD.quads
  OR NEW.coordinate_space IS NOT OLD.coordinate_space
  OR NEW.category IS NOT OLD.category
  OR NEW.detected_text_hash IS NOT OLD.detected_text_hash
  OR NEW.user_decision IS NOT OLD.user_decision
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:redaction_regions');
END;
