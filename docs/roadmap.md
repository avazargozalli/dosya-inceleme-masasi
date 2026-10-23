# Yol haritası ve faz durumu

Şartname: `docs/spec/DOSYA_INCELEME_MASASI_TAM_PROJE_PLANI.md` (bağlayıcı).
Bir faz **ancak** kabul kanıtı `docs/releases/Fxx/` altına yazıldı ve CI yeşil olduğunda
`[x]` işaretlenir.

| Faz | Çıktı | Sürüm | Durum |
|---|---|---|---|
| F00 | Kapsam, lisans, pilot protokolü | v0.0.0-plan | ✅ |
| F01 | Teknik doğrulamalar ve ADR'ler | v0.0.1-spike | 🚧 |
| F02 | Repo, derleme, masaüstü iskeleti | v0.1.0 | 🚧 |
| F03 | Kasa, veritabanı, yedek temeli | v0.2.0 | ✅ |
| F04 | UX ve dosya çalışma alanları | v0.3.0 | ✅ |
| F05 | İçe aktarma ve belge sürümleri | v0.4.0 | ✅ |
| F06 | Metin, OCR, UDF, kaynak konumları | v0.5.0 | 🚧 |
| F07 | Arama ve belge okuyucu | v0.6.0 | ✅ |
| F08 | Kronoloji, iddia, inceleme, görev | v0.7.0 | ✅ |
| F09 | AI sağlayıcı çerçevesi | v0.8.0 | 🚧 |
| F10 | Dört CLI adaptörü | v0.9.0 | 🚧 |
| F11 | Kaynaklı AI inceleme araçları | v0.10.0 | 🚧 |
| F12 | Emsal ve atıf kontrolü | v0.11.0 | 🚧 |
| F13 | Gerçek karartma ve paylaşım | v0.12.0 | 🚧 |
| F14 | Çıktılar, paket, yedek | v0.13.0 | 🚧 |
| F15 | Kalite, güvenlik, pilot | v1.0.0-beta.1 | ⬜ |
| F16 | İmzalı genel sürüm | v1.0.0 | ✅ |
| F17 | Windows ARM64 | v1.1.0 | ⬜ |
| F18 | Genişleme modülleri | modül | ⬜ |

## Neden CI-first

Geliştirme makinesinde disk ve araç zinciri sınırlıdır; ağır Rust/Tauri derlemeleri ve
platform paketleri **GitHub Actions** üzerinde üretilir. Bu tercih aynı zamanda
tekrarlanabilirlik sağlar: "benim makinemde çalışıyordu" kanıt sayılmaz. Lokal ortamda
yalnız kaynak, tip kontrolü, lint ve hızlı birim testleri çalıştırılır.

## Faz kapısı

1. İş kimliği (Fxx-Tyy) ve REQ eşlemesi yazılır.
2. Kod + test aynı PR'da gelir.
3. CI: `fmt`, `clippy -D warnings`, `cargo test`, `tsc`, `vitest`, `pytest`, sözleşme drift.
4. `docs/releases/Fxx/README.md` kanıtı: commit, platform, test çıktısı, bilinen sınır.
5. İnceleyici onayı. Aksi halde faz açık kalır.
