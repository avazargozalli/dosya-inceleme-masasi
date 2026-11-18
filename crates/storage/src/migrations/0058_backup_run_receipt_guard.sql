-- 0058: **yedek makbuzu** (backup_runs) doğuşta tutarlı olmalı ve sonuçlandıktan
-- sonra değişmemeli.
--
-- Neden: `backup_runs` satırı **«geri dönülebilir bir yedek var» iddiasıdır** —
-- hangi snapshot, hangi şema sürümünden, hangi manifest özetiyle, doğrulandı mı,
-- ne zaman. Kasadaki üç okuma yolu bu satıra yaslanır:
--   * `loadBackupFreshness` → `latestBackupSuccess`: `verified`/`succeeded`
--     satırların `completed_at ?? created_at`i **«son başarılı yedek»**tir;
--     `Settings.tsx` bunu «son yedek: …» diye basar ve `nextBackupAt` ile
--     **sonraki otomatik yedeğin tarihini** öteler — damga ileri yazılabilirse
--     otomatik yedek sessizce gecikir (veri kaybı yönünde yalan);
--   * yedek geçmişi rozetleri (`backupRunStatusLabel`/`backupRunStatusTone`):
--     «doğrulandı» ile «başarılı»/«başarısız»/«sürüyor» ayrımı bu sütundan
--     basılır; kanıtsız `verified` yazılabilirse ekran doğrulanmamış yedeğe
--     «doğrulandı» der;
--   * `backup_run_list` IPC → aynı satırlar.
-- Çekirdek kaydı `vault_backup` **yedeği aldıktan ve doğruladıktan sonra**
-- yazar (`record_backup_run`: `integrity_check` kanıtıyla `verified`) — yani
-- satır bir **makbuzdur**, niyet değil. Şemada bu tablonun kapısı ise yalnız
-- `status` CHECK'iydi: manifest özeti biçimsiz, şema sürümü negatif, damga
-- serbest metin, `verified` kanıtsız doğabilir; satır sonradan **her**
-- sütunuyla değiştirilebilir (kanıt silinebilir, tarih ileri alınabilir,
-- `failed`→`verified` çevrilebilir).
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v57; sonda
-- `scratch/dim-v58-probe/measure_v58.py`, log `measure_before.log`):
-- 42 yazımın **40'ı KABUL** (2'si zaten RED — yalnız `status` CHECK'i),
-- 7 meşru yazma KABUL:
--   (b01) boş makbuz kimliği,
--   (b02/b03) snapshot boşluk / kırpılmamış,
--   (b04) snapshot **yol ayırıcı** taşır (`../kacis`) — klasör adı değil,
--   (b05/b06) şema sürümü 0 / negatif — «hangi şemadan» sorusu anlamsız,
--   (b07/b08/b09/b10) manifest 63 hex / **büyük harf** / hex değil / `'hash'`,
--   (b11/b12) hedef boşluk / kırpılmamış,
--   (b13/b14) `created_at`/`completed_at` `'dün'`,
--   (b15) `running` + tamamlanma damgası (kesişen iddia),
--   (b16) `running` + doğrulama kanıtı (bitmemiş yedeğe kanıt),
--   (b17) `verified` + tamamlanma anı yok,
--   (b18/b19) `verified` + **kanıtsız** / kanıt yalnız boşluk —
--            ekranın «doğrulandı» iddiasının dayanağı yok,
--   (b20) bilinmeyen durum (yalnız CHECK yakalıyor),
--   (u01) manifest özeti çevrildi (başka yedeğin künyesi),
--   (u02) snapshot çevrildi (hangi klasör belirsizleşti),
--   (u03) şema sürümü çevrildi (geri dönüş uyumluluğu yalanı),
--   (u04) hedef çevrildi («nereye» yalanı),
--   (u05) `created_at` **2030**'a alındı (tazelik ve otomatik yedek tarihi),
--   (u06) makbuz kimliği takas edildi,
--   (u07) `completed_at` geçmişe çekildi,
--   (u08) `verified` satırın **kanıtı silindi**,
--   (u09) `succeeded`→`verified` **kanıtsız** (doğrulama uydurma),
--   (u10) `failed`→`verified` (başarısızlık başarıya çevrildi),
--   (u11) `running`→`succeeded` tamamlanma anı olmadan,
--   (u12) `running`→`succeeded` `completed_at='dün'` ile,
--   (u13) bilinmeyen durum (CHECK düşürülünce serbest),
--   (u14) `failed` satırın gerekçesi değiştirildi.
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — `id` kırpılmış-dolu; `snapshot_id` **kırpılmış** (kendi trim'i),
--     dolu, `/` `\` ve CR/LF/TAB taşımaz (klasör adıdır, yol değil);
--     `schema_version ≥ 1` (yedeği üreten kasa en az bir göç uygulamıştır;
--     `0` «hiç göç yok» demektir ve yedek üretemez); `manifest_hash` **64 küçük
--     harf hex** (manifest.json baytlarının sha256'sı — çekirdek `hex` ile
--     üretir); `status` kapalı küme; `destination_label` kırpılmış-dolu;
--     `created_at` ISO-8601 (`YYYY-MM-DDTHH:MM:SS…`); `completed_at` NULL ya da
--     ISO-8601; `verification_result` NULL ya da kırpılmış-dolu; `running` ise
--     **ne** tamamlanma anı **ne** kanıt taşır (bitmemiş yedek doğrulanmadı);
--     terminal durumlar (`succeeded`/`failed`/`verified`) tamamlanma anı
--     taşır; `verified` **kanıtlı** doğar.
--   * UPDATE — `id`, `snapshot_id`, `schema_version`, `manifest_hash`,
--     `destination_label`, `created_at` **donar** (yeniden ölçüm yeni satır
--     ister). `status` yalnız `running → succeeded|failed|verified` ilerler;
--     `succeeded → verified` **yalnız o anda kanıt yazılıyorsa** meşrudur
--     (sonradan doğrulama gerçek bir ürün yoludur, kanıtsız «doğrulandı»
--     değildir). Sonuçlanmış satırın durumu başka yöne **çevrilemez**
--     (`failed`→`verified` kapalı). Tamamlanma anı yalnız `running`den çıkarken
--     ve **ISO** olarak yazılır; sonrasında donar. Kanıt (`verification_result`)
--     `failed`/`verified` satırda donar ve her durumda kırpılmış-doludur.
--     Değişmeyen yazma takılmaz — v58 öncesi tutarsız satır **no-op** ile
--     kilitlenmez (yalnız kanıt/durum değişikliği kapıya takılır).
-- Ret `DIM_INTEGRITY:backup_runs` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam sınıfı ayrı yazılmadı: tablo `case_id` taşımaz (kasa düzeyi geçmiş),
-- dosya kapsamı sınıfı yoktur. `GUARDED_TABLES` **25 kalır**.
--
-- Dürüst sınırlar (belgede ayrıntılı): v58 öncesi bozuk satırlar silinmez ve
-- düzeltilmez (donar) — onarım yolu yeni yedek almaktır; `manifest_hash`in
-- gerçekten o klasördeki `manifest.json`un SHA-256'sı olduğu **yeniden
-- hesaplanmaz** (SQLite'ta SHA-256 yok; çekirdek üretir); snapshot klasörünün
-- diskte **var olduğu** doğrulanmaz; damga **gelecekte** olabilir (yalnız
-- biçim ve dondurma); `verification_result`in içeriği denetlenmez (yalnız
-- varlığı ve dondurulması); tetikleyicileri anahtarı olan saldırgan
-- düşürebilir (çekirdek doğrulaması ayrıca durur).

CREATE TRIGGER backup_run_receipt_guard_bi
BEFORE INSERT ON backup_runs
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.snapshot_id IS NULL
  OR NEW.snapshot_id IS NOT trim(NEW.snapshot_id)
  OR length(NEW.snapshot_id) = 0
  OR instr(NEW.snapshot_id, '/') > 0
  OR instr(NEW.snapshot_id, '\') > 0
  OR instr(NEW.snapshot_id, char(9)) > 0
  OR instr(NEW.snapshot_id, char(10)) > 0
  OR instr(NEW.snapshot_id, char(13)) > 0
  OR NEW.schema_version < 1
  OR NEW.manifest_hash IS NULL
  OR length(NEW.manifest_hash) <> 64
  OR NEW.manifest_hash GLOB '*[^0-9a-f]*'
  OR COALESCE(NEW.status, '') NOT IN ('running', 'succeeded', 'failed', 'verified')
  OR NEW.destination_label IS NULL
  OR NEW.destination_label IS NOT trim(NEW.destination_label)
  OR length(NEW.destination_label) = 0
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR (NEW.completed_at IS NOT NULL
      AND NEW.completed_at NOT LIKE '____-__-__T__:__:__%')
  OR (NEW.verification_result IS NOT NULL
      AND (NEW.verification_result IS NOT trim(NEW.verification_result)
           OR length(NEW.verification_result) = 0))
  OR (NEW.status = 'running'
      AND (NEW.completed_at IS NOT NULL OR NEW.verification_result IS NOT NULL))
  OR (NEW.status IN ('succeeded', 'failed', 'verified') AND NEW.completed_at IS NULL)
  OR (NEW.status = 'verified' AND NEW.verification_result IS NULL)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:backup_runs');
END;

CREATE TRIGGER backup_run_receipt_guard_bu
BEFORE UPDATE ON backup_runs
WHEN NEW.id IS NOT OLD.id
  OR NEW.snapshot_id IS NOT OLD.snapshot_id
  OR NEW.schema_version IS NOT OLD.schema_version
  OR NEW.manifest_hash IS NOT OLD.manifest_hash
  OR NEW.destination_label IS NOT OLD.destination_label
  OR NEW.created_at IS NOT OLD.created_at
  OR COALESCE(NEW.status, '') NOT IN ('running', 'succeeded', 'failed', 'verified')
  OR (NEW.status IS NOT OLD.status
      AND NOT (OLD.status = 'running'
               AND NEW.status IN ('succeeded', 'failed', 'verified'))
      AND NOT (OLD.status = 'succeeded'
               AND NEW.status = 'verified'
               AND length(trim(COALESCE(NEW.verification_result, ''))) > 0))
  OR (NEW.completed_at IS NOT OLD.completed_at
      AND (OLD.status IS NOT 'running'
           OR NEW.completed_at IS NULL
           OR NEW.completed_at NOT LIKE '____-__-__T__:__:__%'))
  OR (NEW.status IS NOT OLD.status
      AND NEW.status IN ('succeeded', 'failed', 'verified')
      AND NEW.completed_at IS NULL)
  OR (NEW.status IS NOT OLD.status
      AND NEW.status = 'verified'
      AND length(trim(COALESCE(NEW.verification_result, ''))) = 0)
  OR (NEW.verification_result IS NOT OLD.verification_result
      AND (OLD.status IN ('failed', 'verified')
           OR (NEW.verification_result IS NOT NULL
               AND (NEW.verification_result IS NOT trim(NEW.verification_result)
                    OR length(NEW.verification_result) = 0))))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:backup_runs');
END;
