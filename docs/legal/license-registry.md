# F00-T07 — Lisans ve yeniden dağıtım sicili

Her satır bir **karar** taşır: kullanılabilir / koşullu / kullanılamaz. Karar gerekçesiz
değiştirilemez; değişiklik ADR veya sürüm notu gerektirir.

| Bileşen | Sürüm politikası | Lisans | Karar | Koşul |
|---|---|---|---|---|
| Tauri 2 | minor sabit, patch güncel | MIT / Apache-2.0 | Kullanılabilir | Bildirim dosyası korunur |
| RustCrypto (argon2, aes-gcm, hkdf, sha2) | minor sabit | MIT / Apache-2.0 | Kullanılabilir | — |
| rusqlite + SQLCipher | 0.32 / 4.x | MIT + BSD-style | Kullanılabilir | Açık kaynak SQLCipher build'i; ticari binary dağıtılmaz |
| PyMuPDF | minor sabit | AGPL-3.0 | Koşullu | Uygulama AGPL-3.0-or-later; kaynak dağıtımı sağlanır |
| python-docx | minor sabit | MIT | Kullanılabilir | — |
| Tesseract + tessdata (tur, eng) | 5.x | Apache-2.0 | Kullanılabilir | Dil dosyaları manifestte listelenir, NOTICE'a eklenir |
| llama.cpp | commit sabit (hash) | MIT | Kullanılabilir | Model lisansı ayrı katalogda |
| Yerel modeller | model başına kayıt | Model lisansına bağlı | Model bazında | Lisans/ yeniden dağıtım izni olmayan model paketlenmez |
| Claude Code CLI | kullanıcı kurulumu | Ticari abonelik | Kullanılabilir (araç olarak) | Paketlenmez; kullanıcı kendi hesabıyla çalıştırır |
| Codex CLI | kullanıcı kurulumu | Ticari abonelik | Kullanılabilir (araç olarak) | Paketlenmez |
| Antigravity CLI | kullanıcı kurulumu | Sağlayıcı koşulları | Koşullu | F10 uyumluluk sicili doğrulanmadan varsayılan açılmaz |
| Kimi CLI | kullanıcı kurulumu | Sağlayıcı koşulları | Koşullu | Yalnız etkileşimli kullanım; otomatik batch engellenir |

## AGPL uyum notu

PyMuPDF AGPL-3.0'dır. Bu proje AGPL-3.0-or-later olduğu için uyumludur; ancak:

1. Dağıtılan her ikili ile birlikte **tam kaynak** ve derleme talimatı yayımlanır.
2. Ağ üzerinden hizmet sunulursa (v1'de yok) AGPL §13 kaynak sunma yükümlülüğü uygulanır.
3. Kapalı kaynak ticari kullanım isteyen taraf, PyMuPDF ticari lisansı ve bu proje için
   ayrı lisans görüşmesi yapmak zorundadır.
