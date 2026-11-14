# F05 — İçe aktarma ve belge sürümleri

- **Sürüm etiketi:** `v0.4.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F05-T01 | ✅ | dosya seçimi (dialog izni), türü içerikten algılama |
| F05-T02 | ✅ | SHA-256, özgün bayt değişmezliği, şifreli blob |
| F05-T03 | ✅ | `(document_id, version_no)` benzersiz, sürüm soy ağacı |
| F05-T03b | ✅ | **sürüm soy ağacı çağıranın sözüne bırakılmaz (2026-11-14):** sürüm ancak kendi belgesinin **dosyası** adına yazılır; ebeveyn aynı dosyada olmalı; silinmiş belgeye sürüm eklenemez. Ölçüm + **5/5 mutasyon kapısı** (aşağıdaki bölüm) |
| F05-T04 | ✅ | aynı içerik aynı dosyada → yeni kayıt yok, mevcut gösterilir |
| F05-T05 | ✅ | 250 MiB sınırı, desteklenmeyen tür reddi |

## Doğrulama

`cargo test -p dim-ingest` — 4 test: magic-byte, şifreli blob (düz metin yok),
tekrar tespiti, desteklenmeyen dosya. CI: **bekliyor**.

## Bilinen sınırlar

- Parola korumalı PDF algılanır ama parola girişi F06'da.
- `original_filename_encrypted` şu an boş; görünen ad `documents.display_name`.

## Parola korumalı PDF (ek)

- `document_parse` artık isteğe bağlı `password` alır; parola **kaydedilmez**,
  yalnız worker isteğinde taşınır ve DB'ye/denetime yazılmaz.
- Yanlış veya eksik parola `E_PDF_PASSWORD` döner; belge açılmış sayılmaz.
- Belgeler ekranında parola alanı ve "Ayrıştır"/"OCR" düğmeleri.
- Test: eksik parola, yanlış parola, doğru parola (AES-256 örnek PDF).

## Sürükle-bırak (ek)

- Dosya pencereye bırakıldığında Tauri `onDragDropEvent` yol verir; `importFromPath`
  aynı içe aktarma hattını (SHA-256, magic-byte, 250 MiB, şifreli blob) kullanır.
- HTML5 `drop` olayı Tauri'de dosya yolu vermediği için bilinçli olarak kullanılmadı.
- Seçili dosya yokken bırakma yok sayılır; kullanıcıya sessizce yanlış dosyaya
  yazılmaz.

## F05-T03b — sürüm soy ağacı çağıranın sözüne bırakılmaz (2026-11-14)

**Ölçülen kusur (üç yüzü).** `add_version_with_parent` `document_id` ile
`case_id`'nin **tutarlı olduğunu doğrulamıyordu**:

1. Sürüm, belgenin kendi dosyası yerine **başka dosya** adına yazılabiliyordu;
   üstelik belgenin `current_version_id` işaretçisi o satıra dönüyordu.
2. Ebeveyn sürümün dosyası denetlenmiyordu → soy ağacı dosya sınırını
   aşabiliyordu.
3. Silinmiş belgeye (`deleted_at` dolu) yeni sürüm eklenebiliyordu.

**Düzeltme öncesi gerçek koşu (geçici sonda, silindi):** A dosyasındaki belgeye
`case_id=B` ile `add_version` → `Ok(version_no=2)`; ardından
`coverage_report(B)` → **belge=0, sürüm=1, koşu=1** (boş dosya, başka dosyanın
sürümünü sayıyordu).

**Yazılan (çekirdek).**

- `document_case(document_id)` — silinmemiş belge satırından dosyayı okur;
  kapsam kapılarının **tek kaynağı**.
- `add_version_with_parent` sürümü ancak kendi dosyası adına yazar; ebeveynin
  **dosyasını** doğrular. Ebeveyn başka bir *belgenin* sürümü olabilir (maskeli
  kopya yeni belge açar, bu yol korunur), ama başka bir *dosyanın* sürümü
  olamaz.
- Reddedilen çağrı **hiçbir iz bırakmaz**: satır yok, `current_version_id`
  değişmez, blob `reference_count` artmaz, sürüm numarası tükenmez.

**Testler.** `documents::tests::version_is_written_only_under_its_own_case`,
`documents::tests::derived_version_parent_stays_inside_the_case` (+2) →
`dim-storage --lib` **603 → 607** (F06 ikizi dâhil; 394,97 s).

**Kapı kanıtı (gerçek koşular; betik `~/.hermes/cache/scratch/dim-scope-gates.py`,
her mutasyon tek başına, `try/finally` + `os.utime` damgası + sha256 birebir —
`documents.rs 7d5b509a…`, `parsing.rs 54c17eff…`):** **5/5 KIRMIZI, taban
4/4 yeşilken** (sonrasında taban tekrar 4/4 yeşil, geri yazım birebir) —
M1 belge dosya kapısı düşer · M2 ebeveyn dosya kapısı düşer · M3 koşu
sürüm-dosya kapısı düşer · M4 sayfa koşu-dosya kapısı düşer · M5 silme izi
okunmaz.

**Dürüst sınırlar.** (a) Bugün ürün yolları (`import`, maskeli kopya) `case_id`'yi
sürüm satırından okur → **canlı sızıntı yok**; kapı, sınıfı kapatır (doğrudan
çekirdek çağrısı yanlış eşle gelebilir). (b) `document_versions.case_id` ile
`documents.case_id` arasında DB düzeyinde bileşik kısıt/trigger **yok**; savunma
çekirdek sorgusundadır (şema v30 kısıt eklemek bugünkü satırları taramayı
gerektirirdi). (c) Silinmiş belgeye sürüm yazımı reddedilir ama silme yolunun
kendisi (maskeli kopyayı atma) bu turda değişmedi. (d) Tauri komutları komut
olarak koşulmadı; davranış storage düzeyinde gerçek SQLite ile ölçüldü.

**CI (ölçüldü).** Kod `0465eb6` (2 dosya, **+363/−0**) → run `43616435408`
**10/10 YEŞİL**: `dim-storage` **607** iki OS'ta birebir (macos-15 231,73 s /
ubuntu-latest 580,79 s), `arayüz` 66 dosya / **1428** test, paket
`index-BPk1_PbH.js` 692,77 kB / gzip 184,88 kB; `masaüstü kabuk`, `windows
güvenli depo`, `kaynak arşivi`, `SBOM`, `belge worker'ı`, `sözleşmeler` ve
`sürüm paketi manifesti` yeşil. Belgeler `785b869` → run `43617628086`
**10/10 YEŞİL**.
