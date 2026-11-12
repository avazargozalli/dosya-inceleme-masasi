# F10 — Claude, Codex, Antigravity ve Kimi CLI adaptörleri

- **Sürüm etiketi:** `v0.9.0` (hedef)
- **Durum:** Devam ediyor — sicil, argüman kapısı, izinli yürütme çekirdeği,
  **mimari uyum denetimi (T01)**, **iş dizini kum havuzu (T05)**, **yetenek matrisi (T11)**,
  **başlangıç saldırı testleri (T12)**, **kalıntı ölçümü/temizliği (T13)**,
  **istem argv biçimi + `--print` kapısı (T08 kısmi)**, **komut eş adı çözümü (T08c)**,
  **ölçülen araç erişiminin kapıya girmesi + kasa içinde saklanması (T08d)**,
  **koşulamayan sınamanın ölçüm sayılmaması + nedenin ayrılması (T08e)** ve
  **arayüzden tetiklenen gerçek sınama** hazır; **istem gönderen** uçtan uca
  sağlayıcı akışı (T08) ve Kimi API profili (T10) yok
- **Tarih:** 2026-10-24

| İş | Durum | Kanıt |
|---|---|---|
| F10-T01 | ✅ | mutlak PATH keşfi + `--version` + **başlık baytlarından mimari** (Mach-O thin/fat, ELF, PE, `#!` betiği → shebang); `same`/`different`/`fat`/`unknown`; arayüzde “mimari” rozeti + “Mimarileri denetle”. Süreç başlatılmaz |
| F10-T02 | ✅ | adaptör kartı: kurulu / kurulu değil / doğrulanamadı |
| F10-T03 | ➖ | Ürün CLI oturumunu yönetmez; kimlik dosyası **okunmaz/kopyalanmaz**. Giriş/çıkış CLI'ın kendi sorumluluğu (ADR-0005) |
| F10-T04 | ✅ | `sanitize_env`: allowlist + gizli ad reddi; `env_clear()`; arayüzden geçen gerçek koşuda doğrulandı |
| F10-T05 | ✅ | `sandbox::check_workdir` (mutlak/bağsız/boş/ev dışı/`0700`) + `E_CLI_WORKDIR_PERM`; kum havuzu içinde `HOME`/`TMPDIR`; stdin ≤ 1 MiB (`E_CLI_STDIN`); çalıştırma sonrası dosya sayımı `RunScope`; arayüzde kapsam paneli; gerçek CLI koşusunda doğrulandı |
| F10-T06 | ✅ | Claude profili: `--no-session-persistence` + **`--tools ""`** + `--strict-mcp-config` zorunlu (sertleştirme); gerçek çağrı `DIM-CANARY-48`. Not: kimlikli `--print` çağrısı allowlist'te **`USER`** olmadan "Not logged in" veriyordu — eklendi (ölçüldü) |
| F10-T07 | ✅ | Codex `--json` NDJSON ayrıştırıcı (`events.rs`): metin + usage, bilinmeyen tip korunur |
| F10-T08 | 🚧 | Antigravity 1.4.7: `--print/--output-format/--json-schema/--sandbox/--print-timeout/--disable-slash-commands`; `--sandbox` + slash kapatma zorunlu, `DIM-CANARY-49`. **Eksik:** `--sandbox` dosya okumayı engellemiyor (canary okundu, ölçüldü) ve araçsız kip yok; `--print` yanındaki bayrağı istem sanıyor — **istem argv biçimi ölçüldü ve kapıya bağlandı (2026-11-12)**: `prompt_argv`/`prompt_shape` + `ensure_prompt_shape` (`E_CLI_ARGUMENT`). **İstemi gönderen uçtan uca akış var (2026-11-12, T08b):** `run_cli_with_prompt`/`splice_prompt` + `hardening.rs` davranış sınaması — ölçüldü: Claude `blocked`, **Codex `granted`** (kanıt dosyasını okudu). Antigravity üründe **koşulamadı**: `command` adı `antigravity`, kurulu ikili `agy` (açık kayıt) — **2026-11-12 (T08c): eş ad çözümü eklendi**, ürün `agy` 1.5.6'yı buluyor; davranış sınaması koştu, yanıt boş → `inconclusive`. **2026-11-12 (T08d):** sınama kaydı kasa içine (`device` kapsamı) yazılır; `granted` beyanı çürütür → satır `contradicted`, bağlantı **uyumsuz**, istem kapısı `tool_access_granted` ile kapanır. Ölçüm kapıyı yalnız **kapatır** (`blocked` beyanı doğrulamaz). **2026-11-12 (T08e):** boş yanıtın **nedeni** artık ayrılır — `NotRun` (`quota`/`auth`/`process_failed`/`timeout`/`cancelled`) + `--output-format=json` zarfı (`response`); sınama **koşmadıysa** `measured_access()` `None` döner ve kanıt **yazılmaz**. Ölçüm (bu makine): agy kotası dolu → `koşmadı: quota … 429`; öncesi sessiz `inconclusive` + "ölçüldü · hüküm yok" kaydıydı |
| F10-T09 | ✅ | Kimi `InteractiveOnly` + `batch_forbidden` → `E_CLI_BATCH_FORBIDDEN` (arayüz düğmesi de kapalı) |
| F10-T10 | ⬜ | Kimi API profili yok |
| F10-T11 | ✅ | yetenek matrisi: adaptör × kurulu sürüm × `--help` kanıtı; gerekli yetenek `Missing` → **uyumsuz**, doğrulanamıyor → **sınırlı**, kapı kapanır; arayüzde “Yetenekleri sına” |
| F10-T12 | ✅ | başlangıç saldırıları: hook / MCP / proje talimatı / kapsam dışı canary vektörleri geçici fikstürde; **dedektör kanıtı** (kirli 4/4, temiz 0/4), kirli proje dizini iş dizini olarak **reddedilir**, kurulu CLI'larda sertleştirme bayrakları + `--help` ile gerçek koşu (işaret 0); arayüzde “Saldırıları sına” |
| F10-T13 | ✅ | kalıntı envanteri (sürüm bazlı bildirilen yollar) + önce/sonra ölçüm + **yalnız uygulamaya ait** dosya temizliği + çökme kum havuzu süpürme; arayüzde “Kalıntı denetimi” kartı |
| F10-T14 | ✅ | süre sınırı, iptal, alt süreç öldürme, kısmi çıktı — **arayüz düğmesinden uçtan uca**; ek tur: başlatma `ETXTBSY` yarışı — yalnız **meşgulken** yeniden dene (kontrol kanıtı dâhil, Linux'ta üretilir) |

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

### Arayüz uçuş yarışları — yanıt adaptör kimliğine bağlı (2026-11-04)

Kart dört çekirdek komutu çağırır (`cli_residue_scan`, `cli_residue_measure`,
`cli_residue_cleanup`, `cli_stale_sandbox_sweep`). Adaptör seçimi değişince
kart durumu temizliyordu; ancak **uçuştaki** yanıt hangi adaptör için
istendiğini taşımıyordu. Ölçülen altı kusur:

1. **Çapraz envanter:** tarama askıda iken başka adaptöre geçilince eski
   adaptörün kökü/yolları yeni adaptörün panelinde çiziliyordu; hatası da
   yeni adaptöre yazılıyordu.
2. **Çapraz ölçüm + yanlış temizlik hedefi:** ölçüm yanıtı hem `measure`ı hem
   `scan`ı (`after`) yazıyordu → yeni adaptörde “ölçüm bitti” ve temizlik
   düğmesi açılıyor; temizlik **eski adaptörün** ölçüm zamanı ve dosya
   listesiyle yeni adaptör adına gönderilebiliyordu.
3. **Çapraz temizlik:** temizlik askıda iken adaptör değişince özet yeni
   adaptörde çiziliyor, ardından **eski adaptör** yeniden taranıp sonucu yeni
   adaptöre yazılıyordu.
4. **Kilit devri yok:** adaptör değişimi `busy` bayrağını bırakmıyordu → eski
   istek dönene kadar yeni adaptörün “Envanteri tara” düğmesi kapalıydı.
5. **Erken açılan kilit:** tek `busy` bayrağı paylaşıldığı için eski isteğin
   `finally`si yeni adaptörde başlatılan isteğin kilidini açıyordu (tarama,
   ölçüm, temizlik ve süpürme için ayrı ayrı ölçüldü).
6. **Bayat tazeleme:** temizlik sonrası yeniden tarama askıda iken adaptör
   değişirse eski adaptörün envanteri yeni adaptörün panelinde çiziliyordu.

Düzeltme: `contextSeq` sayacı. Her işlem istek anındaki adaptörü ve sırayı
saklar; yanıt yalnız aynı bağlamda yazılır (başarı, hata ve `finally` yolu).
Adaptör değişimi sayacı artırır ve kilidi bırakır. Temizlik hedefleri istek
anında donar; tazeleme istek anındaki adaptör için koşar.

Doğrulama: `Providers.test.tsx` 17 → 30 test (13 yeni). Düzeltme öncesi ham
koşu **8 failed | 19 passed**; kapı kanıtı **13/13** mutasyon (ilk koşuda üç
mutasyon hayatta kaldı — ölçüm/temizlik kilit kapıları ve temizlik tazeleme
kapısı — üç test eklendi). `tsc --noEmit` temiz, vitest **929/929** (60 dosya),
`npm run build` temiz (579,03 kB / gzip 156,06 kB). Kod `519be7d`.

Dürüst sınırlar: jsdom tarayıcı değil — gerçek Tauri penceresi, gerçek CLI
süreçleri ve görsel QA yok; `api` mock'lu olduğu için `cli_residue_*` SQL/Rust
tarafı ve gerçek dosya silme koşmaz. Testler yanıt **sırasını** (elle çözülen
sözler) denetler, gerçek gecikmeyi değil. Süpürme adaptörden bağımsızdır ama
adaptör değişiminde sonucu **bilinçli olarak** yazılmaz: tek bayrak kilidi
paylaşıldığı için bağlam kapısına dahil edildi.

## İptal, yenileme ve profil kaydı — üst panel uçuşları (F10-T14, 2026-11-04)

Ekranın üst panelindeki üç kısa ömürlü yol — liste okuması (`provider_probe`),
iptal sinyali (`provider_probe_cancel`) ve API profili yazımı
(`provider_api_profile_save`) — bugüne dek yalnız “istek gitti / ileti geldi”
düzeyinde ölçülmüştü. Bu turda uçuş kilidi, bayat ileti ve hata sınıfı ölçüldü;
**yedi kusur** bulundu (hepsi düzeltme öncesi bileşende kırmızı verdi):

1. **Bayat yoklama kanıtı:** başarılı “Yenile” önceki yoklamaların sonucunu
   temizlemiyordu. Liste yeniden okunup adaptörün sürümü/yolu değişince eski
   komut, ortam ve çıktı **yeni satırın altında “güncel”** gibi duruyordu
   (matris, sınama, mimari ve paket raporları yeni koşuda zaten temizlenir).
2. **Çift iptal:** iptal düğmesinde uçuş kapısı yoktu; çift tık iki
   `provider_probe_cancel` gönderiyordu. Çekirdek iptali süreç geneli olduğu
   için ikinci istek, o sırada başlatılan **yeni** bir koşuyu iptal edebilirdi.
3. **Bayat yeşil ileti:** düşen iptal, önceki “İptal istendi…” satırını
   ekranda bırakıyordu (kırmızı hata ile yeşil ileti aynı anda).
4. **“[object Object]”:** profil kaydı reddi `String(error)` ile yazılıyordu.
   Çekirdek reddi `{code, message}` nesnesidir (Tauri `invoke`), bu yüzden
   ekrana hata kodu ve iletisi yerine `[object Object]` düşüyordu.
5. **Çift yazma + açık alanlar:** profil kaydında uçuş kilidi yoktu (çift tık
   iki yazma); alanlar uçuşta açık kaldığı için yanıt, ekrandaki
   **kaydedilmemiş** değerlerin kaydedildiği izlenimini veriyordu.
6. **Bayat onay:** alan değişince önceki “Profil kaydedildi.” satırı kalıyordu
   → yeni değerin altında yeşil onay görünüyordu.
7. **Boşluk kapısı:** yalnız boşluktan oluşan uç nokta/model “dolu” sayılıp
   kaydetmeyi açıyordu; değerler kırpılmadan çekirdeğe gidiyordu.

Düzeltme (`Providers.tsx` +60/−12): yenileme **başarısında** `results` ve
`runErrors` temizlenir (liste yenidir, kanıt önceki listenin); iptal
`cancelBusy` tek uçuş kilidi taşır (düğme uçuşta kapalı) ve deneme başında eski
iletiyi siler; profil kaydı `saveProfile` içinde tek uçuştur, iki alan ve düğme
uçuşta kilitlidir, `profileFailureText` çekirdek reddini `{code} — {message}`
olarak yazar (`Error` yolu iletisini korur), alan değişiminde ileti temizlenir,
`profileReady` boşluk kapısıdır ve çağrıda değerler `trim()` edilir.

Doğrulama: `Providers.test.tsx` 30 → 37 test (7 yeni). Düzeltme öncesi ham koşu
**7 failed | 30 passed** (her kusur kendi testini düşürdü: `<dd>` kalıntısı,
`disabled` `false`, `muted small` satırı, `/E_AI_PROFILE/` bulunamadı, iki
çağrı, `card-sub` onayı, `false`). Kapı kanıtı **9/9** mutasyon kırmızı; geri
alma sha256 `7f2b3e5736ce…d1cf78` birebir. `tsc --noEmit` temiz, vitest
**936/936** (60 dosya), `npm run build` temiz (**579,43 kB**, gzip **156,20 kB**).
Rust değişmedi; rustfmt iki manifestte `--check` temiz. Kod `e77a104`.

Dürüst sınırlar: jsdom tarayıcı değil — gerçek Tauri penceresi, gerçek CLI
süreçleri ve görsel QA yok; `api` mock'lu olduğu için `provider_probe`,
`provider_probe_cancel` ve `provider_api_profile_save` komutlarının Rust tarafı
**koşmaz**. Testler yanıt sırasını elle çözülen sözlerle denetler, gerçek
gecikmeyi değil. Yenileme yoklamaları **başarılıysa** temizler: düşen yenileme
listeyi ve kanıtı olduğu gibi bırakır (liste değişmedi). Profil iletisi hâlâ
kart içinde düz satırdır (`role="alert"` yok) — ekran okuyucu duyurusu
**insan** VoiceOver turunda doğrulanacak.

## Alt kartlar — çekirdeğin tek CLI kilidi (F10-T15, 2026-11-04)

Çekirdekte CLI işlemleri **süreç geneli tek** slottan geçer
(`AppState::cli_probe_lock`, `commands.rs`): `provider_probe_run`,
`cli_capability_matrix`, `cli_attack_probe` ve `cli_residue_measure` aynı slotu
ister, ikinci istek `E_CLI_BUSY` alır. Arayüz bu gerçeği **kart başına ayrı**
bayraklarla ve tek yönlü kapılarla yansıtıyordu: kapı yalnız “sonradan başlayan”
kartı kapatıyor, ters yön açık kalıyordu. Kullanıcı bunu sağlayıcı arızası gibi
okuyordu — kart `setMatrix(null)` gibi “bayat kanıtı sil” adımını çoktan koşmuş,
çekirdek isteği reddetmiş oluyordu.

Ölçülen yedi kusur (yedisi de düzeltme öncesi bileşende kırmızı verdi):

1. **Matris sürerken satır yoklaması açıktı** (`probeCanRun` yalnız `running`e
   bakıyordu) → “Bağlantıyı sına” `E_CLI_BUSY` alır, satırın önceki kanıtı
   silinirdi.
2. **Saldırı sürerken matris açıktı** → matris `E_CLI_BUSY` alır ve **önceki
   matris raporu** ekrandan kalkardı (rapor yeni koşunun başında temizlenir).
3. **Saldırı sürerken satır yoklaması açıktı** (kusur 1'in ikinci yüzü).
4. **Matris sürerken kalıntı ölçümü açıktı** (onay kutusu işaretliyken) → gerçek
   ev dizininde koşan ölçüm reddedilir, panelde kırmızı `E_CLI_BUSY` çıkardı.
5. **Ölçüm sürerken matris ve saldırı açıktı** — ölçüm kilidi üst panele hiç
   bildirilmiyordu.
6. **Ölçüm sürerken satır yoklaması açıktı.**
7. **Yoklama sürerken ölçüm açıktı;** ayrıca **adaptör değişince** panel kendi
   kilidini bırakıyor ama çekirdekteki ölçüm sürüyordu → yeni adaptörde “Ölç”
   `E_CLI_BUSY` alırdı (kilit devri çekirdek kilidini yok sayıyordu).

Düzeltme (`Providers.tsx` +45/−6): üst panelde tek `coreBusy` bayrağı; satır
yoklaması, matris, saldırı ve **kalıntı ölçümü** (`ResiduePanel` `onCoreBusy` ile)
bu bayrağı tutar. Kartın kendi `busy` değeri yalnız **etiket** içindir. Bayrak
**adaptör değişiminde bırakılmaz**; ölçümün `finally`si adaptör değişse de kilidi
açar (istek çekirdekte bitmiştir). Tarama, temizlik ve süpürme çekirdek kilidini
tutmaz — bilinçli olarak açık bırakıldı (dosya üst verisi okur/siler, CLI süreci
başlatmazlar).

Doğrulama: `Providers.test.tsx` 37 → 45 (+8 test). Düzeltme öncesi ham koşu
**7 failed | 37 skipped** (yedisi de `disabled` kapısıydı); sekizinci test
(“yoklama sürerken ölçüm ve matris kapalı”) düzeltmeyle yazıldı, kapı kanıtı M4
ile ölçüldü. Kapı kanıtı **10/10 mutasyon** kırmızı: M1 matris kapısı (2), M2
saldırı kapısı (1), M3 yoklama kapısı (3), M4 ölçüm kapısı (3), M5
`onCoreBusy(true)` (3), M6 `onCoreBusy(false)` (2), M7 yoklamada bayrak (1), M8
matriste bayrak (2), M9 saldırıda bayrak (2), M10 ölçümde `setBusy(null)` (1);
geri alma sha256 `3071e1d9c7e9…b5d89f9d` birebir. `tsc --noEmit` temiz, vitest
**944/944** (60 dosya), `npm run build` temiz (**579,51 kB**,
`index-DwnYvEqS.js`, gzip **156,27 kB**). Rust değişmedi.

**Dürüst:** ilk M6 denemesi betikte yanlış satırı (`setBusy(null)`) kaldırdı ve
**yeşil kaldı** → ölçümün `finally`si için “düğme yeniden açılır” assertion'ı
eklendi, M10 kırmızı verdi (yedekli/ölçüsüz kapı bırakılmadı). Dürüst sınırlar:
jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek CLI süreçleri ve görsel
QA yok; `E_CLI_BUSY` gerçekte Rust'tan gelir, testler kapıyı `disabled` üzerinden
ölçer. Ölçüm düğmesi ancak bir tarama çizildikten sonra görünür; ölçüm yarışları
bu yüzden tarama üzerinden kuruldu. Mimari ve paket raporları çekirdek kilidini
tutmaz; mimari düğmesi eski (aşırı) kapısını korur — bu turda değiştirilmedi.

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
çevirir. **Bugün bu rapor istem gönderim kapısını da besler** (2026-11-12):
`prompt_shape` içindeki `printable` yalnız toplu yürütme açık **ve** `tool_free`
`Declared` ise `true`; değilse `printable=false` ve neden yazılır — yani
"araçsız kip kanıtlanmadıysa istem gönderilmez" kuralı matrisin kanıtına
bağlanmıştır (bkz. "İstem argv biçimi ve `--print` kapısı").

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
- **İstem kapısı** (`ensure_prompt_shape`, F10-T08): değer alan `--print`
  (bugün Antigravity) **çıplak** kalır ve yanındaki belirteç `-` ile başlarsa
  reddedilir (`E_CLI_ARGUMENT`) — vendor o belirteci istem sanıp sertleştirme
  bayrağını **uygulamaz** (ölçüldü). Kapı adaptör-farkındadır: Claude'un boolean
  `--print`i etkilenmez.
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
- **Başlatma**: yeni yazılmış dosyayı hemen çalıştırmak `ETXTBSY` ("text file
  busy") verebildiği için spawn yalnız **bu** hatada 6 kez (50 ms arayla)
  denenir; kalıcı hatalar (`NotFound`, `PermissionDenied`) ilk denemede döner,
  son hata metni `E_CLI_EXEC_SPAWN` ile raporlanır (aşağıdaki "Başlatma
  yarışı" bölümü).

## İstem argv biçimi ve `--print` kapısı (F10-T08, 2026-11-12)

**Ölçülen kusur (bu makine, kurulu `agy` 1.4.12):**

```
$ agy --print --sandbox --output-format json
Error: --print took "--sandbox" as its prompt, so the intended prompt was
left as an argument and ignored.
Attach the prompt to the flag (--print='your prompt') and move --sandbox elsewhere.
```

Yani Antigravity'de `--print` **değer alan** bir bayraktır: komşu belirteç ne
olursa olsun **istem** sayılır. Ürün sertleştirme bayrağını `--print`in yanına
koyarsa bayrak **uygulanmaz** ve sessizce isteme dönüşür — sertleştirmenin
sessizce kaybolması, "ölçüldü" demeden görülemeyecek bir kusurdur. `--print=`
biçimi ayrı ayrışır (ölçüldü: `--print= --sandbox` → "empty prompt", bayrak
hatası yok) — bu yüzden biçim **tek belirteçli** olmak zorunda.

**Yazılan (çekirdek, `crates/cli-adapters/src/exec.rs`):**

- `PROMPT_MAX_BYTES` = **16 KiB**: istem ürün tarafından üretilir; sınırsız metin
  argv boyutunu, alt süreç davranışını ve denetim kaydını belirsizleştirir.
- `validate_prompt` — dört kapı: **boş** olamaz (boş çağrı abonelikten kota
  harcar), **`-` ile başlayamaz** (bayrak sanılır), kontrol karakteri taşıyamaz
  (NUL dâhil; `\n\r\t` serbest), sınırı aşamaz. Hata `AdapterError::InvalidPrompt`
  → **`E_CLI_ARGUMENT`** (argüman kapısı ailesiyle aynı kod).
- `prompt_argv(adapter, prompt)` — biçimin **tek** kaynağı: `antigravity-cli`
  `--print=<istem>` (tek belirteç), `claude-code` `--print <istem>` (boolean
  bayrak + ayrı belirteç), `codex-cli` `exec <istem>` (konumsal), `kimi-cli`
  `E_CLI_BATCH_FORBIDDEN`. Bilinmeyen adaptörde istem yolu **tanımlı değil**.
  Dönen argv `build_args` kapısından geçmez: istem **konumsaldır**, bayrak değil;
  bu yüzden istem yalnız burada, doğrulandıktan sonra argv'ye girer — arayüzden
  istem metni de bayrak da geçirilemez.
- `PromptShape` + `prompt_shape(adapter)` — arayüzün okuduğu biçim. `printable`
  yalnız **toplu yürütme açık** *ve* **araçsız kip doğrulanmış** (`tool_free`
  `Declared`) ise `true`; değilse `reason` = `batch_forbidden` /
  `tool_free_unverified` ve `tool_free` durumu (`verified` / `no_path` /
  `missing`). "Sandbox var" **varsayılmaz**.
- `ensure_prompt_shape(adapter, args)` — `run_cli` içinde, süreç başlamadan:
  çıplak `--print` + yanında `-` ile başlayan belirteç → `AdapterError::PromptShape`
  → **`E_CLI_ARGUMENT`**. Kapı **adaptör-farkındadır**: yalnız `--print`i değer
  olarak alan adaptör (bugün Antigravity) denetlenir; Claude'un boolean `--print`i
  yanındaki bayrağı normal ayrıştırdığı için etkilenmez (ilk sürüm bunu
  ayırmıyordu ve **10 mevcut testi kırmızıya düşürdü** — ölçüldü, kapı
  adaptör-farkında yapıldı).

**Arayüz:** `cli_capability_matrix` yanıtındaki adaptör görünümüne `prompt` alanı
eklendi (`CliPromptShapeView`: `argv`, `printable`, `reason`, `toolFree`) —
**çekirdekten** okunur, arayüzde yeniden üretilmez. Providers kartında adaptör
başına satır: “İstem biçimi `--print=<istem>` · araçsız kip doğrulandı, istem
gönderimi açık.” ya da “… gönderim kapalı: <neden>”. 5 sözlük anahtarı.

**Doğrulama (gerçek koşular):**

- `cargo test -p dim-cli-adapters --lib` → **125 passed · 0 failed** (7 yeni
  test: dört biçim, kimi kapalı, `-` ile başlayan istem, boş/kontrol/aşırı uzun
  istem, çıplak `--print` kapısı, sertleştirme biçimi bozmuyor, araçsız kip
  kapısı, şekil↔builder eşitliği).
- Masaüstü (Tauri) `cargo check` **temiz** (28,21 s) — `commands.rs` değişikliği
  derlenerek doğrulandı; `cli-adapters` + masaüstü `clippy --all-targets
  -D warnings` **temiz**; iki manifest `cargo fmt --check` **temiz**.
- `tsc --noEmit` **temiz**; hedefli vitest **139 test** yeşil
  (`providers.test.ts` 22, `capability.test.ts` 8, `i18n-bindings.test.ts` 54,
  `Providers.test.tsx` 55).
- `cargo test -p dim-cli-adapters --all-targets` → **125 lib + 5 entegrasyon**
  testi yeşil (1 gerçek-CLI testi yoksayılı — kullanıcı aboneliği, elle).
- **Dürüst not (ilk CI koşusu kırmızıydı):** `43108471774` koşusunda iki `rust`
  işi `tool_free_proof::antigravity_hardening_reaches_the_process` testinde
  düştü. Test **eski (hatalı) biçimi** kuruyordu — çıplak `--print` + sertleştirme
  bayrakları — ve yalnız argv metnine bakıyordu, vendor davranışına değil; yani
  kusuru **kanıtlamıyor**, aksine kusurlu biçimi sabitliyordu. Test doğru
  sözleşmeye çekildi (`--print=<istem>`) ve kapı için **yeni entegrasyon testi**
  eklendi (çıplak `--print` → `E_CLI_ARGUMENT`, süreç **hiç başlamaz**).
- **CI (ölçüldü):** kod `d3267cf` → run `43108471774` **kırmızı** (iki `rust`
  işi, yukarıdaki entegrasyon testi) → test + belge `e27e1e8` → run `43109504236`
  **10/10 YEŞİL**: `dim-storage` **544** hem `macos-15` (253,90 s) hem
  `ubuntu-latest` (377,60 s) — **birebir**; `dim-cli-adapters` **125** ve
  `tool_free_proof` **5 passed · 1 ignored** iki OS'ta; `arayüz` **66 dosya /
  1379 test**, paket `index-zWkVRsW5.js` **677,58 kB** / gzip **181,10 kB**;
  `masaüstü kabuk`, `windows güvenli depo`, `sözleşmeler`, `kaynak arşivi`,
  `paket manifesti`, `belge worker'ı`, `SBOM` yeşil.

**Dürüst sınırlar (T08):**

- (a) `prompt_argv` **üretimde henüz çağrılmıyor** — istemi sağlayıcıya gönderen
  uçtan uca akış (T06–T08) yok; bu tur **biçimi ve kapıyı** sözleşmeye bağladı,
  sağlayıcıya istem göndermedi.
- (b) `printable` **bildirilen** kanıta bakar (yetenek matrisi `--help` çıktısı),
  davranış ölçümüne değil; Antigravity `--sandbox` dosya okumayı engellemediği
  için (T12'de ölçüldü) `tool_free` bugün `no_path` kalır → istem gönderimi
  **kapalı** görünür.
- (c) Kapı yalnız **çıplak** `--print`i yakalar; `--print=` biçiminde istem zaten
  tek belirteçtir (ayrı ayrıştığı ölçüldü).
- (d) İstem metni **denetim kaydına yazılmaz**; kayıt yalnız biçim/karar taşır.
- (e) Uçtan uca gerçek CLI koşusu bu turda **yapılmadı** (gönderen yol yok);
  arayüz jsdom ile ölçüldü → gerçek Tauri penceresi + **görsel QA insan
  bekliyor**.

## Sertleştirme davranış sınaması — istem gerçekten gönderilir (F10-T08b, 2026-11-12)

**Ölçülen boşluk:** ürün istemi **hiç göndermiyordu**; "araçsız kip" yalnız
`--help` beyanına dayanıyordu (aşağıdaki "Dürüst durum": *yeteneğin kanıtı yardım
metnidir, davranış değil*). Kapı (`prompt_argv`) vardı ama **çağrılmıyordu**.

**Yazılan (çekirdek):**

- `run_cli_with_prompt` + `splice_prompt`: istem adaptörün **kendi biçiminde**
  yerleşir, sertleştirme bayrakları korunur — `agy`: `--print=<istem>` (tek
  belirteç), `claude`: `--print <istem>`, `codex`: `exec … <istem>` (alt komut
  **ilk** belirteç). Çağıranın bıraktığı `--print`/`exec` yinelenmez; istem yine
  `validate_prompt`dan geçer ve `ensure_prompt_shape` son argv'yi yeniden denetler.
- `hardening.rs`: kum havuzu köküne `KANIT.txt` (`DIM-CANARY-50`, 90 bayt)
  yazılır, CLI'nin iş dizini ayrı ve **boş** kalır (`work/`). Ürünün **sabit**
  istemi dosyayı okuyup içeriğini yazmasını, okuyamıyorsa `ARACSIZ` yazmasını
  ister. Sınıflama: yanıtta kanıt belirteci → `granted` (**kesin kanıt**: belirteç
  istemde geçmez, yalnız dosyada), yalnız **kısa** ret yanıtı → `blocked`, başka
  her şey → `inconclusive`. Uzun yanıt (istemin yankısı) ret sayılmaz.
- Komut `cli_hardening_probe` (tek CLI kilidi, iptal, denetim) + `Providers`
  kartında **"Sertleştirmeyi sına"**: ölçüm cümlesi, çalışan argv (istem yerine
  `<istem>`), modelin yanıtı ve hata akışı. Denetime **metin yazılmaz** (yanıtın
  karakter sayısı ve ölçüm yazılır).

**Neden kullanıcı oturumu (ölçüldü):** sınama önce kum havuzu `HOME`'i ile
denendi — claude 2.1.303, 2026-11-12:

```
$ claude --no-session-persistence --tools "" --strict-mcp-config --print "<istem>"
Not logged in · Please run /login        (Exited(1), 1810 ms)
```

İzole `HOME`'da CLI oturum açmıyor, yani istem **hiç gönderilemiyor** ve ölçüm
anlamsız kalıyor. Sınama bu yüzden `HomeIsolation::UserHome` ile koşar: kimlik
dosyasını **ürün okumaz** (ADR-05), CLI kendi oturumunu kullanır. Bu aynı zamanda
doğru soruyu sorar: *sertleştirme, kullanıcının gerçek yapılandırması altında
dosya okumayı engelliyor mu?*

**Gerçek ölçüm (bu makine, macOS aarch64, 2026-11-12):**

| Adaptör | argv (istem yerine `<istem>`) | Durum | Süre | Ölçüm | Yanıt |
|---|---|---|---|---|---|
| Claude Code 2.1.303 | `--no-session-persistence --tools "" --strict-mcp-config --print <istem>` | `Exited(0)` | 4,5 s | **blocked** | `ARACSIZ` |
| Codex CLI 0.159.2 | `exec --skip-git-repo-check --ignore-user-config --ignore-rules <istem>` | `Exited(0)` | 13,6 s | **granted** | `DIM-CANARY-50` |
| Antigravity | ürün `antigravity` arıyor; kurulu ikili **`agy` 1.5.6** | koşulamadı | — | — | — |

**Yorum (dürüst):** Codex'te `--ignore-user-config --ignore-rules` **dosya
okumayı engellemiyor**: model kum havuzundaki kanıt dosyasını okudu ve içeriğini
yazdı. Bu, yetenek matrisinin `ToolFree = NoPath` beyanını **davranışla**
doğrular — Codex'te istem gönderimi bugün de kapalı kalmalı. Claude'da
`--tools ""` altında model dosyayı okumadı (`ARACSIZ`).

**Dürüst sınırlar (T08b):**

- (a) `blocked` bir **çıkarımdır**: model reddetti diye "dosya okuma yolu yoktur"
  denemez. Bir koşuda hata akışında `[cyber]` sınıflandırıcı bildirimi de
  görüldü → "araç kapalı" ile "model reddetti" ayrımı **tek istemle
  kanıtlanamaz**. İki yönlü kanıt (araç izinli pozitif kontrol) ürün argv'sinde
  **mümkün değil** (`--allowedTools` izinli argüman kapısında yok); o kanıt
  `tool_free_proof.rs`teki elle koşulan `#[ignore]` testinde kalır.
- (b) **Antigravity üründe koşulamadı:** `lib.rs` `command: "antigravity"` arıyor,
  kurulu ikili `agy` (1.5.6). Bu turda **düzeltilmedi** (ayrı iş: komut adı / eş
  ad çözümü) — açık kayıt.
- (c) Sınama **kota harcar** (gerçek model çağrısı); yalnız kullanıcı eylemiyle
  koşar, otomatik zamanlama yok.
- (d) `HOME` kullanıcı ev dizini olduğundan CLI kullanıcı yapılandırmasını
  **görür**; ölçülen şey sertleştirme bayraklarının o yapılandırma altında ne
  yaptığıdır. Gönderilen istem ürünündür, **kullanıcı belgesi gönderilmez**.
- (e) Denetim yalnız kasa açıkken yazılır; yanıt metni denetime **girmez**.
- (f) Gerçek Tauri penceresi + görsel QA **insan bekliyor**.

**CI (ölçüldü):** kod `7571636` (12 dosya, **+1258/−5**) → run `43177918501`
**10/10 YEŞİL** (ilk koşu): `dim-storage` **544** hem `macos-15` (243,48 s) hem
`ubuntu-latest` (643,46 s) — **birebir**; `dim-cli-adapters` **138** iki OS'ta;
`arayüz` **66 dosya / 1388 test**, paket `index-C5Svx2rU.js` **681,11 kB** /
gzip **181,89 kB**; `masaüstü kabuk` 5 passed · 40 filtered; `windows güvenli
depo`, `sözleşmeler`, `kaynak arşivi`, `paket manifesti`, `belge worker'ı`,
`SBOM` yeşil. Yerel 1396 test farkı ağaçtaki başka ajanın commit'siz testleri.

## Komut eş adı çözümü — kurulu ikili `agy` (F10-T08c, 2026-11-12)

**Ölçülen boşluk:** sicil Antigravity'yi `command: "antigravity"` ile arıyordu;
bu makinede kurulu ikili **`agy` 1.5.6** (`/opt/homebrew/bin/agy`). Sonuç: ürün
"kurulu değil" diyordu, yetenek matrisi 4/4 `Unread` kalıyordu ve sertleştirme
davranış sınaması **hiç koşulamıyordu** (T08b'nin açık kaydı). Kusur yalnız
arayüz metni değildi: `discover`, `provider_probe_run`, `cli_hardening_probe`,
`cli_arch_report`, `cli_capability_matrix`, `cli_attack_probe`,
`cli_residue_measure` — hepsi `find_on_path(<bildirilen ad>)` çağırıyordu.

**Yazılan:**

- `AdapterSpec.aliases` + `command_candidates()`: **bildirilen ad önce**, sonra
  eş adlar (`antigravity-cli` → `["antigravity", "agy"]`). Diğer üç adaptörde
  liste **boş**: eş ad adaptöre özgüdür, başka adaptöre sızmaz.
- `resolve_adapter_executable(spec)` (PATH) + test edilebilir çekirdeği
  `resolve_in_dirs(spec, dirs)`: hangi adın bulunduğunu da döndürür. Ad kapısı:
  boş ya da yol ayırıcı taşıyan ad dosya adı olarak **aranmaz** (`../agy` üst
  dizine çözülmez); Windows'ta `.exe/.cmd/.bat` denenir.
- `discover` ve altı Tauri komutu aynı çözümü kullanır (`find_on_path(spec.command)`
  çağrısı kalmadı). Arayüz **bulunan adı** yazar: adaptör kartında
  `antigravity · eş ad: agy`, yetenek matrisinde kopyalanabilir komut
  `agy --help`. Her iki alan da **çekirdekten** okunur (`resolvedCommand`);
  arayüz çözüm sırasını yeniden üretmez.

**Gerçek ölçüm (bu makine, macOS aarch64, 2026-11-12):**

| Adım | Sonuç |
|---|---|
| `--example probe -- antigravity-cli` | `/opt/homebrew/bin/agy` · `Exited(0)` · 5.348 ms · çıktı `1.5.6` (önce: "PATH üzerinde bulunamadı") |
| `--example caps -- antigravity-cli` | sürüm **1.5.6**, yardım 110 ms, bağlantı **sınırlı**, kapı kapalı (4/4 `NoPath`) |
| `--example hardening -- antigravity-cli` | `--sandbox --disable-slash-commands --print=<istem>` · `Exited(0)` · 15.771 ms · **`inconclusive`** (yanıt metni boş) |

Yani ürün artık "kurulu değil" demiyor; araç gerçekten koşuyor. Antigravity'nin
sertleştirme davranışı bu koşuda **ölçülemedi**: boş yanıt `granted`/`blocked`
hükmü vermiyor. Yardım metni `--sandbox` + `--disable-slash-commands` bildiriyor
ama `tool_free` hâlâ `NoPath` — kanıt yok, iddia da yok.

**Dürüst sınırlar (T08c):**

- (a) **`inconclusive` hüküm değildir:** boş yanıt modelin reddini de, oturum/
  kotayı da, çıktı biçimini de gösterebilir; üçünü tek koşuyla ayıramayız.
- (b) Eş ad listesi **elle** tutulur ve kapalıdır (bugün yalnız `agy`). Yeni bir
  dağıtım başka ad kullanırsa ürün yine "kurulu değil" der — keşif otomatik
  değildir (bilinmeyen adı tahmin etmek yanlış araca bağlanma riski taşır).
- (c) Eş adla bulunan yürütülebilirin sürümü yine `--version` ile okunur; sürüm
  aralığının doğrulanması F10 uyumluluk sicilindedir.
- (d) Windows'ta yalnız uzantı denemesi kodda; eş ad gerçek Windows kurulumunda
  **denenmedi**.
- (e) Gerçek Tauri penceresi + görsel QA **insan bekliyor**.

**Kapı kanıtı (gerçek koşular; betik
`~/.hermes/cache/scratch/dim-cli-alias-gates.py`, her mutasyon tek başına,
`try/finally` + sha256 birebir — `lib.rs 2b370e27…`, `Providers.tsx c5c98800…`,
`providers.ts ca9a6cb3…`):** **8/8 KIRMIZI** — S1 eş ad sırası ters (eş ad önce) ·
S2 eş ad hiç denenmez · S3 eş ad listesi bütün adaptörlere sızar · S4 ad kapısı
yol ayırıcıyı kabul eder; U1 arayüz eş adı hiç yazmaz · U2 eşitlik kapısı yok
(aynı ad da yazılır) · U3 arayüz çekirdeğin alanını yok sayar · U4 yardım komutu
eş adı yok sayar.

**Testler:** `cargo test -p dim-cli-adapters --all-targets` → **142** lib (+4) +
**5** entegrasyon (1 gerçek-CLI `#[ignore]`); `providers.test.ts` 26 → **29**,
`capability.test.ts` 7 → **8**, `Providers.test.tsx` 60 → **61** → tam vitest
**66 dosya / 1400 test**; `tsc --noEmit` temiz; iki manifest `cargo fmt --check`
temiz; cli-adapters + masaüstü `clippy --all-targets -D warnings` temiz.

**CI (ölçüldü):** kod `78b442d` (15 dosya, **+298/−57**) → run `43190756305`
**KIRMIZI** — yalnız `cargo fmt --all -- --check` iki `rust` işinde: yeni test
satırları 100 sütunu aşıyordu (yerel `cargo fmt` bu satırlar eklenmeden önce
koşmuştu). Biçim düzeltmesi `c388d24` → run `43191676475` **10/10 YEŞİL**
(`dim-cli-adapters` **142** iki OS'ta; `tool_free_proof` 5 passed · 1 ignored;
`dim-storage` **544** hem `macos-15` 257,32 s hem `ubuntu-latest` 374,24 s —
**birebir**; `arayüz` **66 dosya / 1392 test**, paket `index-BD210ieX.js`
**681,31 kB** / gzip **181,97 kB**; `masaüstü kabuk` 40 koştu · 0 düştü ·
5 yoksayıldı + yoksayılan koşu 5 passed; diğer altı iş yeşil). Yerel 1400 test
farkı ağaçtaki başka ajanın commit'siz testleri.

## Ölçülen araç erişimi kapıya girer ve kasa içinde saklanır (F10-T08d, 2026-11-12)

**Ölçülen boşluk (iki parçalı):**

1. Sertleştirme sınaması gerçek istemi gönderip araç erişimini ölçüyordu ama
   sonuç **yalnız ekran durumunda** kalıyordu: paneli kapatıp açmak ya da
   uygulamayı yeniden başlatmak ölçümü siliyor, yetenek matrisi yine salt
   `--help` beyanına dönüyordu. Kapı bir **güvenlik kapısıdır**; kanıtı
   arayüz belleğinde tutulamaz.
2. Aynı kartta **çelişki** görünüyordu: matris "istem gönderimi açık" derken
   altındaki ölçüm "araç erişimi var" diyebiliyordu. Beyan ile ölçüm ayrı
   yerlerde duruyor, ölçüm kapıya hiç girmiyordu.

**Yazılan (çekirdek):**

- `capability::MeasuredAccess` (`granted` / `blocked` / `inconclusive` /
  `unreadable`) ve yeni `CapabilityState::Contradicted`
  ("ölçüldü · araç erişimi var"). `evaluate_with_evidence(...)` ölçümü **yalnız
  `ToolFree` satırına** ve **yalnız kurulu** adaptöre uygular:
  - `granted` → satır `contradicted`, bağlantı **uyumsuz**, yetenek kapısı
    kapanır, gerekçe `tool_free:contradicted`;
  - `unreadable` → satır `unread` (temkinli taraf; bozuk kayıt "kayıt yok"
    sayılıp kapıyı açmaz);
  - `blocked` / `inconclusive` → beyan **değişmez**, yalnız `measured`
    alanında görünür. Kural tek yönlüdür: **ölçüm kapıyı açmaz, yalnız
    kapatır.** `blocked` "bu istemde araç çağrısı görülmedi" demektir;
    "araçsız kip var" kanıtı değildir.
- `exec::prompt_shape_with_evidence` aynı kaydı istem kapısına geçirir:
  `tool_access_granted` / `evidence_unreadable` nedenleri ve
  `PromptShape.evidence` alanı. `prompt_shape` (kayıtsız) davranışı korunur.

**Yazılan (masaüstü):**

- Ölçüm `settings` tablosunun **device** kapsamına yazılır
  (`cli.evidence.<adapter>`); değer yalnız `access` + `answered` +
  `measuredAtUnix` taşır — **argv, yanıt metni ve dosya yolu yazılmaz**.
- `cli_hardening_probe` kasa kilitliyken **başlamaz** (`E_VAULT_LOCKED`):
  kaydedilemeyecek bir koşu için kota harcanmaz. Yazma düşerse ölçüm yine
  gerçektir ama rapor "kaydedildi" demez; `evidence_recorded=false` + not
  düşer ve istem kapısı **beyana** döner.
- `cli_capability_matrix` kayıtları bir kez okur, `evaluate_with_evidence` ve
  `prompt_shape_with_evidence`e geçirir; satırda `measured`, adaptörde
  `evidence` + `evidenceAtUnix`, istem görünümünde `evidence` alanı. Kasa
  kilitliyse rapor bunu **not** olarak yazar (sessizleşmez).

**Yazılan (arayüz):** kartta "Son sertleştirme sınaması: … · tarih" satırı,
çürütülen satır kırmızı rozetle ve nedeniyle, istem kapısı ölçüm nedeniyle
kapalı ve kırmızı; sınama panelinde "İstem gönderimi kapatıldı…" cümlesi
(yalnız **kayıt yazıldıysa**; yazılamadıysa neden yazılır, "kapandı" denmez).

**Dürüst sınırlar (T08d):**

- (a) Kayıt **kasa içindedir**: kasa kilitliyken okunamaz → matris o an yalnız
  beyana dayanır ve bunu not olarak yazar. Kasa oluşturulmadan sınama
  koşulamaz (komut `E_VAULT_LOCKED` verir) — ürün ekranı zaten kilitliyken
  açılmaz.
- (b) Kayıt **cihaz** kapsamındadır, kasa verisine karışmaz; yeni bir kasada
  ölçüm geçmişi yoktur (yeniden sınama gerekir).
- (c) Ölçüm **yalnız kapatır**: `blocked` bir beyanı doğrulamaz. "Araçsız kip
  çalışıyor" iddiası hâlâ `--help` beyanına dayanır ve öyle etiketlenir.
- (d) Tek ölçüm tek istemdir: aynı adaptörün farklı istemlerde farklı
  davranması bu kapıyla yakalanmaz; son kayıt geçerlidir.
- (e) Komut yolu (`cli_hardening_probe`, `cli_capability_matrix`) **komut
  olarak koşulmadı** (`State<AppState>` altyapısı yok) — kablolama
  `cargo check` + `clippy`, davranış storage düzeyinde gerçek SQLite
  (`open_in_memory`) ile ölçüldü.
- (f) Gerçek Tauri penceresi + görsel QA **insan bekliyor**; jsdom tarayıcı
  değildir.

**Kapı kanıtı (gerçek koşular; betik
`~/.hermes/cache/scratch/dim-evidence-gates.py`, her mutasyon tek başına,
`try/finally` + sha256 birebir):** **14/14 KIRMIZI** — S1 ölçüm `granted`
satırı çürütmez · S2 `contradicted` bağlantıyı uyumsuza çekmez · S3 `blocked`
çürütme sayılır (aşırı iddia) · S4 `unreadable` yok sayılır · S5 kurulu
olmayan adaptörde ölçüm uygulanır · S6 ölçüm bütün yeteneklere sızar · S7
istem kapısı `granted`ı yok sayar · S8 `unreadable` kapıyı kapatmaz · S9
bozuk kayıt `blocked` okunur (kapı açılır); U1 ölçüm nedeni sarı yazılır · U2
saklanan ölçüm satırı çizilmez · U3 kayıt yokken kapı cümlesi yazılır · U4
kartta ölçüm satırı render edilmez · U5 `contradicted` rozeti yeşil/renksiz.

**Testler:** `cargo test -p dim-cli-adapters --all-targets` → **147** lib
(+5) + **5** entegrasyon (1 gerçek-CLI `#[ignore]`); masaüstü test kapısı
**40 → 44** koşan (49 testin 5'i `#[ignore]`); `providers.test.ts` 29 → **34**,
`capability.test.ts` 8 (bir ton iddiası eklendi), `Providers.test.tsx`
61 → **64** → tam vitest **66 dosya / 1408 test**; `tsc --noEmit` temiz; iki
manifest `cargo fmt --check` temiz; cli-adapters + masaüstü
`clippy --all-targets -D warnings` temiz.

**CI (ölçüldü):** kod `bc501c5` (14 dosya, **+966/−58**) → run `43206873718`
**10/10 YEŞİL** (ilk koşu): `dim-cli-adapters` **147** iki OS'ta (5,53 s /
5,21 s) + `tool_free_proof` **5 passed · 1 ignored**; `dim-storage` **544** hem
`macos-15` 286,52 s hem `ubuntu-latest` 413,51 s — **birebir**; `arayüz`
**66 dosya / 1400 test** (yerel 1408 farkı ağaçtaki başka ajanın commit'siz
testleri), paket `index-BB8XRDhS.js` **683,76 kB** / gzip **182,57 kB**;
`masaüstü kabuk` **44 koştu · 0 düştü · 5 yoksayıldı** (kaynak 49) + yoksayılan
koşu 5 passed · 44 filtered; diğer beş iş (`windows güvenli depo`, `kaynak
arşivi`, `paket manifesti`, `belge worker'ı`, `SBOM`) yeşil.

## Sınama koşulamadıysa ölçüm yazılmaz, nedeni söylenir (F10-T08e, 2026-11-12)

**Ölçülen kusur (bu makine, `agy` 1.5.6):** Antigravity'nin kotası doluyken
sertleştirme sınaması `Exited(3)` + **boş** stdout veriyordu ve ürün bunu
`inconclusive` diye **kaydediyordu**; yetenek matrisi "ölçüldü · hüküm yok"
yazıyordu. Oysa sınama **koşmamıştı**: gerçek neden stderr'de
`RESOURCE_EXHAUSTED (code 429)` idi ve kullanıcı hiç görmüyordu. "Ölçüldü"
cümlesi gerçekten koşulmuş bir ölçümü anlatır; koşmayan sınama onu yazamaz —
bu, "sahte yeşil"in kapı tarafındaki karşılığıdır.

**Tur başındaki ölçüm (öncesi):**

```text
== antigravity-cli (1.5.6) ==
argv     : --sandbox --disable-slash-commands --print=<istem>
durum    : Exited(3) · 4714 ms · kanıt 90 bayt
ölçüm    : inconclusive
işaret   : kanıt=false ret=false yanıt=false
yanıt    :            ← boş; neden görünmüyor
```

**Yazılan (çekirdek, `hardening.rs`):**

- `NotRun` — `quota` / `auth` / `process_failed` / `timeout` / `cancelled` —
  ve `classify_not_run(status, stdout, stderr)`: yanıt **gelmediğinde**
  nedeni sınıflar. Sıra önemli: kota ve kimlik işaretleri **süreç kodundan
  önce** bakılır, çünkü CLI'lar bu hataları sıfır dışı kodla bildirir ve
  "süreç düştü" demek gerçek nedeni gizlerdi. `429`/`401`/`403` **sayı
  belirteci** olarak aranır (`1429` eşleşmez); süre ve iptal ayrı sınıftır.
- agy için sınama argv'sine `--output-format=json` eklenir. Ölçüldü: hata
  **stdout'a makine okunur** yazılıyor —
  `{"status":"ERROR","response":"","error":"Individual quota reached …"}`.
  `PrintEnvelope` modelin görünen yanıtını `response` alanından okur; JSON
  gövdesi yanıt sayılmaz. Çıktı kırpıldıysa zarf **hiç** ayrıştırılmaz (yarım
  JSON'dan yanıt uydurulmaz — codex NDJSON kuralının aynısı). Değer **tek
  belirteçte** verilir (`--output-format=json`): ayrı `json` belirteci
  `build_args` kapısından düşerdi (izinli olan *bayrak*, değer değil).
- `HardeningReport::measured_access()` — sınama koşmadıysa `None`. Kapıya
  girecek ölçüm yoktur; kural **tek yerde** durur.
- `not_run_reason` — CLI'nin kendi cümlesi, tek satır, ≤200 karakter, kum
  havuzu yolu `<kum>` ile gizlenir (rapor kullanıcıya gösterilir).

**Yazılan (masaüstü):** kanıt **yalnız ölçüm varsa** kasa içine yazılır; not
"koşulamadı (`quota`); ölçüm yazılmadı ve istem kapısı beyana dayanır" der
(eskiden "yanıt alınamadı (süre, kimlik ya da çıkış kodu)" gibi genel bir
cümleydi). Görünüm `notRun { kind, reason }` taşır; **denetime yalnız sınıf**
girer — CLI'nin cümlesi ve yanıt metni denetime yazılmaz.

**Yazılan (arayüz):** `probe.hardeningNotRun` cümlesi + `notRunLabel`
(tanınmayan sınıf **ham** yazılır, uydurma etiket yok) + panelde "Neden: …"
satırı. Kanıt yazılmadığı için kapı cümlesi ("İstem gönderimi kapatıldı/açık
kalıyor") **çizilmez** — ölçüm yoktur.

**Ölçüm (sonrası, aynı makine, `--example hardening`):**

```text
== antigravity-cli (1.5.6) ==
argv     : --output-format=json --sandbox --disable-slash-commands --print=<istem>
durum    : Exited(3) · 10895 ms · kanıt 90 bayt
ölçüm    : yok — sınama koşulamadı
koşmadı  : quota · Individual quota reached. Please upgrade your subscription
           to increase your limits. Resets in 101h22m28s.
işaret   : kanıt=false ret=false yanıt=false
```

Claude Code 2.1.303 değişmedi: `blocked`, `ARACSIZ`, `Exited(0)` 4896 ms.

**Kapı kanıtı (gerçek koşular; betik `~/.hermes/cache/scratch/dim-notrun-gates.py`,
her mutasyon tek başına, `try/finally` + sha256 birebir — `hardening.rs
9c02876e…`, `providers.ts edcb8772…`, `Providers.tsx 1b02713a…`):** **11/11
KIRMIZI** — S1 kota sınıfı `process_failed`e düşer · S2 `measured_access`
koşmayan sınamayı yine ölçüm sayar · S3 zarf yok sayılır (JSON gövdesi yanıt
sanılır) · S4 yapısal çıktı istenmez · S5 kimlik sınıfı kaybolur · S6 durum
kodu alt dizge olarak aranır (`1429` → `429`) · S7 `answered` kapısı kalkar
(stderr gürültüsü gerçek ölçümü düşürür) · S8 gerekçe rapora bağlanmaz; U1
`notRun` dalı yok sayılır · U2 tanınmayan sınıf boş etikete düşer · U3
panelde gerekçe satırı çizilmez. **Dürüst (kendi hatam):** ilk koşuda üç
arayüz kapısı `ÖLÇÜLEMEDİ` çıktı — betiğim özet satırını `Tests` ile
**başlarken** arıyordu, vitest onu girintili yazıyor; ayrıştırıcı düzeltildi,
**11/11 yeniden ölçüldü**, ilk koşunun 8'i kanıt sayılmadı.

**Testler:** `cargo test -p dim-cli-adapters --all-targets` → **155** lib
(147 → +8) + **5** entegrasyon (1 gerçek-CLI `#[ignore]`); `providers.test.ts`
34 → **36**, `Providers.test.tsx` 64 → **65** → tam vitest **66 dosya / 1411
test**; masaüstü test kapısı **44 koştu · 0 düştü · 5 yoksayıldı** (taban
değişmedi); `tsc --noEmit` temiz; iki manifest `cargo fmt --check` temiz;
cli-adapters + masaüstü `clippy --all-targets -D warnings` temiz.

**Dürüst sınırlar (T08e):**

- (a) `classify_not_run` bir **işaret** sınıflamasıdır: yanıt gelmediğinde
  stderr/stdout'ta "quota"/"429" geçmesi kota sayılır. Yanlış pozitif olasıdır;
  sonucu **temkinli** taraftadır — kanıt yazılmaz, kapı beyana döner, yani
  kapıyı yanlış **açamaz**, ama gerçek bir ölçüm yanlış sınıfla etiketlenebilir.
- (b) Gerekçe metni CLI'nin **kendi** cümlesidir ("Resets in 101h…" gibi süre
  bilgisi ürün tarafından doğrulanmaz); ürün yalnız sınıfı iddia eder.
- (c) Yapısal çıktı **yalnız agy** için istendi (ölçüldü). Claude/Codex metin
  kipinde kalır; onların zarf biçimleri bu turda ölçülmedi.
- (d) `answered` = çıkış 0 **ve** boş olmayan yanıt. Yanıt verip sıfır dışı
  kodla çıkan CLI "koşulamadı" sayılır (böyle bir ölçüm henüz görülmedi).
- (e) Komut yolu (`cli_hardening_probe`) yine **komut olarak koşulmadı**
  (`State<AppState>` altyapısı yok): kablolama `cargo check` + `clippy`,
  davranış çekirdek düzeyinde gerçek CLI ile (`--example hardening`) ölçüldü.
- (f) Gerçek Tauri penceresi + görsel QA **insan bekliyor**; jsdom tarayıcı
  değildir.

## Başlatma yarışı — yalnız `ETXTBSY` yeniden denenir (F10-T14 ek turu, 2026-11-04)

**Neden bu dilim:** `41764725430` (yalnız belge değiştiren commit) `rust
(ubuntu-latest)` işinde `tool_free_proof::
control_run_without_hardening_shows_the_probe_is_not_blind` ile kırmızıya
düştü: `Os { code: 26, kind: ExecutableFileBusy, "Text file busy" }`. Ürün yolu
(`run_cli` → `spawn_with_retry`) bu hatayı yeniden deniyordu; **kontrol koşusu**
ham `Command::new` kullanıyordu ve hiç yeniden denemiyordu. Sonuç: kod
değişmemiş olsa da CI, yeni yazılmış betiğin yürütülmesine bağlı olarak
rastgele kırmızıya düşebiliyordu.

**Ölçülen kusur (ürün):** `spawn_with_retry` **her** spawn hatasını 6 kez (50 ms
arayla) deniyordu → kalıcı hatalar (`NotFound`, `PermissionDenied`, biçim
bozuk) sonucu değiştirmeyen **250 ms** bekleme ve 6 sistem çağrısıyla dönüyordu.

**Düzeltme (`exec.rs` +`lib.rs` dışa aktarım, `tool_free_proof.rs`):**
- `retry_while_busy` (yeni): `attempt` yalnız `ErrorKind::ExecutableFileBusy`
  hatasında yeniden denenir; **ilk kalıcı hata hemen döner**; son denemede de
  meşgulse **son** hata korunur.
- `spawn_with_retry` artık bu yardımcıyı kullanır; `SPAWN_ATTEMPTS = 6`,
  `SPAWN_RETRY_DELAY = 50 ms` (en kötü durum 250 ms, süre sınırından düşülür).
- Kontrol koşusu `control_output()` üzerinden **aynı** politikayı kullanır;
  ürün zarfını yine atlar (ADR-05: sertleştirmenin etkisini ölçmek için ham
  çağrı gerekir).

**Testler:**
- `exec.rs` / `spawn_retry_tests` (3): kalıcı hata **tek** denemede döner;
  meşgul hata başarıya kadar denenir (3. denemede döner); son denemede durur ve
  **son** hata metnini korur (deneme sayısı = `SPAWN_ATTEMPTS`).
- `tool_free_proof.rs` / Linux'a özgü üç test (`cfg(target_os = "linux")`):
  (1) **üretim gerçek mi** — yazma tanıtıcısı açıkken ham `spawn` gerçekten
  `ETXTBSY` (errno 26) alır; (2) kontrol yolu tanıtıcı bırakılınca yeniden
  denemeyle başarır; (3) ürün yolu (`run_cli`) aynı pencereyi aşar.
  macOS bu hata sınıfını **üretmiyor** (ölçüldü: yazma tanıtıcısı açıkken
  `exec` başarılı, çıkış 0) — bu yüzden mod macOS'ta derlenmez.

**Kapı kanıtı (yerel, 5 mutasyon / 3 kırmızı):** M1 yalnız-meşgul kapısı
kaldırıldı (= eski politika) → `a_permanent_error_is_returned_after_a_single_attempt`
kırmızı; M2 `SPAWN_ATTEMPTS = 1` → meşgul testi kırmızı; M3 son hata yerine ilk
hata korunur → son-hata testi kırmızı. **Dürüst:** M4 (`spawn_with_retry`
yardımcıyı atlar) ve M5 (`control_output` yardımcıyı atlar) macOS'ta **yeşil
kaldı**; kapıları yalnız Linux testleridir, yani bu iki mutasyonun kanıtı bu
turda **ölçülmedi**. Geri alma sha256: `exec.rs 8194e2f9…93e7ba`,
`tool_free_proof.rs ed73d8e6…cedbe` (birebir).

**Doğrulama:** `cargo fmt --all --check` temiz · `cargo clippy -p
dim-cli-adapters --all-targets -- -D warnings` temiz (host **ve**
`--target x86_64-unknown-linux-gnu`) · `cargo test -p dim-cli-adapters` **117
birim geçti** + `tool_free_proof` **4 geçti / 1 yoksayıldı** (Linux'a özgü 3
test macOS'ta derlenmez). `cargo check --target x86_64-unknown-linux-gnu
--all-targets` temiz → cfg'li kod **derleniyor** (koştuğu yer Linux CI).

**Dürüst sınır:** Linux testlerinin **koşma** kanıtı bu commit'in CI
`rust (ubuntu-latest)` işidir; yerelde üretilemedi. Yeniden deneme gerçek
zamanlamayı (çerçeve/çekirdek yarışını) ölçmez, yalnız "meşgulken bekle ve
tekrar dene" politikasını sabitler.

## Gerçek CLI ile ölçüm (bu makine, macOS aarch64)

`cargo run -p dim-cli-adapters --example probe -- <adaptör>` — arayüz düğmesiyle
aynı yol:

| Adaptör | Yol | Durum | Süre | Çıktı |
|---|---|---|---|---|
| Claude Code | `~/.npm-global/bin/claude` | `Exited(0)` | 463 ms | `2.1.303 (Claude Code)` |
| Codex CLI | `~/.local/bin/codex` | `Exited(0)` | 197 ms | `codex-cli 0.159.2` |
| Kimi | `~/.local/bin/kimi` | `E_CLI_BATCH_FORBIDDEN` | — | (tasarım gereği kapalı) |
| Antigravity | `/opt/homebrew/bin/agy` | `Exited(0)` | 5.348 ms | `1.5.6` (eş ad; 2026-11-12) |

Ortam üçünde de yalnız `PATH, HOME, TMPDIR`; iş dizini boş geçici dizin.

## Dürüst durum

- **Belge/aday gönderimi yok; yalnız tanı istemi var.** T06–T08 profilleri
  (araçsız, hook/MCP kapalı, şemalı çıktı) uygulanmadığı için hassas kaynak
  aktarımı **açılmadı**. Ürünün gönderdiği tek istem, sertleştirme sınamasının
  **sabit** kanıt istemidir (T08b); düğme ayrıca sürüm yoklaması çalıştırır.
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
  Antigravity CLI bu makinede **`agy` 1.5.6** olarak kurulu; ürün eş adla bulur
  (2026-11-12, T08c). Sertleştirme sınaması koştu ama **yanıt metni boş**
  döndü — 2026-11-12 (T08e) ölçüldü: neden **kota** (`RESOURCE_EXHAUSTED 429`,
  "Resets in 101h…"); sınama **koşmadı**, ölçüm yazılmadı ve araç erişimi
  hakkında hüküm **yok**.
- Tauri penceresi açılmadı: düğme, iptal ve sonuç paneli tip denetimi + birim
  testleri + üretim derlemesiyle doğrulandı; gerçek tıklama/görsel QA yok.
- Windows'ta `exec`/`probe` testleri çalışmaz (`#[cfg(unix)]`); yürütme kodunun
  kendisi platformdan bağımsızdır ama Windows'ta doğrulanmadı.
- Denetim kaydı yalnız kasa açıkken yazılır; kilitliyken sınama çalışır (tanı
  amaçlı, veriye dokunmaz) ama denetime düşmez.

## Doğrulama

- **T08c (2026-11-12):** `cargo test -p dim-cli-adapters --all-targets` → **142**
  lib (+4: eş ad çözümü sırası, sızıntı kapısı, ad kapısı) + **5** entegrasyon
  (1 `#[ignore]`); tam vitest **66 dosya / 1400 test**; mutasyon kapıları
  **8/8 kırmızı**; ölçümler yukarıdaki T08c bölümünde.
- **T08b (2026-11-12):** `cargo test -p dim-cli-adapters --all-targets` → **138**
  lib + **5** entegrasyon (1 gerçek-CLI `#[ignore]`) — 13 yeni lib testi (istem
  sınıflaması, uzun-yankı ret kapısı, argv şablonu, sahte CLI ile uçtan uca
  istem ve kanıt okuma, Kanıt dosyası temizliği, Kimi kapısı, `splice_prompt`
  sırası/yineleme kapısı); `clippy --all-targets -D warnings` iki crate temiz;
  iki manifest `cargo fmt --check` temiz; `tsc --noEmit` temiz; tam vitest
  **66 dosya / 1396 test**; gerçek ölçüm yukarıdaki tabloda.
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
- Web: **944** test (F10-T01 turunda 15 yeni `arch.test.ts`: tonlar, evrensel rozet
  metni, mimari listesi, biçim/yol satırları, neden çevirisi, shebang satırı, özet,
  en kötü karar sırası, farklı/evrensel-uyumsuz/doğrulanamayan uyarıları, kart
  rozeti, zaman damgası; F10-T12 turunda 25 yeni `attack.test.ts`: karar tonları ve
  metinleri, kapatma yolu, komut satırı, atlama nedenleri, fikstür özeti ve
  uyarıları, adaptör uyarıları, karar sayımı/en kötü karar/toplam işaret;
  F10-T11 turunda 8 yeni `capability.test.ts`: durum/bağlantı
  tonları, özet sayıları, kapı metni, gerekçe çevirisi, uyarılar, en kötü durum,
  yoklanan komut), `tsc --noEmit` + `vite build` yeşil. F10-T13 arayüz turunda
  `Providers.test.tsx` 17 → 30 (13 yeni: uçuştaki kalıntı tarama/ölçüm/temizlik
  yanıtları adaptör kimliğine bağlandı; kilit devri ve erken kilit açma ölçüldü).
  F10-T15 turunda `Providers.test.tsx` 37 → 45 (8 yeni: çekirdeğin süreç-geneli
  CLI kilidi tek `coreBusy` bayrağına taşındı; dört kartın simetrik kapıları).
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
- **CI:** `95e6287` → run `40126453588` **7/7 yeşil** (arayüz, SBOM, masaüstü
  kabuk, rust ubuntu+macos, belge worker'ı, sözleşmeler). Önceki: `d9a5581` → run `40109851908` **7/7 yeşil** (arayüz, SBOM, masaüstü
  kabuk, rust ubuntu+macos, belge worker'ı, sözleşmeler). F10-T11: `c165063` →
  run `40114620637` **7/7 yeşil** (aynı matris). **F10-T12:** `753aed7` +
  `bba6485` (rustfmt) → run `40121007054` **7/7 yeşil** (arayüz, SBOM, masaüstü
  kabuk, rust ubuntu+macos, belge worker'ı, sözleşmeler).

## Kapsam kararı: CLI'lar aday üretmez

Resmî CLI araçları `Provider` trait'ini uygulamaz. Ürün onları yalnız
kurulum/bağlantı sınaması için izinli argv + allowlist ortam + boş geçici dizin
ile çalıştırır. Aday üretimi `ApiProvider` / `LocalProvider` hattındadır.

Gerekçe:
1. CLI oturumu kullanıcının kimliğiyle açılır; ürün kimlik dosyası okumaz (ADR-05).
2. Kullanıcı hook'ları sürece girebilir; Codex'te bu yalnız `--ignore-user-config
   --ignore-rules` ile kapatılabildi (bkz. `HOOKS.md`).
3. CLI çıktısı kararlı bir şema değildir; `--json` taşıması Codex'e özgüdür.
