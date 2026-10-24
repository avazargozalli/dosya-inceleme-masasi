# F10 — Claude, Codex, Antigravity ve Kimi CLI adaptörleri

- **Sürüm etiketi:** `v0.9.0` (hedef)
- **Durum:** Devam ediyor — sicil, argüman kapısı, izinli yürütme çekirdeği,
  **mimari uyum denetimi (T01)**, **iş dizini kum havuzu (T05)**, **yetenek matrisi (T11)**,
  **başlangıç saldırı testleri (T12)**, **kalıntı ölçümü/temizliği (T13)** ve
  **arayüzden tetiklenen gerçek sınama** hazır; sağlayıcıya özel akışlar
  (T06–T08, T10) yok
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F10-T01 | ✅ | mutlak PATH keşfi + `--version` + **başlık baytlarından mimari** (Mach-O thin/fat, ELF, PE, `#!` betiği → shebang); `same`/`different`/`fat`/`unknown`; arayüzde “mimari” rozeti + “Mimarileri denetle”. Süreç başlatılmaz |
| F10-T02 | ✅ | adaptör kartı: kurulu / kurulu değil / doğrulanamadı |
| F10-T03 | ⬜ | resmî giriş/çıkış akışı yok; kimlik dosyası **okunmuyor** |
| F10-T04 | ✅ | `sanitize_env`: allowlist + gizli ad reddi; `env_clear()`; arayüzden geçen gerçek koşuda doğrulandı |
| F10-T05 | ✅ | `sandbox::check_workdir` (mutlak/bağsız/boş/ev dışı/`0700`) + `E_CLI_WORKDIR_PERM`; kum havuzu içinde `HOME`/`TMPDIR`; stdin ≤ 1 MiB (`E_CLI_STDIN`); çalıştırma sonrası dosya sayımı `RunScope`; arayüzde kapsam paneli; gerçek CLI koşusunda doğrulandı |
| F10-T06 | ⬜ | Claude abonelik profili (araçsız/hook kapalı/şemalı çıktı) yok |
| F10-T07 | ⬜ | Codex JSON olay taşıması yok (yalnız `--json` argümanı izinli) |
| F10-T08 | ⬜ | Antigravity taşıması yok |
| F10-T09 | ✅ | Kimi `InteractiveOnly` + `batch_forbidden` → `E_CLI_BATCH_FORBIDDEN` (arayüz düğmesi de kapalı) |
| F10-T10 | ⬜ | Kimi API profili yok |
| F10-T11 | ✅ | yetenek matrisi: adaptör × kurulu sürüm × `--help` kanıtı; gerekli yetenek `Missing` → **uyumsuz**, doğrulanamıyor → **sınırlı**, kapı kapanır; arayüzde “Yetenekleri sına” |
| F10-T12 | ✅ | başlangıç saldırıları: hook / MCP / proje talimatı / kapsam dışı canary vektörleri geçici fikstürde; **dedektör kanıtı** (kirli 4/4, temiz 0/4), kirli proje dizini iş dizini olarak **reddedilir**, kurulu CLI'larda sertleştirme bayrakları + `--help` ile gerçek koşu (işaret 0); arayüzde “Saldırıları sına” |
| F10-T13 | ✅ | kalıntı envanteri (sürüm bazlı bildirilen yollar) + önce/sonra ölçüm + **yalnız uygulamaya ait** dosya temizliği + çökme kum havuzu süpürme; arayüzde “Kalıntı denetimi” kartı |
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

## Mimari uyum denetimi (F10-T01, 2026-10-24)

`crates/cli-adapters/src/arch.rs` — yürütülebilirin **başlık baytları** okunur,
biçim ve mimari çıkarılır, uygulamanın süreç mimarisiyle karşılaştırılır.
**Süreç başlatılmaz**, dosya çalıştırılmaz, ağ kullanılmaz; en çok 4 KiB okunur
(PE başlığı daha ilerideyse yalnız o bölüm).

### Okunan biçimler

| Biçim | Nasıl | Mimari kaynağı |
|---|---|---|
| Mach-O thin | `CF FA ED FE` / `CE FA ED FE` (+ büyük-endian varyantları) | `cputype` (x86_64, arm64, …) |
| Mach-O fat | `CA FE BA BE` / `CA FE BA BF` | her dilimin `cputype`'ı; iki+ dilim → **`fat`** |
| ELF | `7F 45 4C 46` | `e_machine` (bayt sırası `e_ident[5]`'ten) |
| PE | `MZ` + `e_lfanew` → `PE\0\0` | `IMAGE_FILE_HEADER.Machine` |
| Betik | `#!` | **shebang yorumlayıcısı** (aşağıda) |

Bozuk/kesik başlıklar **mimari uydurmaz**: `reason` kodu döner
(`magic_unrecognized`, `fat_truncated`, `pe_signature_missing`,
`header_too_short`, `empty_file`, `pe_offset_too_large`, …).

### Betik → yorumlayıcı

npm/uv ile kurulan CLI'lar çoğu zaman sembolik bağ → `#!` betiğidir
(`#!/usr/bin/env node`, `#!/…/bin/python`). Betiğin kendisi mimari taşımaz;
`env` bayrakları (`-S`) ve değişken atamaları atlanır, komut PATH'te bulunur ve
**onun** başlığı okunur. Betik kendi biçimi olarak `betik (shebang)` kalır ama
karşılaştırma yorumlayıcıdan gelir (evrensel yorumlayıcı → `fat`).
Yorumlayıcı bulunamazsa `interpreter_unresolved` → **`unknown`**; "aynı" denmez.

### Kararlar

| Karar | Kural |
|---|---|
| `same` | tek mimari ve uygulamanın süreç mimarisiyle aynı |
| `different` | tek mimari, uygulamadan farklı |
| `fat` | Mach-O evrensel (≥2 dilim); **uygulama mimarisini içerip içermediği ayrıca** (`contains_app_arch`) yazılır |
| `unknown` | başlık okunamadı, mimari yok ya da yorumlayıcı bulunamadı |

### Gerçek ölçüm (bu makine, macOS aarch64)

`cargo run -p dim-cli-adapters --example arch`

| Adaptör | Bulunan yol | Biçim | Mimari | Karar |
|---|---|---|---|---|
| Claude Code 2.1.303 | `~/.npm-global/bin/claude` → `…/claude-code/bin/claude.exe` | Mach-O | aarch64 | `same` |
| Codex CLI 0.159.2 | `~/.local/bin/codex` → `…/0.159.2-aarch64-apple-darwin/bin/codex` | Mach-O | aarch64 | `same` |
| Kimi CLI 1.41.0 | `~/.local/bin/kimi` → `…/kimi-cli/bin/kimi` (betik) → `…/bin/python` | betik | x86_64 + aarch64 | `fat` (uygulama mimarisi var) |
| Antigravity | PATH'te yok | — | — | `unknown` |

Üç kurulu araç da **sembolik bağ** üzerinden çalışıyor; Kimi bir Python betiği
olduğu için mimarisi ancak yorumlayıcıdan okunabildi — bu, betik yolunun gerçek
bir kurulumda gerekli olduğunu gösteriyor.

### Arayüz

- **Adaptör kartı:** “kurulu” rozetinin yanında mimari rozeti
  (`mimari aynı` yeşil / `mimari farklı` kırmızı / `evrensel` sarı /
  `mimari bilinmiyor` nötr) + `Yol` ve `Mimari` satırları. Değer `provider_probe`
  içinde başlıktan okunur; ayrı tıklama gerekmez.
- **“Mimari uyum denetimi” kartı:** “Mimarileri denetle” düğmesi
  (`cli_arch_report`), uygulama mimarisi + sayımlar, adaptör başına yol (bağ
  çözümüyle), biçim, dilimler, yorumlayıcı, neden ve okunan bayt; uyarılar
  (farklı / evrensel-uyumsuz / doğrulanamayan) ve “Denetimin sınırları”.
- **Denetim:** `cli_arch`/`probe` — yalnız adaptör kimliği, kurulu, biçim,
  karar, neden ve uygulama mimarisi. **Yol ve shebang denetime yazılmaz.**

### Dürüst sınırlar (T01)

- Karşılaştırma **süreç** mimarisiyle yapılır (`std::env::consts::ARCH`);
  Rosetta altında çalışan x86_64 süreç kendini x86_64 bildirir. Donanım
  mimarisi ayrıca okunmaz.
- Mimari uyumu **güvenlik kanıtı değildir**: yalnız "bu ikili bu süreçle aynı
  mimaride" der. Sürüm, yetenek kapısı (T11) ve kapsam (T05) ayrı denetlenir.
- Üçüncü taraf yürütülebilirin **içeriği** doğrulanmaz (imza, hash, kötü niyet);
  yalnız başlık baytları okunur.
- Mach-O `cputype`/ELF `e_machine`/PE `Machine` alanlarına güvenilir; dosyanın
  gerçekten o mimaride yüklenebileceği denenmez (süreç başlatılmıyor).
- Windows'ta PE okuma kodu platformdan bağımsız ama bu makinede Windows ikilisi
  ile **gerçek dosyada denenmedi** (birim testleri sentetik başlıklarla).
- Betik zinciri en çok iki düzey çözülür; daha derin zincir `interpreter_chain`
  olarak "bilinmiyor" kalır.
- Tauri penceresi açılmadı → kartın görsel QA'sı yok.

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

## Başlangıç saldırıları — hook, MCP, proje talimatı, canary (F10-T12, 2026-10-24)

`crates/cli-adapters/src/attack.rs`. Saldırı yüzeyi, CLI bir dizinde
başladığında kendiliğinden okunan her şeydir: `CLAUDE.md`/`AGENTS.md`
(proje talimatı), `.claude/settings.json` + hook betikleri, `.mcp.json` /
`~/.codex/config.toml` (MCP sunucuları) ve modelin araç çağrısıyla
erişebildiği dosyalar (kapsam dışı canary).

### Üç ayrı ölçüm (karıştırılmaz)

1. **Kapı (gerçek).** Saldırı dosyalarıyla dolu bir dizin **iş dizini olarak
   reddedilir**: `check_workdir` → `E_CLI_WORKDIR_NOT_EMPTY`. Ürün CLI'ya proje
   dizini vermez; iş dizini her zaman boş `0700` kum havuzudur (F10-T05). Yani
   vektörler sürece hiç ulaşmaz. Ölçüm: fikstürün kirli proje dizini
   `GateCheck` ile reddedilir, temiz dizin kabul edilir; ayrıca `run_cli`
   aynı dizini süreç **başlamadan** reddeder.
2. **Dedektör kanıtı (fikstür).** Kirli fikstürde **naif yükleyici taklidi**
   (`naive_load`; gerçek CLI kodu değil) dört vektörün hepsinde işaret bırakır,
   temiz fikstürde hiçbirinde. “Sıfır işaret” sonucunun körlükten değil
   gerçekten temizlikten geldiğini yalnız bu adım gösterir.
   İşaretler: `project_instruction.mark`, `hook.mark`, `mcp.mark`,
   `canary.mark` — betikler `DIM_ATTACK_MARKERS` ile hedef dizini öğrenir.
3. **Gerçek CLI koşusu (bilgilendirici).** Kurulu araç, F10-T11 matrisinin
   bildirdiği **sertleştirme bayrakları** + `--help` ile boş kum havuzunda,
   `HOME` **sahte ev** dizinine yönlendirilmiş hâlde çalışır; işaretçi ve
   fikstür belirtecinin çıktıda yansıması aranır. İstem gönderilmediği için
   hook/MCP/proje talimatı yolu bu çağrıda **hiç açılmaz**: sonuç “bu çağrıda
   tetiklenmedi”dir, “korunuyor” değildir.

### Kararlar

| Karar | Anlamı |
|---|---|
| `triggered` | İşaretçi ya da yansıma bulundu — saldırı **başarılı** |
| `silent_guarded` | İşaret yok, dedektör kanıtlı, kapatma yolu **bildirildi** (davranış denenmedi) |
| `silent_unguarded` | İşaret yok, dedektör kanıtlı, kapatma yolu **bildirilmedi** → sessizlik koruma sayılmaz |
| `inconclusive` | Dedektör kanıtlanmadı (vekil işaret bırakmadı) → sonuç anlamsız |

`evaluate`/`outcomes` saf fonksiyonlardır: yetenek raporu + gözlem + dedektör
kanıtı → karar. Bu yüzden “sessiz” hiçbir zaman yeşil gösterilmez.

### Ölçüm (macOS aarch64, `--example attack`)

- **Fikstür:** dedektör kirli **4/4**, temiz **0/4** → kanıtlı; kirli proje
  dizini iş dizini olarak **reddedildi** (`E_CLI_WORKDIR_NOT_EMPTY`).
- **Ürün yapılandırması** (boş iş dizini + yalıtılmış `HOME`): vekil yükleyici
  proje talimatına, hook'a ve MCP'ye **ulaşamadı**; kapsam dışı canary'yi
  **mutlak yolla okuyabildi**. Bu sınır kapatılmadı (aşağıya bakın).
- **Claude Code 2.1.303:** `--tools= --bare --strict-mcp-config --help`,
  722 ms, çıkış 0, **0 işaret**; dört vektör de “sessiz · kapatma yolu
  bildirildi”. `--bare` yardım yoklamasını 13,1 sn'den ~0,7 sn'ye indirdi.
- **Codex CLI 0.159.2:** `exec --ignore-user-config --ignore-rules --help`,
  26 ms, çıkış 0, **0 işaret**; proje/hook/MCP “sessiz · bildirildi”, canary
  “sessiz · koruma kanıtlanmadı” (`tool_free` yol yok).
- **Antigravity:** kurulu değil → koşu yok. **Kimi 1.41.0:** etkileşimli-only →
  otomatik saldırı koşusu yapılmaz.

### Arayüz

Providers → “Başlangıç saldırı testi” kartı: “Saldırıları sına” düğmesi
(`cli_attack_probe`, async, `spawn_blocking`, tek CLI işlemi `E_CLI_BUSY`),
fikstür özeti (dedektör, kapı) + ürün yapılandırması satırları, adaptör başına
sürüm/argv/durum ve dört vektör (kapatma bayrakları + karar rozeti), uyarılar ve
“Saldırı testinin sınırları”. Denetime `cli_attack`/`probe` yazılır: yalnız
sayı, durum ve karar kimlikleri — **fikstür belirteci, dosya adı ve yardım
metni yazılmaz**.

### Dürüst sınırlar

- **Sessizlik davranış kanıtı değildir.** Gerçek CLI'ya istem gönderilmedi
  (T06–T08 istem yolu yok); hook/MCP/proje talimatı ancak bir oturum açıldığında
  yüklenir. Bu çağrıda oturum açılmadı.
- **Dedektör kanıtı kendi vekilimizle ölçülür**: `naive_load` gerçek CLI kodu
  değil, fikstürün “naif yükleyici” taklididir. Kanıtladığı şey dedektörün kör
  olmadığıdır; gerçek CLI'ların hook çalıştırdığı değil.
- **Kapsam dışı canary kapatılmadı:** `unsafe_code = "forbid"` olduğu için
  seccomp/kum havuzu yok; mutlak yolla okuma OS düzeyinde engellenmiyor.
  Koruma istem/araç yolunun yokluğuna dayanıyor; arayüz bunu uyarı olarak yazar.
- Bayrakların **kabul edildiği** görüldü (çıkış 0 + yardım metni); araç
  çağrısının gerçekten sıfır olduğu, oturum yazılmadığı **ölçülmedi**.
- Windows'ta betik yürütme yok (`sh` yok) → fikstürün hook/MCP adımları
  çalışmaz; bu platformda yalnız dosya yerleşimi ve kapı testleri anlamlıdır.
- Tauri penceresi açılmadı → kartın görsel QA'sı yok.

## Kalıntı denetimi — ölçüm ve temizlik (F10-T13, 2026-10-24)

`crates/cli-adapters/src/residue.rs` — transcript/log/oturum kalıntısı için
**envanter + ölçüm + sınırlı temizlik**.

- **Bildirilen yollar sicilde sabit:** her adaptör için ev dizinine göreli liste
  (`.claude/projects`, `.codex/sessions`, …) tür ve notla birlikte tanımlı.
  Kullanıcı ev dizini **genel olarak taranmaz**; yalnız bu yollar.
- **Yalnız üst veri:** ad, bayt, değişim zamanı okunur; **dosya içeriği
  açılmaz**, sembolik bağ izlenmez, derinlik 8 ve dosya 4000 ile sınırlı.
- **Ölçüm (`measure_residue`):** sürüm yoklaması boş kum havuzu iş dizininde,
  `HOME` **kullanıcının kendi ev dizini** (açık onay: `confirmUserHome`);
  önce/sonra anlık görüntüsü karşılaştırılır → `created` / `modified` /
  `removed` / `createdDirs`.
- **Temizlik iki kapılı:** `cleanup_created` yalnız ölçümde oluşan dosyaları
  siler; `cleanup_created_since` arayüz yolunda ayrıca **değişim zamanı
  ölçümden yeni değilse dokunmaz**. İkisinde de kapsam dışı yol, `..`, mutlak
  yol ve sembolik bağ reddedilir (`E_CLI_RESIDUE_SCOPE`,
  `E_CLI_RESIDUE_SYMLINK`). Önceden var olan dosya **asla** silinmez.
- **Kum havuzu süpürme (`sweep_stale_sandboxes`):** çökme sonrası kalan
  `dim-cli-sandbox-*` dizinleri; yalnız dizin, sembolik bağ değil, Unix'te
  `0700`, sahibi biz ve en az **1 saat** yaşında. Süren koşuya dokunulmaz.
- **Oturum kalıcılığını kapatma (sürüm bazlı, `--help` ile doğrulandı):**
  Claude Code `2.1.303` → `--no-session-persistence` (yalnız `--print`),
  Codex CLI `0.159.2` → `exec --ephemeral`. İkisi de argüman kapısına eklendi.
  Kimi/Antigravity için **doğrulanmış kapatma yolu yok**; arayüz bunu yazar ve
  koruma `HOME` yönlendirmesine (T05) kalır.
- **Arayüz:** “CLI kalıntı denetimi” kartı — adaptör seçimi, “Envanteri tara”,
  yol tablosu (tür, durum, yaş, not), kalıcılık satırı, onay kutusuyla
  “Kalıntıyı ölç”, ölçüm sonucu (önce/sonra, oluşan/değişen dosyalar),
  “Yalnız bu ölçümde oluşanları sil” ve “Bekleyen kum havuzlarını temizle”.
- **Denetim:** `cli_residue` / `measure` ve `cleanup` — yalnız sayılar ve
  politikalar; **dosya adı ve içerik yazılmaz**.

### Gerçek ölçüm (bu makine, macOS aarch64)

`cargo run -p dim-cli-adapters --example residue -- <adaptör>`:

| Adaptör | Sürüm | Mevcut kalıntı | `--version` sonrası yeni |
|---|---|---|---|
| Claude Code | 2.1.303 | 396 dosya · 590 MB (389 transkript) | **0 dosya** |
| Codex CLI | 0.159.2 | 94 dosya · 235 MB (93 oturum) | **0 dosya** |

Bekleyen kum havuzu: 0. Yani sürüm yoklaması kalıntı üretmiyor; asıl kalıntı
kullanıcının kendi oturumlarından geliyor ve **uygulama onlara dokunmuyor**.

### Dürüst sınırlar (T13)

- Envanter **bildirilen** yollardır: CLI'nin listede olmayan bir yere yazması bu
  araçla yakalanmaz. Yeni sürüm yolu değiştirirse sicil güncellenmelidir.
- Ölçüm `--version` yoklamasıdır; gerçek bir istem/oturum sırasında oluşan
  kalıntı **ölçülmedi** (istem gönderilmiyor). Bu yüzden `--ephemeral` /
  `--no-session-persistence` bayraklarının etkisi de ölçümle değil, `--help`
  belgesiyle doğrulandı.
- Temizlik yalnız **bu ölçümde oluşan** dosyalara uzanır; kullanıcının önceki
  oturumlarından kalan 590 MB / 235 MB kalıntıyı silmek üründe **yok** (bilinçli:
  o dosyalar kullanıcı verisi olabilir). Kullanıcı bunları elle siler.
- Windows'ta Unix izin/sahiplik denetimi yok; süpürme orada yalnız önek + yaş +
  dizin koşullarına bakar ve test edilmedi.
- Tauri penceresi açılmadı → kartın gerçek tıklaması/görsel QA'sı yok. Ölçüm ve
  temizlik çekirdeği gerçek süreçle test edildi (sentetik işaretçi testi).

## Yetenek matrisi ve bağlantı durumu (F10-T11, 2026-10-24)

`crates/cli-adapters/src/capability.rs`. Dört yetenek kaydedilir: **araçsız
çalışma** (`tool_free`), **hook/MCP kapalı** (`hook_mcp_off`), **şemalı çıktı**
(`structured_output`), **oturum kalıcılığı kapalı** (`session_persistence_off`).

### Kanıt türü ve durumlar

Kanıt, **kurulu sürümün kendi `--help` metni**dir; sürüm numarası tahmin edilmez,
rapora kurulu sürüm yazılır. Yardım metni de `exec::run_cli` üzerinden okunur:
izinli argv, allowlist ortam, boş `0700` kum havuzu, 30 sn sınır, iptal.

- `Declared` — bayrak(lar) yardım metninde **belirteç olarak** var
  (`mentions_flag`: öncesi boşluk/parantez/virgül, sonrası boşluk/`=`/`,`/`<`;
  `--ephemeral` metni `--ephemeral-extra` ile eşleşmez). **Davranış kanıtı
  değildir**: araç çağrısı yapılmadı, oturum yazımı ölçülmedi.
- `Missing` — yardım metni okundu, bayrak **yok**: bu sürümde yol yok.
- `NoPath` — sicilde doğrulanmış yol yok (Kimi/Antigravity taşıması, `codex exec`
  araçsız kipi). "Desteklenmiyor" demek için kanıt olmadığı için bu durum
  ayrı tutulur.
- `Unread` — yardım okunamadı (kurulu değil, sıfır olmayan çıkış, süre, iptal).
  Kesilmiş/boş çıktı **kanıt sayılmaz** (`help_text_of`).

### Bağlantı durumu ve kapı

| Durum | Kural | Arayüz |
|---|---|---|
| `not_installed` | sürüm okunamadı | “kurulu değil” |
| `incompatible` | gerekli bir yetenek `Missing` **ya da** `batch_forbidden` | “uyumsuz” (kırmızı) |
| `limited` | gerekli bir yetenek `NoPath`/`Unread` | “sınırlı” (sarı) |
| `declared` | gerekli yeteneklerin hepsi `Declared` | “bildirildi · denenmedi” (sarı — yeşil değil) |

`gate_open` yalnız `declared` durumunda `true`; `gate_reasons` makine okunur
`yetenek:durum` listesidir (`tool_free:no_path`). Arayüz bunu okunur satıra
çevirir. **Bugün kapı yalnız raporlanıyor**: istem gönderen yol (T06–T08) henüz
yok; olduğunda `ensure_capable` benzeri bir çağrı bu raporu kullanacak.

### Matris ile argüman kapısı ayrışmasın

Matriste bildirilen her bayrak `AdapterSpec.allowed_args`'a eklendi
(`claude`: `--tools`, `--bare`, `--strict-mcp-config`, `--json-schema`;
`codex`: `--output-schema`, `--ignore-user-config`, `--ignore-rules`).
`declared_capability_flags_pass_the_argument_gate` testi ikisini birlikte tutar:
matriste bildirilen bir bayrak kapıya takılırsa test kırmızı olur.

### Arayüz

Providers ekranında **Yetenek matrisi** kartı: “Yetenekleri sına” düğmesi
(`cli_capability_matrix`, async, `spawn_blocking`; aynı anda tek CLI işlemi,
`E_CLI_BUSY`), adaptör başına sürüm + yoklanan komut (`claude --help`,
`codex exec --help`), dört satır (durum rozeti + bayrak/eksik/nereden) ve
bağlantı + kapı rozeti. “Matrisin sınırları” katlanır listesi dürüstlük
notlarını taşır. Denetime `cli_capability`/`probe` yazılır: adaptör, sürüm,
bağlantı, kapı, gerekçe — **yardım metni yazılmaz**.

### Gerçek ölçüm (bu makine, macOS aarch64)

`cargo run -p dim-cli-adapters --example caps`

| Adaptör | Sürüm | Yardım | Sonuç | Kapı |
|---|---|---|---|---|
| Claude Code | 2.1.303 | 13.121 ms | 4/4 `declared` | **açık** |
| Codex CLI | 0.159.2 | 78 ms | 3/4 `declared` · `tool_free` = `NoPath` | kapalı (sınırlı) |
| Kimi CLI | 1.41.0 | yoklanmadı (etkileşimli-only) | 4/4 `NoPath` | kapalı (uyumsuz) |
| Antigravity | kurulu değil | — | 4/4 `Unread` | kapalı |

`claude --help` 13 sn sürdüğü için `HELP_TIMEOUT` 20 → **30 sn** yapıldı.
Codex'te `--sandbox read-only` araçları kısıtlar ama kapatmaz; bu yüzden
`tool_free` bilinçli olarak `NoPath` — "araçsız" denmez.

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
- **Yeteneğin kanıtı yardım metnidir, davranış değil.** `Declared` satırı
  "bayrak var" der; araç çağrısının gerçekten sıfır olduğu, oturum dosyasının
  yazılmadığı veya şemalı çıktının uyduğu **ölçülmedi** (istem gönderilmiyor).
  Bu yüzden en iyi durum bile "bildirildi · denenmedi" etiketiyle sarı gösterilir.
- Matris yalnız **bildirilen** bayraklara bakar; CLI'nin yardım metninde
  yazmayan bir kapatma yolunu (ör. ortam değişkeni, yapılandırma dosyası)
  bilmiyoruz. Kimi ve Antigravity için doğrulanmış taşıma yok; `codex exec`
  araçsız kip bildirmiyor.
- Sürüm yoklaması (`discover`) yardım yoklamasından ayrıdır: `--version` hâlâ
  kullanıcının çalışma dizininde koşar (ortam artık allowlist'e çekildi).
  `--help` ise boş kum havuzunda koşar.
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

- `cargo test -p dim-cli-adapters` → **97** (F10-T01 turunda 17 yeni: Mach-O thin/fat
  dilimleri, ELF x86_64/arm64, PE AMD64/ARM64, 4 KiB ötesindeki PE başlığı, bozuk
  PE/FAT başlığı, betik → yorumlayıcı mimarisi, evrensel yorumlayıcı → `fat`,
  bulunamayan yorumlayıcı, `env` bayrakları, tanınmayan/boş dosya, sembolik bağ,
  gerçek test ikilisinin kendi mimarisiyle eşleşmesi; F10-T12 turunda 17 yeni: fikstür
  yerleşimi, dört vektörün tetiklenmesi, temiz yerleşimde sessizlik, işaretçinin
  diskten okunması, çıktı yansıması, kirli proje dizininin iş dizini olarak
  reddi, ürün yapılandırmasında proje/hook/MCP kapalı + canary açık, fikstür
  temizliği, kanıtsız dedektörde “koruma yok” kararı, bildirilmiş/korumasız
  ayrımı, tetiklenmenin üstünlüğü, sertleştirme argv'sinin argüman kapısından
  geçmesi, raporun belirteci taşımaması, gerçek alt süreçte hook yakalama,
  sessiz CLI'da sıfır işaret, kirli iş dizinini reddeden gerçek koşu).
  F10-T11 turunda 13 yeni: bilinmeyen
  adaptör, Claude 4/4 bildirildi, eksik bayrak → uyumsuz + kapalı kapı, okunamayan
  yardım → sınırlı, kurulu değil, etkileşimli-only uyumsuz, codex `tool_free`
  doğrulanamadı, bayrak belirteç sınırı, matris ↔ argüman kapısı tutarlılığı,
  her adaptörün sicilde olması, gerçek yardım süreci, kesik yardım kanıt değil,
  sıfır olmayan çıkış kanıt değil). F10-T13 turunda 14, T05 turunda 13 yeni test
  (ayrıntı: kum havuzu, kalıntı, stdin, ölçüm/temizlik).
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` temiz;
  `clippy -p dim-cli-adapters --all-targets -D warnings` temiz; rustfmt iki
  manifest temiz.
- Web: **246** test (F10-T01 turunda 15 yeni `arch.test.ts`: tonlar, evrensel rozet
  metni, mimari listesi, biçim/yol satırları, neden çevirisi, shebang satırı, özet,
  en kötü karar sırası, farklı/evrensel-uyumsuz/doğrulanamayan uyarıları, kart
  rozeti, zaman damgası; F10-T12 turunda 25 yeni `attack.test.ts`: karar tonları ve
  metinleri, kapatma yolu, komut satırı, atlama nedenleri, fikstür özeti ve
  uyarıları, adaptör uyarıları, karar sayımı/en kötü karar/toplam işaret;
  F10-T11 turunda 8 yeni `capability.test.ts`: durum/bağlantı
  tonları, özet sayıları, kapı metni, gerekçe çevirisi, uyarılar, en kötü durum,
  yoklanan komut), `tsc --noEmit` + `vite build` yeşil.
- Gerçek yetenek yoklaması (`cargo run -p dim-cli-adapters --example caps`):
  Claude 4/4 `declared` (kapı açık), Codex 3/4 (`tool_free` doğrulanamadı),
  Kimi uyumsuz, Antigravity kurulu değil — tablo yukarıda.
- Gerçek mimari denetimi (`cargo run -p dim-cli-adapters --example arch`):
  Claude Code ve Codex CLI Mach-O `aarch64` → `same`; Kimi sembolik bağ →
  Python betiği → evrensel yorumlayıcı (`fat`, uygulama mimarisi var);
  Antigravity PATH'te yok → `unknown`.
- Gerçek kalıntı ölçümü (`--example residue -- …`): Claude Code `2.1.303`
  401 ms, Codex `0.159.2` 124 ms; ikisinde de `--version` **0 yeni dosya**
  bıraktı. Mevcut kalıntı: Claude 396 dosya / 590 MB, Codex 94 dosya / 235 MB
  (uygulama bunlara dokunmuyor).
- Gerçek kum havuzu koşusu (`--example probe -- …`): iş dizini `0700`,
  `HOME`/`TMPDIR` kum havuzunda, kalan dosya 0. `HOME` yönlendirmesi gerçek
  araçların `--version` yolunu bozmadı.
- **CI:** `d9a5581` → run `40109851908` **7/7 yeşil** (arayüz, SBOM, masaüstü
  kabuk, rust ubuntu+macos, belge worker'ı, sözleşmeler). F10-T11: `c165063` →
  run `40114620637` **7/7 yeşil** (aynı matris). **F10-T12:** `753aed7` +
  `bba6485` (rustfmt) → run `40121007054` **7/7 yeşil** (arayüz, SBOM, masaüstü
  kabuk, rust ubuntu+macos, belge worker'ı, sözleşmeler).
