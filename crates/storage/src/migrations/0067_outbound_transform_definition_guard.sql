-- 0067: **maske tanımı içerik adreslidir** — `outbound_transforms` satırı
-- «şu ayarların özeti `settings_hash`, maskede **şu kadar** terim var, yer
-- tutucu **şu**» iddiasıdır ve doğuşta kanıtlı olmalı, sonrasında
-- **değişmemeli**.
--
-- Neden: 0027 tabloyu **içerik adresli** tanımladı (`settings_hash` UNIQUE) ve
-- 0045 bu varsayıma **yaslandı**: kendi kapanış notunda «`outbound_transforms`
-- satırları içerik adreslidir ve hiçbir yol silmez» diye yazdı, bu yüzden
-- «var olmayan dönüşüm kimliği»ni *denetim izi* değil **kanıtlanabilir yalan**
-- sayıp fail-closed kapıya taktı. Ama bu varsayımın **hiçbir zorlayıcısı
-- yoktu**: tablonun 0027'den beri tek kapısı `kind` CHECK'i ve UNIQUE
-- indeksidir; `settings_hash` biçimi, `term_count` işareti, `placeholder`
-- biçimi, `created_at` damgası denetlenmiyordu ve satır **her sütunuyla**
-- çevrilebiliyor, **silinebiliyordu**.
--
-- Okuyucular bu satıra doğrudan yaslanır:
--   * `start_ai_run` onayın maskesini `SELECT settings_hash ... WHERE id=?`
--     ile okur ve gönderilecek maskenin özetiyle karşılaştırır (0030/0045:
--     «maske onayın parçasıdır»). `settings_hash` çevrilebilirse onaylanan
--     maske A iken B (ya da maskesiz) gönderim kapıyı **geçer**.
--   * `list_transfer_segments`/`sent_segments` dönüşümü **sol birleştirir**;
--     ekran türü (`identity` / `mask_terms`) ve terim sayısını buradan basar
--     (Ai.tsx: «maskelendi · N terim» / «kimlik»). `kind` çevrilebilirse
--     maskeli gönderim **maskesiz** görünür; `term_count` çevrilebilirse
--     onayın terim sayısı yalan olur (0045'in eşitlik kuralı da bu sayıya
--     bakar).
--   * Silinirse onayın maske bağı **çözülemez** hâle gelir (start_ai_run ham
--     veritabanı hatası verir), sicil satırı «bilinmeyen tür»e düşer.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v66; sonda
-- `scratch/dim-v67-probe/measure_v67.py`, loglar `measure_before.log` /
-- `measure_after.log`): **47 yazımın 44'ü KABUL** — 36 kusur (27 doğum + 6
-- UPDATE + 3 DELETE) + 8 meşru yol; 3 RED yalnız mevcut kapılardan (`kind`
-- CHECK, iki UNIQUE çakışması). Doğum (28 saldırı, biri CHECK'te zaten RED): boş/boşluk/kırpılmamış/65 hane
-- kimlik; boş/boşluk/kırpılmamış/63 hane/**büyük harf**/hex olmayan/LF/CR/TAB
-- `settings_hash`; **negatif `term_count`**; **`identity` + 3 terim** çelişkisi;
-- boş/boşluk/kırpılmamış/LF/CR/TAB/65 hane `placeholder`; `created_at`
-- `dün`/boş/kırpılmamış/**olmayan gün** (`2026-02-30`)/**olmayan saat**
-- (`99:00`)/TR biçimi/**boşluk ayırıcı** (`2026-01-01 00:00:00Z`). UPDATE (6):
-- hash çevirme, `kind` çevirme, terim sayısı 1→0 ve 1→9, `placeholder`,
-- `created_at` 2030, `id`. DELETE (3): **onayla** bağlı, **sicille** bağlı ve
-- **yalnız sicille** bağlı (onayı çözülemeyen çalıştırma) tanım silindi.
-- Düzeltmeden sonra: **39 RED · 8 KABUL · sapma 0** (sekiz kabulün hepsi
-- meşru). Dürüst olay: ilk sonda koşusunda «büyük harf hash» saldırısı
-- `"2"*64`ün büyük harfiydi (rakamda büyük/küçük yok) ve CRLF tek saldırıydı
-- (LF/CR mutantı yalıtılamıyordu) → saldırılar `"AB"*32`, LF ve CR ayrı, boşluk
-- ayırıcı damga eklendi; iki taraf **yeniden ölçüldü**, ilk koşunun hükümleri
-- kanıt sayılmadı.
--
-- Yazılan kural (3 tetikleyici):
--   * BI — `id` kırpılmış-dolu ≤64; `kind` zaten CHECK (kapalı küme);
--     `settings_hash` **64 küçük harf hex** (içerik adresi); `term_count ≥ 0`;
--     **`identity` ⇒ `term_count = 0`** (kimlik satırı «terim maskeledim»
--     diyemez); `placeholder` kırpılmış-dolu ≤64, CR/LF/TAB yok;
--     `created_at` ISO-8601 + gerçek gün/saat (0066 deseni).
--   * BU — **altı sütun donar** (`id`, `kind`, `settings_hash`, `term_count`,
--     `placeholder`, `created_at`): içerik adresli tanım yazıldıktan sonra
--     değişmez. No-op takılmaz (değer aynıysa yazma geçer).
--   * BD — **canlı tanım silinemez**: `transfer_consents.outbound_transform_id`
--     ya da `transfer_segments.transform_manifest_id` hâlâ gösteriyorsa satır
--     silinemez (onay/sicil bağı başka sütunda donduğu için sessiz yetim
--     kalamaz). Referanssız tanım silinebilir (bugün ürün yolu yok).
-- Ret `DIM_INTEGRITY:outbound_transforms` → `E_DB_INTEGRITY`; tablo `case_id`
-- taşımaz → `GUARDED_TABLES` **25 kalır**.
--
-- Çekirdek hizası: `ensure_outbound_transform` özeti **kendisi** üretir
-- (`sha256`, 64 küçük hex), `kind='mask_terms'`, yer tutucu sabiti ve
-- `now_iso()` yazar — kural ürünün yazdığı biçimi **kapsar**. Ek olarak
-- `start_ai_run`, onayın dönüşüm satırı **çözülemiyorsa** ham veritabanı
-- hatası yerine `E_AI_CONSENT_UNBOUND` döner (fail-closed, tek anlamlı kod).
--
-- Dürüst sınırlar (kapanışta belgeye de yazılır): (a) `settings_hash`in
-- gerçekten o terim listesinin SHA-256'sı olduğu **yeniden hesaplanmaz**
-- (SQLite'ta SHA-256 yok) — biçim + değişmezlik ölçülür; (b) `placeholder`
-- değerinin (`[MASKED]`) ürün sabitiyle aynı olduğu denetlenmez, yalnız
-- biçimi; (c) `term_count`in gerçek terim sayısı olduğu doğrulanmaz (özetten
-- türetilemez) — yalnız işaret + `identity` tutarlılığı; (d) v67 öncesi
-- tutarsız satırlar silinmez/düzeltilmez, kimlik sütunları donduğu için
-- **onarılamaz** — onarım yolu yeni tanım (yeni maske) ve yeni onay;
-- (e) tetikleyicileri anahtarı olan saldırgan düşürebilir; (f) onay satırının
-- silinme yolu üründe yok — varsa transform BD kapısı yüzünden silinemez
-- (içerik adresli satırlar küçüktür, bilinçli).

CREATE TRIGGER outbound_transform_definition_guard_bi
BEFORE INSERT ON outbound_transforms
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR length(NEW.settings_hash) <> 64
  OR NEW.settings_hash GLOB '*[^0-9a-f]*'
  OR NEW.term_count < 0
  OR (NEW.kind = 'identity' AND NEW.term_count <> 0)
  OR length(trim(COALESCE(NEW.placeholder, ''))) = 0
  OR NEW.placeholder IS NOT trim(NEW.placeholder)
  OR length(NEW.placeholder) > 64
  OR instr(NEW.placeholder, char(10)) > 0
  OR instr(NEW.placeholder, char(13)) > 0
  OR instr(NEW.placeholder, char(9)) > 0
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.created_at, 1, 10)) IS NOT substr(NEW.created_at, 1, 10)
  OR time(substr(NEW.created_at, 12, 8)) IS NOT substr(NEW.created_at, 12, 8)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:outbound_transforms');
END;

CREATE TRIGGER outbound_transform_definition_guard_bu
BEFORE UPDATE ON outbound_transforms
WHEN NEW.id IS NOT OLD.id
  OR NEW.kind IS NOT OLD.kind
  OR NEW.settings_hash IS NOT OLD.settings_hash
  OR NEW.term_count IS NOT OLD.term_count
  OR NEW.placeholder IS NOT OLD.placeholder
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:outbound_transforms');
END;

CREATE TRIGGER outbound_transform_definition_guard_bd
BEFORE DELETE ON outbound_transforms
WHEN EXISTS (
       SELECT 1 FROM transfer_consents c
        WHERE c.outbound_transform_id = OLD.id
     )
  OR EXISTS (
       SELECT 1 FROM transfer_segments s
        WHERE s.transform_manifest_id = OLD.id
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:outbound_transforms');
END;
