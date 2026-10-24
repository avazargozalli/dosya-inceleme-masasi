# F10 — Claude, Codex, Antigravity ve Kimi CLI adaptörleri

- **Sürüm etiketi:** `v0.9.0` (hedef)
- **Durum:** Devam ediyor — sicil, argüman kapısı, izinli yürütme çekirdeği,
  **iş dizini kum havuzu (T05)** ve **arayüzden tetiklenen gerçek sınama** hazır;
  sağlayıcıya özel akışlar (T06–T08, T10) yok
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F10-T01 | 🚧 | PATH keşfi + `--version` yoklaması; **mimari kontrolü yok** |
| F10-T02 | ✅ | adaptör kartı: kurulu / kurulu değil / doğrulanamadı |
| F10-T03 | ⬜ | resmî giriş/çıkış akışı yok; kimlik dosyası **okunmuyor** |
| F10-T04 | ✅ | `sanitize_env`: allowlist + gizli ad reddi; `env_clear()`; arayüzden geçen gerçek koşuda doğrulandı |
| F10-T05 | ✅ | `sandbox::check_workdir` (mutlak/bağsız/boş/ev dışı/`0700`) + `E_CLI_WORKDIR_PERM`; kum havuzu içinde `HOME`/`TMPDIR`; stdin ≤ 1 MiB (`E_CLI_STDIN`); çalıştırma sonrası dosya sayımı `RunScope`; arayüzde kapsam paneli; gerçek CLI koşusunda doğrulandı |
| F10-T06 | ⬜ | Claude abonelik profili (araçsız/hook kapalı/şemalı çıktı) yok |
| F10-T07 | ⬜ | Codex JSON olay taşıması yok (yalnız `--json` argümanı izinli) |
| F10-T08 | ⬜ | Antigravity taşıması yok |
| F10-T09 | ✅ | Kimi `InteractiveOnly` + `batch_forbidden` → `E_CLI_BATCH_FORBIDDEN` (arayüz düğmesi de kapalı) |
| F10-T10 | ⬜ | Kimi API profili yok |
| F10-T11 | 🚧 | `Unverified` durumu var; yetenek matrisi yok |
| F10-T12 | 🚧 | izinsiz bayrak reddi + **proje talimatı okunamayan boş iş dizini**; hook/MCP/canary saldırı testleri yok |
| F10-T13 | ⬜ | transcript/telemetri temizliği yok |
| F10-T14 | ✅ | süre sınırı, iptal, alt süreç öldürme, kısmi çıktı — **arayüz düğmesinden uçtan uca** |

## Arayüz bağlantısı — “Bağlantıyı sına” (2026-10-24)

Uygulamada düğmenin arkasında gerçek çekirdek var; sahte yerel durum yok.

- **Komutlar:** `provider_probe_run` (async; yürütme `spawn_blocking` içinde, arayüz
  kilitlenmez) ve `provider_probe_cancel`.
- **Yol:** `find_on_path` → `dim_cli_adapters::run_probe` (`crates/cli-adapters/src/probe.rs`)
  → `run_cli`. Arayüz ile testler **aynı** fonksiyonu çalıştırır.
- **Serbest argüman yok:** arayüz yalnız adaptör kimliği gönderir; argv sicildeki
  `version_args[0]` (`--version`) olarak kurulur. UI'dan bayrak geçirilemez.
- **Boş geçici iş dizini:** `dim-cli-probe-<pid>-<n>` sistem geçici dizininde
  açılır, **boş** olduğu test edilir, yürütmeden sonra silinir. Böylece depodaki
  `AGENTS.md`/`CLAUDE.md` gibi proje talimatları ve kullanıcı dosyaları alt sürece
  görünmez (T12'nin proje-talimatı vektörü yapısal olarak kapalı).
- **Ortam:** `env_clear()` + allowlist (bu makinede `PATH, HOME, TMPDIR`).
  Anahtar/token/cookie adları reddedilir.
- **Tek koşu:** aynı anda tek sınama; ikincisi `E_CLI_BUSY`. İptal bayrağı alt
  süreci öldürür, kısmi çıktı raporlanır.
- **Denetim:** kasa açıksa `cli_probe`/`run` kaydı yazılır (adaptör, argv, ortam
  anahtarları, durum, süre). **Çıktı metni denetime yazılmaz.**
- **Arayüz:** kart başına “Bağlantıyı sına” + çalışırken “İptal”; sonuçta durum
  rozeti, süre, çıkış kodu, gerçek argv, ortam anahtarları, iş dizini, stdout/stderr
  ve 64 KiB kesme uyarısı. Kimi'de düğme kapalı (etkileşimli-only).

### Bulunan ve düzeltilen kusur

Argüman kapısı (`build_args`) yalnız `allowed_args` listesine bakıyordu; sicilin
kendi `version_args`'ı (`--version`, `--help`) listede olmadığı için **düğme
`E_CLI_ARGUMENT` verecekti**. Kapı artık `version_args`'ı da izinli sayıyor
(`version_flags_pass_the_argument_gate` testi). Bilinmeyen bayrak hâlâ reddedilir.

## İş dizini ve dosya izin kapsamı (F10-T05, 2026-10-24)

`crates/cli-adapters/src/sandbox.rs` — politika düzeyinde kapsam:

- **`RunSandbox`**: sistem geçici dizininde `dim-cli-sandbox-<pid>-<n>`, oluşturulur
  oluşturulmaz `0700`'e çekilir, kanonik yol raporlanır; bırakılınca
  `remove_dir_all` ile **silinir**. Alt süreç için `home/` ve `tmp/` alt dizinleri
  de `0700` açılır.
- **`check_workdir`**: mutlak yol, sembolik bağ değil, dizin, **boş**, ev dizini
  ya da ev dizinin atası değil, Unix'te izin `& 0o077 == 0`. Her ihlal ayrı kod:
  `E_CLI_EXEC_WORKDIR`, `E_CLI_WORKDIR_NOT_EMPTY`, `E_CLI_WORKDIR_SCOPE`,
  `E_CLI_WORKDIR_PERM`. Boşluk kuralı proje talimatı/hook dosyası taşıyan dizini
  de kapatır: alt süreç `AGENTS.md`/`CLAUDE.md`/.mcp okuyamaz.
- **stdin**: `MAX_STDIN_BYTES = 1 MiB`; aşım süreç **başlamadan** `E_CLI_STDIN`.
  Yazım 8 KiB parçalarla, parçalar arasında iptal bayrağına bakılarak yapılır.
- **`HOME`/`TMPDIR`**: `HomeIsolation::Sandbox` (varsayılan) kum havuzuna
  yönlendirir; `UserHome` açık seçimdir, `RunScope.home_isolation` ile raporlanır
  ve arayüzde uyarı üretir.
- **Sayım (`RunScope`)**: çalıştırma sonrası dosyalar göreli ad + bayt olarak
  sayılır (içerik okunmaz), 200 girdide kesilir, toplam 64 MiB'ı aşarsa
  `size_limit_exceeded`. Arayüzde "İzin · HOME/TMPDIR · stdin · Bırakılan dosya"
  satırları + dosya listesi; denetime yalnız sayılar ve politikalar yazılır
  (**dosya adı ve içerik yazılmaz**).

## Yürütme çekirdeği (`crates/cli-adapters/src/exec.rs`)

`run_cli(&CliRunRequest, &Arc<AtomicBool>)`:

- **Argüman kapısı**: `build_args` — `allowed_args` + `version_args` dışındaki
  bayrak `E_CLI_ARGUMENT`, süreç **başlatılmadan** reddedilir. Shell string
  birleştirme yok.
- **Yol**: yürütülebilir **mutlak** olmalı (`E_CLI_EXEC_PATH`) ve çalıştırılabilir
  olmalı (`E_CLI_EXEC_MISSING`).
- **İş dizini kapsamı** (`sandbox::check_workdir`, F10-T05): mutlak, sembolik bağ
  değil (`E_CLI_EXEC_WORKDIR`), **boş** (`E_CLI_WORKDIR_NOT_EMPTY` — proje
  talimatı/hook dosyası taşıyan dizin çalıştırılmaz), kullanıcı ev dizini ya da
  onun üstü değil (`E_CLI_WORKDIR_SCOPE`), Unix'te `0700` (`E_CLI_WORKDIR_PERM`;
  grup/diğer erişimi varsa reddedilir).
- **Ortam**: `Command::env_clear()` + allowlist. `TOKEN`, `SECRET`, `API_KEY`,
  `PASSWORD`, `COOKIE`, `CREDENTIAL`, `SESSION`, `PRIVATE_KEY` içeren adlar
  allowlist'e yazılsa bile `E_CLI_EXEC_ENV` ile reddedilir. Varsayılan liste:
  `PATH`, `HOME`, `LANG`, `LC_ALL`, `TMPDIR`, `TZ`.
- **HOME/TMPDIR politikası**: `HomeIsolation::Sandbox` (varsayılan) ise alt süreç
  için kum havuzu içinde `home/` ve `tmp/` dizinleri açılır (`0700`) ve `HOME` /
  `TMPDIR` oraya yönlendirilir; `UserHome` **açık seçimdir** ve raporda öyle
  görünür.
- **Girdi**: `stdin_text` en çok **1 MiB**; aşan istek süreç başlamadan
  `E_CLI_STDIN` ile reddedilir. Yazım ayrı iş parçacığında, 8 KiB parçalarla ve
  iptal bayrağına bakarak yapılır (boru tıkanması olmaz).
- **Süre/iptal**: 1–600 sn aralığına kırpılır; süre dolunca veya `AtomicBool`
  iptal bayrağı işaretlenince süreç **öldürülür**; durum `TimedOut` /
  `Cancelled`, kısmi çıktı korunur.
- **Kapsam kanıtı**: çalıştırmadan sonra kum havuzunda kalan dosyalar **göreli ad
  + bayt** olarak sayılır (`RunScope`), içerik okunmaz; toplam 64 MiB'ı aşarsa
  `size_limit_exceeded` işaretlenir (sınır **sonradan** ölçülür, süreç
  durdurulmaz). Liste 200 girdide kesilir ve `inventory_truncated` yazılır.
- **Çıktı**: stdout/stderr baş kısmı en çok **64 KiB**; kesildiği
  `stdoutTruncated`/`stderrTruncated` ile işaretlenir. Okuyucu boru kapanmazsa
  en çok 2 sn beklenir ve çıktı **tam sayılmaz**.
- **Başlatma**: yeni yazılmış dosyayı hemen çalıştırmak Linux'ta `ETXTBSY`
  verebildiği için spawn 6 kez (50 ms arayla) denenir; son hata metni
  `E_CLI_EXEC_SPAWN` ile raporlanır.

## Gerçek CLI ile ölçüm (bu makine, macOS aarch64)

`cargo run -p dim-cli-adapters --example probe -- <adaptör>` — arayüz düğmesiyle
aynı yol:

| Adaptör | Yol | Durum | Süre | Çıktı |
|---|---|---|---|---|
| Claude Code | `~/.npm-global/bin/claude` | `Exited(0)` | 463 ms | `2.1.303 (Claude Code)` |
| Codex CLI | `~/.local/bin/codex` | `Exited(0)` | 197 ms | `codex-cli 0.159.2` |
| Kimi | `~/.local/bin/kimi` | `E_CLI_BATCH_FORBIDDEN` | — | (tasarım gereği kapalı) |
| Antigravity | — | kurulu değil | — | — |

Ortam üçünde de yalnız `PATH, HOME, TMPDIR`; iş dizini boş geçici dizin.

## Dürüst durum

- **İstem/model çağrısı yok.** T06–T08 profilleri (araçsız, hook/MCP kapalı,
  şemalı çıktı) uygulanmadığı için hassas kaynak aktarımı **açılmadı**; düğme
  yalnız sürüm yoklaması çalıştırır.
- **Dosya izin kapsamı (T05) bir politika kapsamıdır, çekirdek sanal alanı
  değildir.** Uygulama iş dizinini kendisi açar (`0700`), boş ve ev dizini dışında
  olmasını zorlar, `HOME`/`TMPDIR`'i kum havuzuna yönlendirir, stdin'i 1 MiB'te
  keser ve çalıştırma sonrası kum havuzunda kalan dosyaları sayıp siler. Buna
  karşılık: alt süreç **mutlak yolla** kullanıcının okuyabildiği dosyaları yine
  okuyabilir; `HOME` yönlendirmesi yalnız varsayılan konumları etkiler.
  `unsafe_code = "forbid"` olduğu için `setrlimit`/`umask` (fork sonrası kanca)
  uygulanamaz: dosya boyutu sınırı (64 MiB) **sonradan** ölçülür, süreç
  durdurulmaz. Bu yüzden arayüz serbest istem/argüman kabul etmiyor.
- Gerçek abonelik hesaplarıyla oturum açma/çıkma denenmedi (insan/hesap bekliyor).
  Antigravity bu makinede kurulu değil.
- Tauri penceresi açılmadı: düğme, iptal ve sonuç paneli tip denetimi + birim
  testleri + üretim derlemesiyle doğrulandı; gerçek tıklama/görsel QA yok.
- Windows'ta `exec`/`probe` testleri çalışmaz (`#[cfg(unix)]`); yürütme kodunun
  kendisi platformdan bağımsızdır ama Windows'ta doğrulanmadı.
- Denetim kaydı yalnız kasa açıkken yazılır; kilitliyken sınama çalışır (tanı
  amaçlı, veriye dokunmaz) ama denetime düşmez.

## Doğrulama

- `cargo test -p dim-cli-adapters` → **36** (F10-T05 turunda 13 yeni: kum havuzu
  boş/`0700`/silinir, çocuk dizinleri `0700`, geniş izin reddi, boş olmayan dizin
  reddi, sembolik bağ reddi, ev dizini reddi, stdin aşımı süreç başlamadan,
  sınırda stdin tam teslim, `HOME`/`TMPDIR` yönlendirme + dosya sayımı, `UserHome`
  açık seçimi, sayım kesme sınırı).
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` temiz;
  `clippy -p dim-cli-adapters --all-targets -D warnings` temiz; rustfmt iki
  manifest temiz.
- Web: **190** test (3 yeni: bayt biçimi, kapsam satırları, kapsam uyarıları),
  `tsc --noEmit` + `vite build` yeşil.
- Gerçek CLI koşusu (`cargo run -p dim-cli-adapters --example probe -- …`):
  Claude Code `2.1.303` 463 ms, Codex `codex-cli 0.159.2` 197 ms; ikisinde de iş
  dizini `0700`, `HOME`/`TMPDIR` kum havuzunda, kalan dosya 0. `HOME`
  yönlendirmesi gerçek araçların `--version` yolunu bozmadı.
