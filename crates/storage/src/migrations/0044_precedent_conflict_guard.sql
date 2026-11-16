-- 0044: **emsal sürümünün çakışma işareti kendi emsaline bağlı kalır**.
--
-- Neden: `identity_conflict` arayüzde görünen tek çakışma iddiasıdır
-- (`screens/Precedents.tsx` → `versionConflictText` rozeti); `conflict_with_version_id`
-- iddianın karşı tarafını gösterir (F12-T06: "kullanıcı iki metni
-- karşılaştırabilsin"). Çekirdek ikisini birlikte ve doğru yazar
-- (`import_precedent_copy`: aynı künye + farklı SHA-256), ama **şemada kapı
-- yoktu**: sütunlar serbestti ve tablo hiçbir tetikleyici taşımıyordu
-- (v43 taraması: `precedent_versions` FK'siz + tetikleyicisiz).
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v43; sonda
-- `scratch/dim-v44-probe/measure_v44.py`, log `measure_before.log`):
--   (1) A emsalinin sürümü **B emsalinin sürümüyle** çakıştığını iddia etti
--       (`conflict_with_version_id='vB1'`, bayrak=1 → KABUL);
--   (2) sürüm **kendini** çakıştığı sürüm gösterdi (`v-self → v-self`);
--   (3) "aynı künye, **farklı metin**" rozeti **aynı** `body_hash` ile yazıldı
--       — rozet ekranda yalan söylüyordu;
--   (4) `identity_conflict=3` yazılabildi (her sıfır-dışı değer rozeti açar);
--   (5) işaret dolu + bayrak 0 → okuyucu işareti taşır, rozet görünmez;
--   (6) bayrak 1 + işaret NULL → rozetin karşılığı yok;
--   (7) var olan sürümün işareti UPDATE ile B emsaline çevrildi;
--   (8) `body_hash` UPDATE ile değiştirildi (iddia sonradan "doğru" ya da
--       "yanlış" hâle getirilebildi);
--   (9) sürüm **başka emsale taşındı** (`precedent_id` UPDATE): ona işaret eden
--       satırlar çapraz kaldı.
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT: `identity_conflict` 0/1 olmalı; rozet ile işaret **birlikte**
--     yazılır (`= 1` ⇔ `conflict_with_version_id IS NOT NULL`); işaret varsa
--     hedef kendisi olamaz, **var olan** hedef aynı emsalin sürümü olmalı ve
--     `body_hash` **farklı** olmalı ("farklı metin" iddiası kanıtlanabilir).
--     Ayrıca **ters yön**: yeni satırın kimliğine işaret eden eski bir satır
--     varsa, yeni satır da o iddiayı tutmalı (aynı emsal + farklı hash) —
--     yoksa "hedef sonradan yazıldı" boşluğu doğardı.
--   * UPDATE OF id, precedent_id, body_hash, identity_conflict,
--     conflict_with_version_id: bu beş sütun **değişmez**. Sürüm satırı
--     yazıldıktan sonra taşınmaz; metin özeti ve çakışma iddiası sonradan
--     düzeltilemez (mevcut kod sürümü yalnız INSERT eder; `verification_state`
--     gibi diğer sütunların güncellemesi takılmaz — v35 dersi).
--
-- Bilinçli gevşeklik: **var olmayan** hedef kimliği ne kapıya takılır ne
-- sayılır (0033/0034/0038/0039/0043 ile aynı sınıf: denetim izi). Bu tablo
-- `case_id` taşımaz; kural dosya kapsamı değil **emsal bütünlüğü**dür, bu
-- yüzden `scope_violations()` alanı **eklenmez** (GUARDED_TABLES 25 kalır).
-- Ret `RAISE(ABORT,'DIM_INTEGRITY:precedent_versions')` →
-- `StorageError::Integrity` → **`E_DB_INTEGRITY`**.

CREATE TRIGGER precedent_versions_conflict_guard_bi
BEFORE INSERT ON precedent_versions
WHEN NEW.identity_conflict NOT IN (0, 1)
  OR (NEW.identity_conflict = 1) <> (NEW.conflict_with_version_id IS NOT NULL)
  OR (NEW.conflict_with_version_id IS NOT NULL AND (
        NEW.conflict_with_version_id = NEW.id
        OR EXISTS (
             SELECT 1 FROM precedent_versions w
              WHERE w.id = NEW.conflict_with_version_id
                AND (w.precedent_id <> NEW.precedent_id
                     OR w.body_hash = NEW.body_hash))))
  OR EXISTS (
       SELECT 1 FROM precedent_versions x
        WHERE x.conflict_with_version_id = NEW.id
          AND (x.precedent_id <> NEW.precedent_id
               OR x.body_hash = NEW.body_hash))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedent_versions');
END;

CREATE TRIGGER precedent_versions_conflict_guard_bu
BEFORE UPDATE OF id, precedent_id, body_hash, identity_conflict, conflict_with_version_id
ON precedent_versions
WHEN NEW.id IS NOT OLD.id
  OR NEW.precedent_id IS NOT OLD.precedent_id
  OR NEW.body_hash IS NOT OLD.body_hash
  OR NEW.identity_conflict IS NOT OLD.identity_conflict
  OR NEW.conflict_with_version_id IS NOT OLD.conflict_with_version_id
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedent_versions');
END;
