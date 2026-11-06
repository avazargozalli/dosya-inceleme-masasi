-- Dışa aktarma kaydı **nereye** yazıldığını da taşır. Denetim defteri tür +
-- hedef türü + zaman + özet taşıyordu ama seçilen gerçek yol yoktu: avukat
-- "hangi klasöre/dosyaya çıktı" sorusunu yanıtlayamıyordu. Eski kayıtlar için
-- değer NULL'dır (uydurulmaz).
ALTER TABLE exports ADD COLUMN destination_path TEXT;
