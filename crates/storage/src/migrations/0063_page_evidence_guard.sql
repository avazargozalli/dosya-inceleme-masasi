-- 0063: **sayfa kanıtı** — `pages` satırı «bu sayfa şu geometride, şu kökenle
-- okundu» iddiasıdır ve doğuşta iyi biçimli olmalı, sonrasında **değişmemeli**.
--
-- Neden: aynı satır dört yerde kanıt olarak okunur:
--   * `text_origin` — arama `ocr`/`mixed` filtresi (`search_page_windowed`
--     `p.text_origin IN ('ocr','mixed')`), dual reader rozeti ve kapsam
--     raporunun «OCR kullanılan sayfa» sayımı (`sum(CASE WHEN p.text_origin
--     IN ('ocr','mixed') ...)`) **doğrudan** bu sütundan gelir. «ocr»→«native»
--     çeviren satır, OCR'sız görünen bir belge üretir.
--   * `width`/`height`/`rotation`/`coordinate_space` — maskeleme bölgesi sayfa
--     sınırı kapısı (v54) bölgeyi **bu** kutuya göre doğrular; ekran kutuları bu
--     geometriyle çizer. Doğrulandıktan sonra kutu küçültülürse «sayfa içinde»
--     hükmü geriye dönük bozulur, maske sayfa dışına taşar (hiçbir şey
--     maskelenmez) ya da fazla maskeler.
--   * `quality_flags` — okuma uyarıları ekranda bu listelenir.
--   * `page_label` — basılı etiket; fiziksel numaradan türetilmez (v4 kuralı).
-- Sayfa metni `text_segments`te ve orada kapsam kapılı; bu tablo metin
-- taşımaz, **hangi sayfanın hangi koşuda hangi geometriyle okunduğunu** taşır.
-- Tablo 0001'den beri tetikleyicisizdi: tek kapı `page_index >= 0` CHECK'i ve
-- (`parse_run_id`,`page_index`) UNIQUE'iydi.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v62; sonda
-- `scratch/dim-v63-probe/measure_v63.py`, log `measure_before.log`):
-- 47 yazımın **42'si KABUL**, 5'i zaten RED (negatif indeks CHECK, bilinmeyen
-- köken CHECK, var olmayan blob/koşu FK, mükerrer sayfa UNIQUE); 7 meşru /
-- zararsız yol (4 meşru doğum, no-op UPDATE, 2 silme) da KABUL. Yani **35
-- kusur** yazılabiliyordu. 23 doğum kusuru: **NULL** / boş / boşluk /
-- kırpılmamış / 65 haneli kimlik (SQLite'ta `TEXT PRIMARY KEY` NULL'a izin
-- verir); sayfa indeksi / genişlik / yükseklik sütunlarına **metin** (SQLite
-- karşılaştırmasında `TEXT` her zaman sayıdan büyüktür → aralık cümleleri tek
-- başına yetmez); boş ve boşluk etiket; **sıfır ve negatif**
-- genişlik/yükseklik; negatif ve 360 döndürme; boş/boşluk/kırpılmamış
-- koordinat uzayı; bozuk JSON, nesne ve metin olmayan öğe taşıyan uyarı
-- listesi.
-- 12 UPDATE kusuru: sayfanın başka indekse ve **başka koşuya** taşınması,
-- `ocr`→`native` çevrilmesi, geometrinin küçültülmesi/sıfırlanması,
-- döndürme/uzay/uyarı/etiket değişimi, kimliğin yeniden adlandırılması, türev
-- blob bağlanması. (Silme bu dilimin dışında: dosya/sürüm silme ürün yoludur
-- ve `plaintext` kaskad testi içeriğin temizlendiğini bu yolla ölçer; sayfa
-- satırı silinemez yapılsaydı o güvenlik testi düşerdi. Dürüst sınır olarak
-- yazıldı.)
--
-- Yazılan kural (2 tetikleyici):
--   * BI — `id` kırpılmış-dolu ≤64; `page_index >= 0`; `rotation` 0..359;
--     `page_label` NULL ya da kırpılmış-dolu; `width`/`height` NULL ya da
--     **pozitif** (sıfır/negatif alanlı sayfa kutu olamaz); `coordinate_space`
--     NULL ya da kırpılmış-dolu; `quality_flags` **metin dizisi** (v60'ın
--     `included_ids` deseni: `json_valid` yoksa da reddet).
--   * BU — on bir kanıt sütunu **donar**; no-op takılmaz. Bugün
--     `derivative_blob_id`i yazan ürün yolu yok (yalnız `ON DELETE SET NULL`);
--     yine de donar, çünkü sayfanın gösterim görüntüsü bağlanırsa hangi
--     görüntünün kanıt olduğu sonradan değişmemelidir.
-- Ret `DIM_INTEGRITY:pages` → `E_DB_INTEGRITY`; tablo `case_id` taşımaz →
-- `GUARDED_TABLES` **25 kalır**.
CREATE TRIGGER page_evidence_guard_bi
BEFORE INSERT ON pages
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:pages')
  WHERE NEW.id IS NULL
     OR NEW.id IS NOT trim(NEW.id)
     OR trim(NEW.id) = ''
     OR length(NEW.id) > 64
     OR typeof(NEW.page_index) <> 'integer'
     OR NEW.page_index < 0
     OR NEW.rotation < 0
     OR NEW.rotation > 359
     OR (NEW.page_label IS NOT NULL
         AND (NEW.page_label IS NOT trim(NEW.page_label)
              OR trim(NEW.page_label) = ''))
     OR (NEW.width IS NOT NULL
         AND (typeof(NEW.width) NOT IN ('integer', 'real')
              OR NOT (NEW.width > 0.0)))
     OR (NEW.height IS NOT NULL
         AND (typeof(NEW.height) NOT IN ('integer', 'real')
              OR NOT (NEW.height > 0.0)))
     OR (NEW.coordinate_space IS NOT NULL
         AND (NEW.coordinate_space IS NOT trim(NEW.coordinate_space)
              OR trim(NEW.coordinate_space) = ''))
     OR (CASE WHEN json_valid(NEW.quality_flags)
              THEN json_type(NEW.quality_flags) ELSE 'x' END) <> 'array'
     OR EXISTS (
          SELECT 1 FROM json_each(NEW.quality_flags)
           WHERE type <> 'text'
        );
END;

CREATE TRIGGER page_evidence_guard_bu
BEFORE UPDATE ON pages
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:pages')
  WHERE NEW.id IS NOT OLD.id
     OR NEW.parse_run_id IS NOT OLD.parse_run_id
     OR NEW.page_index IS NOT OLD.page_index
     OR NEW.page_label IS NOT OLD.page_label
     OR NEW.width IS NOT OLD.width
     OR NEW.height IS NOT OLD.height
     OR NEW.rotation IS NOT OLD.rotation
     OR NEW.coordinate_space IS NOT OLD.coordinate_space
     OR NEW.text_origin IS NOT OLD.text_origin
     OR NEW.quality_flags IS NOT OLD.quality_flags
     OR NEW.derivative_blob_id IS NOT OLD.derivative_blob_id;
END;
