-- 0053: **kaynak bağının kanıtı** (source_anchors) doğuşta tutarlı olmalı ve
-- alıntı kimliği sonrasında değişmemeli.
--
-- Neden: `source_anchors` satırı **kalıcı kaynak bağıdır** (F06-T09) — bir
-- alıntının hangi belge sürümünde, hangi ayrıştırma koşusunda, hangi segmentte
-- ve hangi Unicode scalar aralığında durduğunu söyler. Kasadaki bütün kanıt
-- yolları bu satıra yaslanır:
--   * `locate_anchor` alıntıyı **segment metninde** bu kayıttaki ofsetle arar;
--     okuyucunun "kaynağa git" bağı fiziksel sayfayı `page_id`den çözer ve
--     `quote_intact` yalnız bu satırın alanlarıyla hesaplanır;
--   * `rebind_suggestion` alıntıyı güncel sürümde `exact_quote` ile arar;
--     yanlış alıntı metni "bulunamadı" hükmünü yanıltır;
--   * metin seçimi (`mark_from_selection`, `note_from_selection`) ve AI aday
--     terfisi (`ensure_anchor`) aynı `create_anchor` yolundan geçer;
--   * `claims`/`evidence_links`/`event_sources`/`tasks` bu kimliğe bağlanır
--     (0032); not/inceleme JSON listeleri bu kimlikleri taşır (0033).
-- Şemada bu tablonun kapısı yalnız **kapsam**tı (0031: sürüm/koşu/segment aynı
-- dosyada, sayfa aynı koşuda) ve `quote_hash` için yalnız
-- `CHECK (length = 64)` vardı.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v52; sonda
-- `scratch/dim-v53-probe/measure_v53.py`, log `measure_before.log`):
-- 40 yazımın **27 kusuru KABUL** (3'ü zaten RED — biri 0031 kapsam, biri
-- `length = 64` CHECK'i, biri 0031 segment kapsamı — ve 10 meşru yazma KABUL):
--   (1) doğuşta özet **64 hex değil** (`'Z'*64`), (2) **büyük harf** hex
--       (`'A'*64`), (3) alıntı **boş**, (4) alıntı **yalnız boşluk**,
--   (5) alıntı uzunluğu ofset aralığıyla **uyuşmuyor** (16 karakter ↔ 15),
--   (6) alıntı **iddia edilen ofsette değil** (0..16 yazıldı, alıntı gerçekte
--       8..24'te) — okuyucu yanlış aralığı vurgular, `locate_anchor` "kopmuş"
--       der ama kayıt bunu **yanlış konumla** söyler,
--   (8) `page_id` aynı koşuda ama **segmentin sayfası değil** (p3 ≠ p1) —
--       "kaynağa git" yanlış sayfayı açar,
--   (9) `document_version_id` **koşunun sürümü değil** (v2 ↔ r1/v1) — bağ
--       başka sürümün ekranında görünür (0031 yalnız dosya kapsamına bakar),
--   (10) **boş çapa kimliği**, (11) oluşturma damgası **boş**, (12) güncelleme
--       damgası **ISO değil** (`'dün'`),
--   (13) çapa **doğuştan `rebound`** (yeniden doğrulanmış iddiası; doğuşta
--       yalnız `active` meşrudur — `create_anchor` daima `'active'` yazar),
--   (14) çevre **öneki uydurma**, (15) çevre **soneki uydurma**,
--   (17) alıntı **başka segmentin metninden** (s3; konum s1'i gösterir);
--   (18) alıntı metni UPDATE ile çevrildi, (19) özet çevrildi, (20) başlangıç
--       ofseti çevrildi (uzunluk bozuldu), (21) bitiş ofseti çevrildi,
--   (22) sayfa **başka sayfaya** çevrildi, (23) sürüm **başka sürüme**
--       çevrildi (koşu sabit), (24) çapa kimliği çevrildi, (25) oluşturma
--       damgası çevrildi, (26) önek uyduruldu, (27) sonek uyduruldu,
--   (28) **doğrulanmamış çapa terfi ettirildi**: `active`→`unresolved`
--       (dürüst düşürme) sonra **konum değişmeden** `active` — "yeniden
--       doğrulandı" iddiası hiçbir doğrulama olmadan geri gelir.
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — `id` kırpılmış-dolu; `anchor_status` **`active`** (doğuşta
--     yeniden doğrulanmış/çözülememiş doğamaz); `exact_quote` boşluktan ibaret
--     değil ve `length(exact_quote) = end_offset - start_offset` (SQLite
--     `length()` metinde **karakter** sayar = çekirdeğin `chars().count()`u);
--     `quote_hash` **64 küçük harf hex** (CHECK 64'ü zorlar, karakter kümesi
--     burada — 0052'nin `original_sha256` deseni); iki damga ISO-8601
--     (`0049` ile aynı biçim); `document_version_id` **koşunun sürümü**;
--     `page_id` **segmentin sayfası** (NULL'lar `IS` ile eşleşir);
--     `exact_quote` **segment metninde tam o ofsette**; `prefix`/`suffix`
--     **segment metninden** üretilmiş çevre (yarıçap 24 — çekirdeğin
--     `context_around(..., 24)` sözleşmesi; start=0 → önek boş, metin sonu →
--     sonek boş).
--   * UPDATE — `id`, `exact_quote`, `quote_hash`, `created_at` **donar**
--     (yeniden bağlama aynı alıntıyı taşır; farklı alıntı yeni satırdır);
--     `updated_at` değişiyorsa ISO-8601 olmalı. **Terfi kapısı:** doğrulanmamış
--     (`active`/`rebound` dışı) bir çapa, **konumu değişmeden** yeniden
--     `active`/`rebound` olamaz (dürüst düşürme serbest; gerçek yeniden
--     bağlama konumu değiştirir ve aşağıdaki tutarlılık kapısından geçer).
--     **Tutarlılık kapısı:** konum/çevre sütunlarından biri değişiyorsa
--     `document_version_id` koşunun sürümü, `page_id` segmentin sayfası,
--     uzunluk = ofset farkı ve `exact_quote`/`prefix`/`suffix` **yeni segment
--     metniyle** birebir olmalı — yeniden bağlama ancak alıntının gerçekten
--     yeni konumda durduğu yazımı geçer. Değişmeyen yazma takılmaz.
-- Ret `DIM_INTEGRITY:source_anchors` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- **Çekirdek hizası:** `create_anchor` alıntıyı `verify_quote` ile
-- doğruluyordu; bu işlev **ipucu tutmazsa tüm metni tarar** ve tek eşleşmede
-- `Exact` döner — yani çağıran yanlış `start_offset` verse de satır yazılır
-- (ölçülen kusur (6): 0..16 aralığına 8..24'teki alıntı yazılabiliyordu).
-- Çekirdek **iddia edilen konumda birebir eşleşme** isteyecek şekilde
-- sıkılaştırıldı (`quote_starts_at`; `rebind_anchor`ın zaten yaptığı kontrol
-- tek tanıma alındı) → artık `E_QUOTE_NOT_FOUND` (yazma hiç denenmez).
--
-- Kapsam taraması: tablo `case_id` taşıyor ve **zaten** taranıyor → yeni sınıf
-- eklenmedi, `GUARDED_TABLES` **25 kalır** (0044–0052 ile aynı sınıf — kural
-- dosya kapsamı değil **kanıt bütünlüğü**dür). Tarama, kopmuş kaynağı
-- (`quote_intact=false`) meşru sayar; bu kapı **yazım anını** bağlar.
--
-- Dürüst sınırlar: (a) v53 öncesi tutarsız çapalar **silinmez**; alıntı kimliği
-- donduğu için eski satırlarda değiştirilemez, onarım yolu **yeni çapa**
-- yazmaktır — temizlik ürün kararı. (b) Kapı `quote_hash`in **alıntının**
-- SHA-256'sı olduğunu doğrulamaz (SQLite'ta SHA-256 yok) — biçim + alıntı/konum
-- tutarlılığı ölçülür; özet ile metni birlikte yazan yol tutarlı görünen
-- sahtecilik üretir. (c) Tutarlı bir "sessiz yeniden bağlama" (segment +
-- ofset + çevre hep birlikte, alıntı yeni segmentte gerçekten duruyorsa)
-- şemadan ayırt edilemez; `rebind_anchor`ın denetim kaydı (`audit_events`)
-- zorunluluğu şemada değil çekirdektedir. (d) SQLite `trim()` yalnız ASCII
-- boşluk kırpar; yalnız U+00A0'dan oluşan alıntı bu kapıdan geçer (çekirdek
-- `str::trim` reddeder). (e) SQLite `length()`/`substr()` NUL'da durur; PDF
-- metninde NUL beklenmez, bu sınır ölçülmedi. (f) Çevre yarıçapı **24**
-- şemada sözleşme olarak durur; çekirdek yarıçapı değiştirirse göç de
-- değişmelidir (tek kaynak `context_around` çağrısı). (g) `text_segments`
-- metni doğrudan UPDATE ile değiştirilirse eski çapalar "kopmuş" görünür
-- (`locate_anchor` bunu dürüstçe bildirir); bu kapı satırı silmez. (h) Korunan
-- sütunlar `anchor_status`/`updated_at` dışında tümüyledir; durum geçişleri
-- yalnız **terfi** yönünde kapılıdır. (i) Tetikleyicileri anahtarı olan
-- saldırgan düşürebilir. (j) Arayüz davranışı değişmedi; gerçek pencere +
-- görsel QA **insan bekliyor**. (k) Saf **kapsam** ihlalleri (segment/sayfa
-- başka koşuda, koşu başka dosyada) bilinçli olarak **0031'in sınıfında**
-- kalır: bu kapı yalnız aynı koşudaki satırın kanıtını ölçer, kapsam
-- ihlalini ayrıca sınıflandırmaz (iki kapı da aynı yazımı reddederdi ve ret
-- sınıfı tetikleyici sırasına bağlı kalırdı).

CREATE TRIGGER source_anchor_evidence_guard_bi
BEFORE INSERT ON source_anchors
WHEN length(trim(NEW.id)) = 0
  OR NEW.anchor_status <> 'active'
  OR length(trim(NEW.exact_quote)) = 0
  OR length(NEW.exact_quote) <> NEW.end_offset - NEW.start_offset
  OR NEW.quote_hash GLOB '*[^0-9a-f]*'
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR NEW.updated_at NOT LIKE '____-__-__T__:__:__%'
  -- Kapsam ihlalleri (koşu/segment/sayfa başka dosyada ya da başka koşuda)
  -- **0031'in sınıfına** bırakılır: bu kapı yalnız aynı koşudaki satırın
  -- kanıtını ölçer. Aksi hâlde iki kapı da aynı yazımı reddeder ve ret sınıfı
  -- (`DIM_SCOPE` ↔ `DIM_INTEGRITY`) tetikleyici sırasına bağlı kalırdı.
  OR (EXISTS (SELECT 1 FROM parse_runs pr WHERE pr.id = NEW.parse_run_id)
      AND NEW.document_version_id IS NOT (
            SELECT pr.document_version_id FROM parse_runs pr
             WHERE pr.id = NEW.parse_run_id))
  OR (EXISTS (SELECT 1 FROM text_segments s
               WHERE s.id = NEW.segment_id
                 AND s.parse_run_id = NEW.parse_run_id)
      AND (
        (NEW.page_id IS NULL
         OR EXISTS (SELECT 1 FROM pages p
                     WHERE p.id = NEW.page_id
                       AND p.parse_run_id = NEW.parse_run_id))
        AND NEW.page_id IS NOT (
              SELECT s.page_id FROM text_segments s WHERE s.id = NEW.segment_id)
        OR NEW.exact_quote IS NOT substr(
             (SELECT s.raw_text FROM text_segments s WHERE s.id = NEW.segment_id),
             NEW.start_offset + 1, length(NEW.exact_quote))
        OR NEW.prefix IS NOT substr(
             (SELECT s.raw_text FROM text_segments s WHERE s.id = NEW.segment_id),
             MAX(0, NEW.start_offset - 24) + 1, MIN(NEW.start_offset, 24))
        OR NEW.suffix IS NOT substr(
             (SELECT s.raw_text FROM text_segments s WHERE s.id = NEW.segment_id),
             NEW.end_offset + 1, 24)
      ))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:source_anchors');
END;

CREATE TRIGGER source_anchor_evidence_guard_bu
BEFORE UPDATE ON source_anchors
WHEN NEW.id IS NOT OLD.id
  OR NEW.exact_quote IS NOT OLD.exact_quote
  OR NEW.quote_hash IS NOT OLD.quote_hash
  OR NEW.created_at IS NOT OLD.created_at
  OR (NEW.updated_at IS NOT OLD.updated_at
      AND NEW.updated_at NOT LIKE '____-__-__T__:__:__%')
  OR (OLD.anchor_status NOT IN ('active', 'rebound')
      AND NEW.anchor_status IN ('active', 'rebound')
      AND NEW.document_version_id IS OLD.document_version_id
      AND NEW.parse_run_id IS OLD.parse_run_id
      AND NEW.segment_id IS OLD.segment_id
      AND NEW.start_offset IS OLD.start_offset
      AND NEW.end_offset IS OLD.end_offset)
  OR ((NEW.document_version_id IS NOT OLD.document_version_id
       OR NEW.parse_run_id IS NOT OLD.parse_run_id
       OR NEW.segment_id IS NOT OLD.segment_id
       OR NEW.start_offset IS NOT OLD.start_offset
       OR NEW.end_offset IS NOT OLD.end_offset
       OR NEW.page_id IS NOT OLD.page_id
       OR NEW.prefix IS NOT OLD.prefix
       OR NEW.suffix IS NOT OLD.suffix)
      AND (length(NEW.exact_quote) <> NEW.end_offset - NEW.start_offset
        OR (EXISTS (SELECT 1 FROM parse_runs pr WHERE pr.id = NEW.parse_run_id)
            AND NEW.document_version_id IS NOT (
                  SELECT pr.document_version_id FROM parse_runs pr
                   WHERE pr.id = NEW.parse_run_id))
        OR (EXISTS (SELECT 1 FROM text_segments s
                     WHERE s.id = NEW.segment_id
                       AND s.parse_run_id = NEW.parse_run_id)
            AND (
              (NEW.page_id IS NULL
               OR EXISTS (SELECT 1 FROM pages p
                           WHERE p.id = NEW.page_id
                             AND p.parse_run_id = NEW.parse_run_id))
              AND NEW.page_id IS NOT (
                    SELECT s.page_id FROM text_segments s
                     WHERE s.id = NEW.segment_id)
              OR NEW.exact_quote IS NOT substr(
                   (SELECT s.raw_text FROM text_segments s
                     WHERE s.id = NEW.segment_id),
                   NEW.start_offset + 1, length(NEW.exact_quote))
              OR NEW.prefix IS NOT substr(
                   (SELECT s.raw_text FROM text_segments s
                     WHERE s.id = NEW.segment_id),
                   MAX(0, NEW.start_offset - 24) + 1, MIN(NEW.start_offset, 24))
              OR NEW.suffix IS NOT substr(
                   (SELECT s.raw_text FROM text_segments s
                     WHERE s.id = NEW.segment_id),
                   NEW.end_offset + 1, 24)
            ))))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:source_anchors');
END;
