-- 0029: maskeleme planının çıktı bağının **silinme izi** (F13-T04d).
--
-- 0028 `output_version_id`yi `ON DELETE SET NULL` yabancı anahtar yaptı:
-- çıktı sürümü silinince bağ NULL'a döner, denetim satırı yaşar. Ama o hâlde
-- "bağ hiç yazılmadı (bu sütundan önce yazılmış plan)" ile "çıktı vardı,
-- sonra silindi" **ayırt edilemiyordu**; arayüz ikisine de aynı cümleyi
-- ("Çıktı sürümü kayıtlı değil") yazıyordu ve sicil sustuğu yerde yanlış
-- izlenim bırakıyordu.
--
-- `output_version_ref` bağ yazılırken kaydedilen kimliği taşır ve **yabancı
-- anahtar değildir**: çıktı silinse de iz kalır, satır neyi kaybettiğini
-- söyleyebilir. Silinen çıktının **adı ve sürüm numarası yazılmaz** (o bilgi
-- kasadan silinmiştir; uydurulmaz) — yalnız kimliği kalır.
ALTER TABLE redaction_plans ADD COLUMN output_version_ref TEXT;

-- v28 kasalar: bağ hâlâ duruyorsa iz geriye dönük yazılır. 0029'dan **önce**
-- zaten silinmiş çıktıların izi kurtarılamaz (bağ NULL'a dönmüştü, geri
-- getirilemeyen bilgi tahmin edilmez) — o planlar "hiç bağlanmamış" gibi
-- okunur; sınır durum dosyasında yazılıdır.
UPDATE redaction_plans SET output_version_ref = output_version_id
 WHERE output_version_id IS NOT NULL;
