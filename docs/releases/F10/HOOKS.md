# CLI yürütmede hook / kullanıcı yapılandırması izolasyonu

- **Tarih:** 2026-10-25
- **Araç:** Codex CLI `0.159.2` (kullanıcının kendi aboneliği)
- **Sonuç:** Ürün artık `codex-cli` çağrılarına `--ignore-user-config --ignore-rules`
  bayraklarını **zorunlu** ekler (`crates/cli-adapters/src/exec.rs::hardened_args`).

## Bulgu

İlk gerçek çağrıda kullanıcının kendi Codex hook'ları ürünün çalıştırdığı süreçte
devreye girdi:

```
session id: 01a17090-...
warning: loading hooks from both /Users/.../.codex/hooks.json and config.toml
hook: SessionStart
hook: SessionStart Completed
codex
DIM-CANARY-42
hook: Stop
hook: Stop Completed
```

Bu, "ürünün başlattığı işlem yalnız ürünün belirlediği talimatla çalışır" kuralını
ihlal eder: kullanıcı hook'u modele ek talimat enjekte edebilir veya çıktıyı
değiştirebilir.

## Denenen bayraklar

| Bayrak | Hook'lar kapandı mı? |
|---|---|
| `-c hooks.enabled=false` | ❌ (etkisiz) |
| `-c 'hooks={}'` | ❌ (etkisiz) |
| `--disable hooks` | ✅ |
| `--ignore-user-config --ignore-rules` | ✅ (auth korunur, çağrı çalışır) |

`--disable hooks` yalnız hook'ları kapatır; `--ignore-user-config --ignore-rules`
MCP sunucularını, proje talimatını (`AGENTS.md`) ve kullanıcı kurallarını da
devre dışı bırakır. Ürün ikincisini seçti.

## Kanıt (aynı canary, iki koşu)

```
# bayraklar olmadan
hook: SessionStart
DIM-CANARY-42
hook: Stop

# --ignore-user-config --ignore-rules ile
DIM-CANARY-46
```

Model yanıtı her iki koşuda da doğru; fark, izinsiz yan kanalların kapanması.

## Kural

- `hardened_args` **çağırandan bağımsız** çalışır: arayüz bayrağı vermeyi unutsa
  bile ürün kendi çerçevesini uygular.
- Kimlik dosyası okunmaz/kopyalanmaz; CLI kendi oturumunu kullanır, ürün yalnız
  süreç çıktısını ve çıkış kodunu görür.
- Benzer izolasyon diğer adaptörler için ayrı kanıt gerektirir; `claude-code`,
  `antigravity-cli`, `kimi-cli` bu turda **doğrulanmadı**.

## Araç yalıtımı ve oturum ortamı — ölçüldü (2026-10-27)

Araç: Claude Code **2.1.303**, Antigravity **1.4.7**, Codex **0.159.2** (kullanıcının
kendi abonelikleri). Kanıt: kum havuzu dışında bir dizine `DIM-CANARY-…` yazıldı ve
modelden **mutlak yolla** okunması istendi.

### Ölçümler

| Deney | Sonuç |
|---|---|
| Claude, araç izinli (`--allowedTools Read`), boş iş dizini | canary **okundu** (`DIM-CANARY-…`) → boş iş dizini tek başına yalıtım değil |
| Claude, `--tools ""` + aynı izin | `NO-TOOLS` → araç yüzeyi kalktı, mutlak yol okunamadı |
| Claude, `--tools=` (tek belirteç) | model araç çağrısı **üretmeye devam etti** → bu biçim kapatmıyor |
| Claude, `--bare` | `Not logged in` → kimlik yüklenmiyor, zorunlu sertleştirmeye **girmez** |
| Claude, kum havuzu `HOME` (boş dizin) | `Not logged in` → kimlikli çağrı ürün kum havuzunda yapılamaz |
| Claude, `env -i` + allowlist (`USER` yok) | `Not logged in`; **yalnız `USER` eklendiğinde** yanıt geldi |
| Claude, iş dizininde `CLAUDE.md` | proje talimatı **okundu** (yanıt `DIM-PROJECT-RULE`) → boş iş dizini şart |
| Antigravity, `--sandbox` açık | canary **okundu** → `--sandbox` dosya okumayı engellemiyor |
| Antigravity, `--sandbox` kapalı | canary **okundu** (fark yok) |
| Antigravity, `--print` yanında bayrak | `Error: --print took "--sandbox" as its prompt` → istem bayrağa bitişik ya da bayraklar ayrı konmalı |

### Ürüne yansıyan düzeltmeler

- `hardened_args` Claude için **`--tools ""`** (iki belirteç) ve `--strict-mcp-config`
  ekler; çağıran `--tools Read,Edit` veya `--tools=…` verse bile **ürünün değeri**
  geçerlidir (`strip_flag`). Antigravity'ye `--disable-slash-commands` eklendi.
- `default_env_allowlist` **`USER`** taşır. Eksikliği kimlikli CLI çağrısını tamamen
  kapatıyordu (`E_CLI_EXEC_*` değil, CLI'nin kendi "Not logged in" hatası).
- `attack::hardening_argv` eskiden `--tools=` yazıyordu; artık `--tools` + boş
  belirteç. `build_args` boş dizeyi yalnız bayrak değeri olarak kabul eder (serbest
  metin hâlâ reddedilir).
- Hermetik kanıt: `crates/cli-adapters/tests/tool_free_proof.rs` sahte CLI ile
  argv'nin sürece ulaştığını, kontrol koşusunun `TOOLS-ON` yazdığını doğrular.
  Gerçek CLI kanıtı `--ignored` (kullanıcı aboneliğiyle iki istem).

### Dürüst sınırlar

- **Kimlikli araçsız çağrı ürün sandbox'ında mümkün değil:** kum havuzu `HOME`
  kimlik taşımıyor; kullanıcı ev dizini (`HomeIsolation::UserHome`) seçilirse
  kimlik çalışır ama kullanıcı ayarları görünür olur. Bu yüzden CLI **aday üretim
  hattına bağlanmaz** (F09/F10 kararı); yalnız yoklama/yetenek/saldırı koşuları.
- Hook yalıtımı **doğrulanmış bayrakla** değil, `HOME` kum havuzu + boş iş diziniyle
  sağlanır; `--bare` kimliği düşürdüğü için kullanılamıyor.
- Antigravity'de araçsız kip **yok**; `--sandbox` yalnız terminal kısıtı bildiriyor.
  Kimi etkileşimli-only (toplu çağrı kapalı). Codex'te araç kapatma bayrağı yok.
- Ölçüm macOS aarch64 + bu sürümlerle; sürüm değişince yeniden ölçülmeli
  (`--help` yetenek matrisi bunu yakalar, davranış kanıtı yakalamaz).
