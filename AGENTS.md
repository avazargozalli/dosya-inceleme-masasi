# Kodlama ajanları için bağlayıcı talimat

1. Önce `docs/spec/DOSYA_INCELEME_MASASI_TAM_PROJE_PLANI.md`, `docs/roadmap.md`, ilgili
   ADR ve fazın bağımlılıklarını oku.
2. Yalnız atanmış Fxx fazını uygula. Tamamlanmamış bağımlılığı tamamlandı varsayma.
3. Çalışan dikey dilim üret: UI'da görünen butonun arkasında gerçek çekirdek komutu olmalı.
   Sahte local state ile "çalışıyor" görüntüsü üretme.
4. Veri, kaynak konumu, şifreleme ve sağlayıcı izinlerini UI kontrolüne bırakma.
5. CLI kimlik dosyası okuma/kopyalama; global kullanıcı ayarı değiştirme.
6. Gerçek müvekkil verisi ekleme; `fixtures/synthetic/` kullan.
7. Test geçsin diye güvenlik kapısını, kaynak doğrulamasını veya kapsam denetimini kaldırma.
8. Her teslimde değişiklik, gerekçe, doğrulama kanıtı ve açık sınırlamaları yaz.
9. Gerçek hesap/platform testi yapılmadıysa yapıldı deme.
10. Commit: `Fxx-Tyy: kısa açıklama`. Bir commit tek işi hedefler.
