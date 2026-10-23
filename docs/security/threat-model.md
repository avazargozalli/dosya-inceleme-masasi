# Tehdit modeli (v0 — F00)

## Varlıklar

1. Müvekkil belgeleri ve türevleri (metin, OCR, görüntü)
2. Kasa ana anahtarı, parola türevi, kurtarma anahtarı
3. Dosya stratejisi: notlar, kronoloji, iddia–delil bağları
4. Sağlayıcı oturumları (CLI'nin kendi kimliği)
5. Kaynak zinciri bütünlüğü (alıntı → sürüm → konum)

## Aktörler

- Yerel kullanıcı (avukat), aynı makinedeki diğer kullanıcı, kötü niyetli belge,
  ağ üzerindeki gözlemci, sağlayıcı hizmeti, tedarik zinciri (bağımlılık), fiziksel hırsız.

## Tehditler ve kontroller

| # | Tehdit | Kontrol | Kanıt |
|---|---|---|---|
| T1 | Diskten kasa okunması | SQLCipher + şifreli blob; parola yoksa anahtar yok | F03 testleri |
| T2 | Parola kırma | Argon2id, yüksek maliyet, sürümlü başlık | `vault` unit test |
| T3 | Blob kesme/bit değiştirme | Chunked AEAD + son parça işareti + uzunluk | `vault` tamper test |
| T4 | Kötü niyetli PDF/DOCX | Ayrı worker süreci, limitler, güvenli XML, JS çalıştırma yok | F06 güvenlik testi |
| T5 | Worker'ın kasa anahtarını alması | Anahtar yalnız çekirdekte; worker'a parça metin gider | mimari + test |
| T6 | Dosyalar arası sızıntı | Her sorguda `case_id`; bileşik FK; kapsam testi | F08 security test |
| T7 | Onaysız bulut aktarımı | Aktarım izni, önizleme, `transfer_receipts` | F09 test |
| T8 | CLI kimliğinin kopyalanması | Token/çerez okunmaz; CLI kendi oturumunu yönetir | `cli-adapters` test |
| T9 | Aşırı yetkili CLI flag | İzin listesi; araç kapatma; capability sicili | F10 |
| T10 | Maskelemede gizli katman | Gerçek çıkarma + bağımsız doğrulama; başarısızsa bloke | F13 |
| T11 | Tedarik zinciri | Kilitli sürümler, SBOM, `--locked`, imzalı release | CI |
| T12 | Yedekte açık metin | Yedek de şifreli; anahtar pakette yok | F03/F14 |
| T13 | Loglarda belge içeriği | Yapılandırılmış, allowlist metadata; içerik loglanmaz | F02/F15 |

## Artık riskler (kabul edilen)

- Aynı kullanıcı hesabında çalışan kötü niyetli süreç, kilitsiz kasa açıkken ekran/bellek
  okuyabilir. İşletim sistemi düzeyinde tam sandbox v1 hedefi değildir; otomatik kilit ve
  bellek temizliği azaltır, sıfırlamaz.
- CLI sağlayıcıları verinin kendi bulutlarına gittiğini kullanıcıya bildirir; uygulama
  sağlayıcının saklama politikasını denetleyemez.
- Fiziksel erişim + güçlü parola yoksa kurtarma mümkün değildir (tasarım gereği).
