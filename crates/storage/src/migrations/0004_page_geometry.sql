-- 0004: sayfa koordinat uzayı. Kaynak konumu (bbox/quad) hangi uzayda
-- üretildiyse adı sayfayla birlikte saklanır; okuyucu ve redaksiyon aynı
-- uzayı kullanır (F06-T08). Eski kayıtlarda NULL kalır: koordinat yok demektir,
-- tahmin edilmez.
ALTER TABLE pages ADD COLUMN coordinate_space TEXT;
