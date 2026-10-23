# F10 — Claude, Codex, Antigravity ve Kimi CLI adaptörleri

- **Sürüm etiketi:** `v0.9.0` (hedef)
- **Durum:** Sicil ve argüman kapısı hazır; gerçek yürütme yok
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F10-T01 | ✅ | dört adaptör sicili (`crates/cli-adapters`) |
| F10-T02 | ✅ | izinli argüman kapısı; listede olmayan bayrak reddedilir |
| F10-T03 | ✅ | Kimi için toplu iş engeli (`E_CLI_BATCH_FORBIDDEN`) |
| F10-T04 | 🚧 | PATH keşfi var; `--version` yoklaması ve uyumluluk sicili yok |
| F10-T05 | ⬜ | gerçek abonelik hesabıyla uçtan uca doğrulama |

## Dürüst durum

Kurulu olmak güvenli kabul değildir. `discover()` yalnız dosya varlığını
bildirir ve sonucu **`Unverified`** olarak işaretler. Hiçbir CLI bu fazda
çalıştırılmaz, kimlik dosyası okunmaz, token kopyalanmaz.

## Doğrulama

`cargo test -p dim-cli-adapters` — 6 test. CI: **bekliyor**.
