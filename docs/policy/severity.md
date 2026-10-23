# Hata şiddeti ve veri bütünlüğü engelleri

| Sınıf | Tanım | Sürüm etkisi |
|---|---|---|
| S0 — Veri kaybı/bozulma | Kasa, blob veya DB'de geri dönüşsüz kayıp; yanlış anahtarla açma | **Bloke** |
| S0 — Kapsam sızıntısı | Bir dosyanın belgesi/notu başka dosyada görünür | **Bloke** |
| S0 — Yetkisiz aktarım | Onay olmadan dışarı veri gitmesi; CLI kimliği kopyalanması | **Bloke** |
| S0 — Yanıltıcı kaynak | Var olmayan alıntıyı "doğrulandı" göstermek | **Bloke** |
| S1 — Güvenlik zayıflığı | Şifreleme/izin hatası ama sömürü kanıtı yok | Sürüm notunda açık, düzeltme zorunlu |
| S1 — İşlev kaybı | Ana akış tamamlanamıyor | Düzeltme zorunlu |
| S2 — Kısmi işlev | Alternatif yol var, veri etkilenmiyor | Planlı düzeltme |
| S3 — Kozmetik | Metin/hizalama | Planlı |

## Bildirim sorumluluğu

- Güvenlik bildirim sorumlusu: depo `SECURITY.md` kanalı; ilk yanıt 72 saat.
- S0 bir hata açıkken yeni sürüm etiketi yayımlanmaz; CI kırmızıysa "yeşil" denmez.
- Hata kaydı; yeniden üretim adımı, etkilenen sürüm ve düzeltme commit'i taşır.
