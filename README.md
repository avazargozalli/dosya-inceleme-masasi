# Dosya İnceleme Masası

Türkiye'deki avukatlar için **yerel, şifreli ve kaynak izlenebilir** dosya inceleme masası.
Belgeler cihazdan çıkmadan okunur; her otomatik bulgu bir belge sürümüne ve kaynak konumuna
bağlanır; yapay zekâ öneri üretir, hukuki kaydı yalnız avukat onayıyla oluşturur.

> **Durum: pre-alpha (F02–F10 arası, kısmi).** Bu depo çalışan bir ürün değildir.
> Fazlar `docs/roadmap.md` içinde izlenir; bir faz ancak kabul kanıtı ve yeşil CI
> ile tamamlanmış sayılır.

## Şu an gerçekten ne çalışıyor?

| Yetenek | Durum | Nerede |
|---|---|---|
| Şifreli kasa oluşturma/açma (Argon2id + AES-256-GCM) | ✅ gerçek | `crates/vault` |
| Kurtarma anahtarı (32 bayt, base32, tek gösterim) | ✅ gerçek | `crates/vault` |
| SQLCipher veritabanı, 47 tablo, migrasyon checksum | ✅ gerçek | `crates/storage` |
| Dosya (case) oluşturma, arşiv, geri dönüşümlü silme | ✅ gerçek | `crates/storage`, Dosyalar ekranı |
| Belge içe aktarma: SHA-256, şifreli blob, sürüm | ✅ gerçek | `crates/ingest` |
| PDF metin çıkarma (PyMuPDF worker) | ✅ gerçek (motor kuruluysa) | `workers/document` |
| Tam metin arama (FTS5, dosya kapsamlı) | ✅ gerçek | Arama ekranı |
| Kronoloji: tarih türü/kesinliği, bilinmeyen tarih | ✅ gerçek | Kronoloji ekranı |
| Görevler (kullanıcı tarihli) | ✅ gerçek | Görevler ekranı |
| OCR (Tesseract) | ❌ yok | F06 devamı |
| Belge okuyucu / sayfaya atlama | ❌ yok | F07 devamı |
| AI sağlayıcı bağlantısı | ❌ yok (`OffProvider`) | F09–F11 |
| CLI adaptör yürütme | ❌ yok (yalnız sicil) | F10 |
| Maskeleme, rapor, yedek paketi | ❌ yok | F13–F14 |

“Buton var, iş yok” durumu bilinçli olarak engellenir: hazır olmayan ekranlar
`Fxx` etiketiyle “hazır değil” der, sahte veri göstermez.

## Neden

Dosya incelemesi (belge okuma, kronoloji, iddia–delil eşleştirme, duruşma hazırlığı)
avukatın en çok zaman harcadığı ve en kolay gözden kaçırdığı iştir. Mevcut araçlar ya
bulut tabandır (meslek sırrı riski) ya da kaynaksız özet üretir (denetlenemez çıktı).
Bu proje üç şeyi bir arada tutar:

1. **Yerel kasa** — SQLCipher + şifreli blob'lar; arama indeksi ve yedek dahil şifreli.
2. **Kaynak zinciri** — her alıntı `belge sürümü + sayfa/paragraf + karakter aralığı + alıntı hash'i` taşır.
3. **İnsan onayı** — AI çıktısı kalıcı kayda doğrudan yazmaz; inceleme kuyruğundan geçer.

## Mimari (kısa)

```
React (Vite)  ──IPC──▶  Rust çekirdeği (Tauri 2)
                          ├─ vault      : Argon2id + AES-256-GCM (chunked AEAD)
                          ├─ storage    : SQLCipher/SQLite + FTS5
                          ├─ jobs       : tek yazıcı kuyruk, lease, idempotency
                          ├─ evidence   : alıntı doğrulama, anchor durumları
                          ├─ policy     : dosya kapsamı, aktarım izni
                          └─ providers  : yerel model · CLI adaptörleri · API
                                 │
                          worker (Python): PyMuPDF · OCR · DOCX · redaksiyon
```

Ayrıntı: `docs/adr/` (mimari kararlar), `docs/security/threat-model.md`,
`docs/spec/DOSYA_INCELEME_MASASI_TAM_PROJE_PLANI.md` (bağlayıcı şartname).

## Geliştirme

Gereksinimler: Node 22+, pnpm 10+, Rust stable, Python 3.11+.

```bash
pnpm install
pnpm typecheck
pnpm test

# masaüstü (Rust gerekir)
pnpm --filter @dim/desktop tauri dev
```

Bu depo **lokal ağır derleme tutmaz**: Rust/Tauri derlemeleri ve paketleme GitHub Actions
üzerinde yapılır (`docs/roadmap.md` → "Neden CI-first"). Lokal ortamda yalnız kaynak kod
ve tip/lint kontrolleri çalıştırılır.

## Katkı

`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` ve `SECURITY.md` okunmalıdır.
Gerçek müvekkil belgesi, kişisel veri veya meslek sırrı içeren dosya **asla** issue,
PR veya test fixture'ı olarak eklenemez. Yalnız `fixtures/synthetic/` altındaki kurgu
veriler kullanılır.

## Geliştirme

```bash
pnpm install
bash scripts/install-hooks.sh   # commit sonrası disk koruması (core.hooksPath)
```

`scripts/disk-guard.sh` her commit sonrası arka planda çalışır: repo 2 GB'ı veya
boş disk 5 GB'ı geçerse **yalnız yeniden üretilebilir** çıktıları (cargo target,
PyInstaller build/dist, `.worker-venv`, paketlenmiş worker) siler. Kaynak, `docs/`
ve `.git` korunur.

## Dağıtım ve imza

Paketler **imzasız** dağıtılır (ADR-0012). macOS Gatekeeper / Windows Defender
uyarısı normaldir. Kaynak açık olduğu için isteyen kendi sertifikasıyla imzalar;
güncelleme akışı varsayılan kapalıdır.

## Lisans

AGPL-3.0-or-later. Ayrıntı: `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`.
Ticari/ kapalı kaynak kullanım için ayrı lisans görüşmesi gerekir.
