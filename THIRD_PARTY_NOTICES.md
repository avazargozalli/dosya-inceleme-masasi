# Üçüncü taraf bileşenler ve lisanslar

Bu dosya F00-T07 kapsamında tutulan **lisans envanterinin** insan okunur özetidir.
Makine okunur SBOM sürüm hattında üretilir (`pnpm sbom`, `cargo cyclonedx`).

| Bileşen | Kullanım | Lisans | Yeniden dağıtım notu |
|---|---|---|---|
| Tauri 2 | masaüstü kabuk | MIT / Apache-2.0 | Uyumlu; bildirim korunur |
| React, Vite | arayüz | MIT | Uyumlu |
| rusqlite + SQLCipher | şifreli veri tabanı | MIT / BSD-style (SQLCipher) | SQLCipher açık kaynak build'i kullanılır; ticari build dağıtılmaz |
| PyMuPDF | PDF okuma/redaksiyon | AGPL-3.0 (ticari alternatif mevcut) | **AGPL uyumu:** uygulama AGPL-3.0-or-later; kaynak dağıtımı zorunlu |
| python-docx | DOCX okuma | MIT | Uyumlu |
| Tesseract + tessdata (tur, eng) | OCR | Apache-2.0 / tessdata Apache-2.0 | Dil dosyaları paket manifestinde listelenir |
| llama.cpp | yerel model çalıştırıcı | MIT | Model lisansı ayrı katalogda |
| Argon2 (RustCrypto) | parola türetme | MIT / Apache-2.0 | Uyumlu |
| aes-gcm (RustCrypto) | blob şifreleme | MIT / Apache-2.0 | Uyumlu |

## Kural

Yeni bağımlılık ekleyen PR, bu tabloya satır eklemek ve `docs/legal/license-registry.md`
içindeki kararı güncellemek zorundadır. AGPL ile uyumsuz lisanslı bileşen (ör. yalnız
ticari, AGPL-dışı copyleft) **kabul edilmez**; istisna ancak yazılı hukuk görüşü ve
sürüm notu ile alınır.
