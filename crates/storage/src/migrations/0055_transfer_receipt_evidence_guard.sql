-- 0055: **aktarım makbuzunun kanıtı** (transfer_receipts) doğuşta tutarlı
-- olmalı ve sonuçlandıktan sonra değişmemeli.
--
-- Neden: `transfer_receipts` satırı **dışarıya ne çıktığının makbuzudur** —
-- «nereye, kaç öğe, kaç bayt, hangi içerik özetiyle, ne zaman». Kasadaki
-- aktarım zincirinin son halkasıdır:
--   * `begin_transfer_receipt` makbuzu **deneme anında** açar (`attempted`);
--     gönderim düşse de iz kalır, çünkü içerik çıkmış olabilir (KVKK/denetim);
--   * `finish_transfer_receipt` tek koşullu UPDATE ile `attempted`ı
--     `dispatched`/`failed`a bağlar; açılış uzlaştırması (`reconcile_…`)
--     sonucu yazılmamış denemeleri `interrupted` yapar — satırın **kendi
--     kaydı** "gönderildi" ile "denendi" arasındaki farktır;
--   * ekran (`Ai.tsx`) son gönderimleri bu satırdan basar: hedef, öğe sayısı,
--     bayt sayısı, içerik özeti, durum, zaman. `list_transfer_receipts` dosya
--     kapsamını `ai_runs.case_id` üzerinden kurar.
-- Şemada bu tablonun **hiçbir kapısı yoktu**: `status` için CHECK bile yok,
-- `item_count`/`byte_count` negatif doğabilir, `dispatched_at` serbest metin,
-- `outbound_hash` biçimi denetlenmez ve satır sonradan **her** sütunuyla
-- değiştirilebilir; makbuz başka dosyanın çalıştırmasına taşınabilir.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v54; sonda
-- `scratch/dim-v55-probe/measure_v55.py`, log `measure_before.log`):
-- 16 doğum kusurunun **14'ü KABUL** (2'si zaten RED — yalnız FK), 12 UPDATE
-- saldırısının **12'si KABUL**, 7 meşru yazma KABUL:
--   (b01) doğuştan `dispatched` — hiç denenmemiş gönderim "gönderildi" der;
--   (b02) bilinmeyen durum (`sent`) — ekran etiketi çözemez,
--   (b03/b04) boş / yalnız boşluk hedef etiketi — "nereye" sorusu cevapsız,
--   (b05/b06/b07) 63 haneli / büyük harf / hex olmayan özet,
--   (b08/b09) `item_count` 0 / negatif, (b10/b11) `byte_count` 0 / negatif,
--   (b12/b13) damga `dün` / boş, (b14) boş makbuz kimliği,
--   (u01) hedef etiketi çevrildi (makbuz başka adrese işaret eder),
--   (u02) içerik özeti çevrildi (hangi baytların çıktığı yalanı),
--   (u03/u04) öğe/bayt sayısı değiştirildi (açıklanan hacim yalanı),
--   (u05) damga geriye yazıldı (gönderim zamanı kaydırıldı),
--   (u06) makbuz **başka dosyanın** çalıştırmasına taşındı (kapsam kırıldı),
--   (u07) makbuz kimliği değiştirildi (sicil satırı takas edildi),
--   (u08) bilinmeyen durum yazıldı, (u10) `dispatched`→`attempted`
--          (sonuç geri alındı: "hâlâ uçuşta" görünür),
--   (u11) `failed`→`dispatched` (başarısızlık başarıya çevrildi),
--   (u12) `interrupted`→`dispatched` (kesinti gönderime çevrildi),
--   (u13) sonuçlanmış satırın hedefi çevrildi.
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT: `id` kırpılmış-dolu; `ai_run_id` **var olan** çalıştırmayı
--     gösterir (fail-closed; FK zaten bağlar, kapı FK kapalıyken de doğru hüküm
--     verir); `destination_label` boşluktan ibaret değil; `outbound_hash` **64
--     küçük harf hex**; `item_count ≥ 1` ve `byte_count ≥ 1` (çekirdek boş
--     kapsamı zaten reddeder — `ai_run_execute`; sıfır öğeli "gönderim" makbuz
--     değildir); doğuşta `status='attempted'` (makbuz **denemeden** önce
--     açılır; doğuştan "gönderildi" diyen satır denetlenemeyen aktarımdır);
--     `dispatched_at` ISO-8601 damgası.
--   * UPDATE: `id`, `ai_run_id`, `destination_label`, `outbound_hash`,
--     `item_count`, `byte_count`, `dispatched_at` **donar** — yeniden ölçüm
--     yeni satır ister. `status` yalnız `attempted` → `dispatched` | `failed` |
--     `interrupted` ilerler; sonuçlanmış satırın durumu **terminal**dir
--     (geri alma ve sonuç uydurma kapalı). Değişiklik yoksa takılmaz.
-- Ret `DIM_INTEGRITY:transfer_receipts` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam sınıfı ayrı yazılmadı: tablo `case_id` taşımaz, dosya kapsamı
-- `ai_runs` üzerindendir ve `ai_run_id` bu kapıda donar — makbuz başka
-- çalıştırmaya taşınamaz (u06). Bu yüzden `GUARDED_TABLES` **25 kalır**.
--
-- Dürüst sınırlar (belgede ayrıntılı): v55 öncesi bozuk satırlar silinmez,
-- donar (onarım yolu yeni gönderimdir); `outbound_hash`in gerçekten gönderilen
-- baytların SHA-256'sı olduğu **yeniden hesaplanmaz** (SQLite'ta SHA-256 yok,
-- çekirdek üretir); `item_count`/`byte_count`in segmentlerle tutarlılığı
-- doğrulanmaz (retry'lar farklı kapsam gönderebilir); damga gelecekte olabilir;
-- tetikleyicileri anahtarı olan saldırgan düşürebilir.

CREATE TRIGGER transfer_receipt_evidence_guard_bi
BEFORE INSERT ON transfer_receipts
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NOT EXISTS (SELECT 1 FROM ai_runs a WHERE a.id = NEW.ai_run_id)
  OR NEW.destination_label IS NULL
  OR length(trim(NEW.destination_label)) = 0
  OR NEW.outbound_hash IS NULL
  OR length(NEW.outbound_hash) <> 64
  OR NEW.outbound_hash GLOB '*[^0-9a-f]*'
  OR NEW.item_count < 1
  OR NEW.byte_count < 1
  OR NEW.status IS NOT 'attempted'
  OR NEW.dispatched_at NOT LIKE '____-__-__T__:__:__%'
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_receipts');
END;

CREATE TRIGGER transfer_receipt_evidence_guard_bu
BEFORE UPDATE ON transfer_receipts
WHEN NEW.id IS NOT OLD.id
  OR NEW.ai_run_id IS NOT OLD.ai_run_id
  OR NEW.destination_label IS NOT OLD.destination_label
  OR NEW.outbound_hash IS NOT OLD.outbound_hash
  OR NEW.item_count IS NOT OLD.item_count
  OR NEW.byte_count IS NOT OLD.byte_count
  OR NEW.dispatched_at IS NOT OLD.dispatched_at
  OR (NEW.status IS NOT OLD.status
      AND NOT (OLD.status = 'attempted'
               AND NEW.status IN ('dispatched', 'failed', 'interrupted')))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:transfer_receipts');
END;
