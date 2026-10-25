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
