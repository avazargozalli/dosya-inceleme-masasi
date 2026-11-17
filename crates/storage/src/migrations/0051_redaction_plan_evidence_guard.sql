-- 0051: **maskeleme planının kanıt bağı** (redaction_plans) doğuşta tutarlı
-- olmalı ve sonrasında değişmemeli.
--
-- Neden: plan satırı maskelemenin **künyesidir** — hangi belge sürümünün
-- hangi render'ına ait olduğu (`render_hash`), hangi çıktıyı ürettiği
-- (`output_version_id` + silinme izi `output_version_ref`) ve kapanıp
-- kapanmadığı (`state`). Üç okuma yolu bu sütunlara yaslanır:
--   * `screens/Redaction.tsx` render özetinin ilk 16 hanesini basar
--     (`plan.renderHash`), yani «bu geometri **bu** render'a ait» iddiası
--     ekranda görünür;
--   * yeniden inceleme nedeni `render_changed` **yalnız** bu sütunla
--     `document_versions.original_sha256` karşılaştırılarak hesaplanır
--     (`get_redaction_plan`/`list_redaction_plans`): özet çevrilebilirse
--     uyarı sessizce kaybolur;
--   * `output_link_state` (`linked`/`removed`) bağdan ve **izden** okunur —
--     `0029` «çıktı vardı, sonra silindi» ile «hiç bağlanmadı»yı ayırmak için
--     yazıldı; iz sahtelenebilirse ayrım anlamsızlaşır.
-- Çekirdek planı **yalnız** `create_redaction_plan` ile doğurur ve
-- `render_hash` olarak **kaynak sürümün `original_sha256`**sini yazar
-- (`commands.rs`), `output_version_id` verilmişse çıktının kaynağın **alt
-- sürümü** olduğunu doğrular. Sonrasında tek meşru yazma
-- `add_redaction_regions`ın `draft`→`applied` geçişidir. Şemada plan
-- tablosunun kapısı ise yalnız **kapsam**tı (0031/0034: sürüm/ayrıştırma/
-- çıktı aynı dosyada) — kanıt sütunları ve kapanış serbestti.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v50; sonda
-- `scratch/dim-v51-probe/measure_v51.py`, log `measure_before.log`): 26 kusur
-- yazımının **26'sı KABUL** (meşru 8 yazma da KABUL — taban 34/34):
--   (1) doğuşta `render_hash` kaynak sürümün özeti **değil** (`'h'`, hatta
--       başka 64 hex) — renew «render değişti» uyarısı hiç doğmaz;
--   (2) koordinat uzayı `page`; (3) çıktı profili boş; (4) boşluk;
--   (5) `plan_version=0`; (6) `created_at=''`; (7) `updated_at=''`;
--   (8) çıktı izi **başka** kimlik (`ref='v3'`, bağ `'vout'`);
--   (9) bağ dolu iken iz NULL (silinme izi yok); (10) **çıktı = kaynağın
--       kendisi**; (11) boş plan kimliği; (12) bağ yokken **uydurma** iz;
--   (13) `render_hash` UPDATE ile çevrildi (uyarı susturuldu);
--   (14) koordinat uzayı çevrildi; (15) çıktı profili çevrildi;
--   (16) plan sürümü çevrildi; (17) `created_at` geriye alındı;
--   (18) plan **aynı dosyada başka belgeye** taşındı (kapsam kapısı aynı
--        dosyayı serbest bırakır) — «hangi belgede maskeleme yapıldı»
--        yanıtı değişti;
--   (19) plan başka belgenin **ayrıştırmasına** bağlandı;
--   (20) sicil satırı **başka dosyaya** taşındı (`case_id`+sürüm birlikte);
--   (21) çıktı bağı aynı dosyada **başka sürüme** çevrildi;
--   (22) çıktı bağı **sonradan dolduruldu** (doğuşta yoktu);
--   (23) çıktı **izi** başka kimliğe çevrildi;
--   (24) **kapanmış plan yeniden açıldı** (`applied`→`draft`) — v50'nin
--        «kapanmış plana bölge eklenemez» cümlesi bu geçişle boşa düşüyordu;
--   (25) `applied`→`approved`; (26) doğuştan `applied` yazımı sayılmadı —
--        aşağıdaki (h) sınırına bakınız.
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — `id` kırpılmış-dolu; `render_hash` **kaynak sürümün
--     `original_sha256`si** (kanıt bağı: geometri bu render'a aittir);
--     koordinat uzayı `pdf_page_points` (çekirdek sabit yazar, v50 bölge
--     kapısı da aynı uzayı ister); çıktı profili kırpılmış-dolu;
--     `plan_version ≥ 1`; `created_at`/`updated_at` kırpılmış-dolu;
--     `output_version_ref` **ya NULL ya da `output_version_id` ile aynı**
--     (silinme izi bağın kimliğidir, başka kimlik değil); çıktı bağı
--     kaynak sürümün **kendisi olamaz** (çekirdek de reddeder).
--   * UPDATE — `id`, `case_id`, `document_version_id`, `parse_run_id`,
--     `render_hash`, `coordinate_space`, `output_profile`, `plan_version`,
--     `created_at`, `output_version_ref` **donar**. `output_version_id`
--     yalnız **NULL'a çözülebilir** (`0028`in `ON DELETE SET NULL`ı: çıktı
--     silinince bağ çözülür, iz kalır) — başka sürüme çevrilemez, NULL'dan
--     doldurulamaz. `state='applied'` **terminaldir**: uygulanmış maskeleme
--     yeniden açılamaz (v50 kapanışı buna dayanır); diğer geçişler serbest,
--     `reviewer` ve `updated_at` serbest. Değişmeyen yazma takılmaz.
-- Ret `DIM_INTEGRITY:redaction_plans` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam taraması: tablo zaten `case_id` taşıyor ve taranıyor → yeni sınıf
-- eklenmedi, `GUARDED_TABLES` **25 kalır** (0044–0050 ile aynı sınıf — kural
-- dosya kapsamı değil **plan kanıtı bütünlüğü**dür).
--
-- Dürüst sınırlar: (a) v51 öncesi tutarsız planlar **silinmez** ve artık
-- **onarılamaz** (donmuş) — temizlik ürün kararı. (b) Kapı `render_hash`in
-- gerçekten o render'ın özeti olduğunu **yeniden hesaplamaz** (SQLite'ta
-- SHA-256 yok): doğuşta kaynak sürümün sütunuyla **eşitlik** ölçülür; sürüm
-- özeti de yanlışsa plan onu devralır. (c) `document_versions.original_sha256`
-- biçimi (64 hex) burada denetlenmez. (d) `output_version_id`nin kaynağın
-- **alt sürümü** olduğu bu kapıda doğrulanmaz (çekirdek doğrular; kapsam
-- kapısı yalnız aynı dosyayı ister) — bilinçli: geçmiş fikstürlerde alt sürüm
-- bağı her yerde kurulu değil, kural olsaydı okuma yolları sınanamazdı.
-- (e) Doğuşta `state` serbesttir: `applied` doğan bir plan (bölgesiz) «uygulandı»
-- der — kapanış kapısı bunu **durdurmaz**, yalnız sonradan açılmasını durdurur;
-- geçmiş kasaları taklit eden yükseltme fikstürleri bu yüzden serbest kalmalı.
-- (f) `updated_at ≥ created_at` sırası denetlenmez (yalnız ikisi de dolu).
-- (g) Tetikleyicileri anahtarı olan saldırgan düşürebilir. (h) Arayüz
-- davranışı değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

CREATE TRIGGER redaction_plan_evidence_guard_bi
BEFORE INSERT ON redaction_plans
WHEN length(trim(NEW.id)) = 0
  OR NEW.render_hash IS NOT (
       SELECT v.original_sha256 FROM document_versions v
        WHERE v.id = NEW.document_version_id)
  OR NEW.coordinate_space <> 'pdf_page_points'
  OR length(trim(NEW.output_profile)) = 0
  OR NEW.plan_version < 1
  OR length(trim(NEW.created_at)) = 0
  OR length(trim(NEW.updated_at)) = 0
  OR NEW.output_version_ref IS NOT NEW.output_version_id
  OR (NEW.output_version_id IS NOT NULL
      AND NEW.output_version_id IS NEW.document_version_id)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:redaction_plans');
END;

CREATE TRIGGER redaction_plan_evidence_guard_bu
BEFORE UPDATE ON redaction_plans
WHEN NEW.id IS NOT OLD.id
  OR NEW.case_id IS NOT OLD.case_id
  OR NEW.document_version_id IS NOT OLD.document_version_id
  OR NEW.parse_run_id IS NOT OLD.parse_run_id
  OR NEW.render_hash IS NOT OLD.render_hash
  OR NEW.coordinate_space IS NOT OLD.coordinate_space
  OR NEW.output_profile IS NOT OLD.output_profile
  OR NEW.plan_version IS NOT OLD.plan_version
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.output_version_ref IS NOT OLD.output_version_ref
  OR (NEW.output_version_id IS NOT OLD.output_version_id
      AND NEW.output_version_id IS NOT NULL)
  OR (OLD.state = 'applied' AND NEW.state IS NOT OLD.state)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:redaction_plans');
END;
