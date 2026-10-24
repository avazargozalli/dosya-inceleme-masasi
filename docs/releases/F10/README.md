# F10 — Claude, Codex, Antigravity ve Kimi CLI adaptörleri

- **Sürüm etiketi:** `v0.9.0` (hedef)
- **Durum:** Devam ediyor — sicil, argüman kapısı ve **izinli yürütme çekirdeği**
  hazır; arayüz düğmesi ve sağlayıcıya özel akışlar yok
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F10-T01 | 🚧 | PATH keşfi + `--version` yoklaması; **mimari kontrolü yok** |
| F10-T02 | ✅ | adaptör kartı: kurulu / kurulu değil / doğrulanamadı |
| F10-T03 | ⬜ | resmî giriş/çıkış akışı yok; kimlik dosyası **okunmuyor** |
| F10-T04 | ✅ | `sanitize_env`: allowlist + gizli ad reddi; `env_clear()` |
| F10-T05 | 🚧 | mutlak yol, iş dizini, stdin sınırı var; **dosya izin kapsamı yok** |
| F10-T06 | ⬜ | Claude abonelik profili (araçsız/hook kapalı/şemalı çıktı) yok |
| F10-T07 | ⬜ | Codex JSON olay taşıması yok (yalnız `--json` argümanı izinli) |
| F10-T08 | ⬜ | Antigravity taşıması yok |
| F10-T09 | ✅ | Kimi `InteractiveOnly` + `batch_forbidden` → `E_CLI_BATCH_FORBIDDEN` |
| F10-T10 | ⬜ | Kimi API profili yok |
| F10-T11 | 🚧 | `Unverified` durumu var; yetenek matrisi yok |
| F10-T12 | 🚧 | izinsiz bayrak reddi testi var; hook/MCP/canary saldırı testleri yok |
| F10-T13 | ⬜ | transcript/telemetri temizliği yok |
| F10-T14 | ✅ | süre sınırı, iptal, alt süreç öldürme, kısmi çıktı |

## Yürütme çekirdeği (`crates/cli-adapters/src/exec.rs`)

`run_cli(&CliRunRequest, &Arc<AtomicBool>)`:

- **Argüman kapısı**: `build_args` — listede olmayan bayrak `E_CLI_ARGUMENT`,
  süreç **başlatılmadan** reddedilir. Shell string birleştirme yok.
- **Yol**: yürütülebilir **mutlak** olmalı (`E_CLI_EXEC_PATH`) ve çalıştırılabilir
  olmalı (`E_CLI_EXEC_MISSING`); iş dizini var olan dizin olmalı
  (`E_CLI_EXEC_WORKDIR`).
- **Ortam**: `Command::env_clear()` + allowlist. `TOKEN`, `SECRET`, `API_KEY`,
  `PASSWORD`, `COOKIE`, `CREDENTIAL`, `SESSION`, `PRIVATE_KEY` içeren adlar
  allowlist'e yazılsa bile `E_CLI_EXEC_ENV` ile reddedilir. Varsayılan liste:
  `PATH`, `HOME`, `LANG`, `LC_ALL`, `TMPDIR`, `TZ`.
- **Girdi**: `stdin_text` ayrı iş parçacığında yazılır (boru tıkanması olmaz).
- **Süre/iptal**: 1–600 sn aralığına kırpılır; süre dolunca veya `AtomicBool`
  iptal bayrağı işaretlenince süreç **öldürülür**; durum `TimedOut` /
  `Cancelled`, kısmi çıktı korunur.
- **Çıktı**: stdout/stderr baş kısmı en çok **64 KiB**; kesildiği
  `stdoutTruncated`/`stderrTruncated` ile işaretlenir. Okuyucu boru kapanmazsa
  en çok 2 sn beklenir ve çıktı **tam sayılmaz**.
- **Başlatma**: yeni yazılmış dosyayı hemen çalıştırmak Linux'ta `ETXTBSY`
  verebildiği için spawn 6 kez (50 ms arayla) denenir; son hata metni
  `E_CLI_EXEC_SPAWN` ile raporlanır.

## Dürüst durum

- Bu çekirdek **arayüze bağlı değil**: uygulamada “çalıştır” düğmesi yok, bu
  yüzden uçtan uca dikey dilim sayılmaz. F10 açık kalır.
- Gerçek abonelik hesaplarıyla hiçbir CLI denenmedi (insan/hesap bekliyor).
- Dosya izin kapsamı (T05) ve araç/MCP kısıtları (T06–T08) uygulanmadı; çocuk
  süreç yalnız argüman ve ortam düzeyinde sınırlanır.
- Windows'ta `exec` testleri çalışmaz (`#[cfg(unix)]`); yürütme kodunun kendisi
  platformdan bağımsızdır ama Windows'ta doğrulanmadı.
- Kimlik dosyası okunmaz, token kopyalanmaz (INV-11) — bu değişmedi.

## Doğrulama

`cargo test -p dim-cli-adapters` → **17** (10 yeni yürütme testi; gerçek süreç,
stdin, zaman aşımı, iptal, ortam allowlist'i, gizli ad reddi, 64 KiB kesme).
`cargo clippy -p dim-cli-adapters --all-targets -- -D warnings` temiz.
