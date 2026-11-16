-- 0045: **gönderim sicilinin ve aktarım onayının dönüşüm bağı kanıtlanabilir
-- olmalı** (INV-05 / şartname §737 ve §763).
--
-- Neden: `transfer_segments.transform_manifest_id` satırın «hangi maske
-- tanımıyla» gönderildiğini söyler; `list_transfer_segments`/`sent_segments`
-- bu kimliği `outbound_transforms`la **sol birleştirir** ve ekran türü
-- (`identity` / `mask_terms`), alıntı doğrulaması da özgün aralıklara geri
-- eşlemeyi buradan kurar (arayüz: Ai.tsx gönderim sicili). 0027 tabloyu
-- tanımladı, 0034/0038 satırın **sürümünü ve segmentini** kapıladı — ama
-- dönüşüm bağı hiçbir tetikleyici taşımıyordu (v44 taraması: gönderim/aktarım
-- zincirinde FK'siz ve kapısız kalan tek okuyucu-yazıcı bağ çifti; kalan
-- FK'siz sütunlar başka sınıf — `redaction_plans.transform_manifest_id` ve
-- `render_derivatives.*` okuyucusuz, `citation_checks.candidate_precedent_id`
-- emsal eşleştirme sicili).
--
-- `transfer_consents.outbound_transform_id` ise onayın **maske tanımıdır**
-- (0030: «maske onayın parçasıdır»): `start_ai_run` karşılaştırmayı bu satırın
-- `settings_hash`i üzerinden yapar. Bağ değiştirilebilirse onaylanan maske A
-- iken B ile (ya da maskesiz) gönderim kapıyı geçer — 0030'un kapattığı
-- "onaylanan içerik ≠ gönderilen içerik" açığı doğrudan SQL yolundan geri gelir.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v44; sonda
-- `~/.hermes/cache/scratch/dim-v45-probe/measure_v45.py`, log
-- `measure_before.log`):
--   (1) run1'in (onayı A maskesi) siciline **B maskesinin** dönüşümü yazıldı;
--   (2) **var olmayan** dönüşüm kimliği yazıldı (ekran "unknown" der);
--   (3) maskeli satır `transform_manifest_id=NULL` ile yazıldı — okuyucu onu
--       **kimlik** sayar, alıntı doğrulaması maskeli metni özgün sanar;
--   (4) `kind='identity'` dönüşüm satırı maskeli sicile bağlandı;
--   (5) var olan satırın dönüşümü UPDATE ile B maskesine çevrildi;
--   (6) dönüşüm bağı UPDATE ile **silindi** (NULL);
--   (7) sicil satırı **başka çalıştırmaya** taşındı (onay bağlamı değişti);
--   (8) onayın maske bağı UPDATE ile tA→tB çevrildi: maske kapısının
--       karşılaştırdığı `settings_hash` `h-a`→`h-b` oldu;
--   (9) onayın maske bağı NULL'a çekildi (yasal kayıt silindi);
--   (10) onaya **var olmayan** dönüşüm kimliği yazıldı;
--   (11) onaya `kind='identity'` dönüşüm bağlandı.
--   v44 taramasının bu sınıfı sayan alanı yok (kör): 4 uydurma/çapraz sicil
--   satırı ve 2 bozuk onay bağı hiçbir yerde görünmüyordu.
--
-- Yazılan kural (4 tetikleyici):
--   * `transfer_segments` BI — dolu `transform_manifest_id` **var olan** ve
--     `kind='mask_terms'` bir dönüşümü göstermeli (kimlik `NULL` ile temsil
--     edilir; çekirdek `identity` satırı yazmaz). Ayrıca çalıştırmanın onayı
--     **çözülebiliyorsa** bağ onayla aynı olmalı: onayın dönüşümü 1+ terimliyse
--     sicil satırı **aynı** kimliği göstermeli, 0 terimliyse `NULL` olmalı.
--     Böylece "onaylandı A, gönderildi B" iddiası yazılamaz.
--   * `transfer_segments` BU — `id`, `ai_run_id`, `transform_manifest_id`
--     **değişmez**. Satır hangi çalıştırmaya aitse ve hangi maskeyle
--     gönderildiyse öyle kalır; diğer sütunlar (mevcut kod hiç güncellemese de
--     ileride bir `verification_state` benzeri alan) takılmaz — v35 dersi.
--   * `transfer_consents` BI — dolu `outbound_transform_id` var olan ve
--     `kind='mask_terms'` dönüşümü göstermeli. `NULL` (0030 öncesi onay)
--     serbesttir: "bağ bilinmiyor" meşru bir durumdur ve çalıştırma zaten
--     reddedilir.
--   * `transfer_consents` BU — `outbound_transform_id` **değişmez**: onaylanan
--     maske tanımı sonradan ne genişletilebilir ne daraltılabilir ne de
--     silinebilir (yeni maske **yeni onay** ister; 0030 sözleşmesi).
--
-- Bilinçli sınırlar (dürüst):
--   (a) Denetim izi gevşekliği 0033/0034/0038/0043/0044 ile aynı sınıf
--       **değildir**: `outbound_transforms` satırları içerik adreslidir ve
--       hiçbir yol silmez (ON DELETE yok, silme yolu yok), bu yüzden
--       "var olmayan kimlik" burada denetim izi değil **kanıtlanabilir yalan**
--       sayıldı ve fail-closed kapıya takıldı.
--   (b) Onayın kendisi **var olmayan** bir çalıştırma/onay zinciri taşıyorsa
--       (0043'ün bilinçli gevşekliği) eşitlik kuralı boşta kalır.
--   (c) Bu tablolar `case_id` taşımaz → `scope_violations()` alanı
--       **eklenmez**, `GUARDED_TABLES` **25 kalır** (v44 ile aynı gerekçe:
--       kural dosya kapsamı değil, aktarım bütünlüğüdür).
--   (d) v45 öncesi yanlış bağlar **silinmez ve sayılmaz** (tarama alanı yok);
--       immutability onların UPDATE ile onarılmasını da engeller — temizlik
--       ürün kararı.
--   (e) Sicil metni (`outbound_text`/`outbound_hash`) bu dilimin dışında:
--       UPDATE ile değiştirilebilir kalır (sıradaki aday).
--   (f) Tetikleyicileri anahtarı olan saldırgan düşürebilir (uygulama hatasına
--       karşı).
--
-- Ret `RAISE(ABORT,'DIM_INTEGRITY:<tablo>')` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`** (0044 ile aynı önek; "dosya kapsamı ihlali" demek
-- yanlış olurdu).

CREATE TRIGGER transfer_segments_transform_guard_bi
BEFORE INSERT ON transfer_segments
WHEN (NEW.transform_manifest_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM outbound_transforms x
         WHERE x.id = NEW.transform_manifest_id AND x.kind = 'mask_terms'
      ))
   OR EXISTS (
        SELECT 1
          FROM ai_runs r
          JOIN transfer_consents c ON c.id = r.consent_id
          JOIN outbound_transforms x ON x.id = c.outbound_transform_id
         WHERE r.id = NEW.ai_run_id
           AND ((x.term_count > 0
                 AND (NEW.transform_manifest_id IS NULL
                      OR NEW.transform_manifest_id <> c.outbound_transform_id))
                OR (x.term_count = 0 AND NEW.transform_manifest_id IS NOT NULL))
      )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_segments');
END;

CREATE TRIGGER transfer_segments_transform_guard_bu
BEFORE UPDATE OF id, ai_run_id, transform_manifest_id ON transfer_segments
WHEN NEW.id IS NOT OLD.id
  OR NEW.ai_run_id IS NOT OLD.ai_run_id
  OR NEW.transform_manifest_id IS NOT OLD.transform_manifest_id
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_segments');
END;

CREATE TRIGGER transfer_consents_transform_guard_bi
BEFORE INSERT ON transfer_consents
WHEN NEW.outbound_transform_id IS NOT NULL AND NOT EXISTS (
       SELECT 1 FROM outbound_transforms x
        WHERE x.id = NEW.outbound_transform_id AND x.kind = 'mask_terms'
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_consents');
END;

CREATE TRIGGER transfer_consents_transform_guard_bu
BEFORE UPDATE OF outbound_transform_id ON transfer_consents
WHEN NEW.outbound_transform_id IS NOT OLD.outbound_transform_id
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_consents');
END;
