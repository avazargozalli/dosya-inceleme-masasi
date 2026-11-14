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

## F05-T03c — kapsam kapısı şemada: doğrudan SQL de çapraz dosya yazamaz (şema v31, 2026-11-14)

**Neden.** T03b'nin dürüst sınırı (b) şuydu: `document_versions.case_id` ile
`documents.case_id` arasında veritabanı düzeyinde kısıt/tetikleyici **yoktu**;
savunma yalnız çekirdek sorgusundaydı. Yani doğrudan SQL yazan bir yol (yeni bir
komut, bir betik, ileride eklenecek toplu içe aktarma) A dosyasının sürümünü,
koşusunu veya segmentini B dosyası adına yazabiliyordu — P0 sınıfı sızıntı.

**Yazılan (şema v31, `migrations/0031_scope_guards.sql`).** Kural artık
şemada: bir satırın `case_id`'si üst kayıtlarının dosyasıyla birebir aynı
olmalı; üst kayıt yoksa satır yazılmaz (fail-closed). **8 tablo × BEFORE
INSERT/UPDATE = 16 tetikleyici**:

- `document_versions` → `documents.case_id`
- `parse_runs` → `document_versions.case_id`
- `text_segments` → `parse_runs.case_id` **+** `page_id` kendi koşusunun sayfası
  olmalı (yanlış sayfa = alıntı yanlış belgeye konumlanır)
- `source_anchors` → sürümün dosyası, koşunun dosyası **ve** segmentin
  (`segment_id` metin; yabancı anahtar değil) aynı koşu + dosyada olması
- `source_marks` → sürüm dosyası (+ varsa koşu ve çapa dosyası)
- `redaction_plans` → sürüm dosyası (+ varsa koşu dosyası)
- `document_tags` → belgenin dosyası
- `embeddings` → segmentin dosyası

Reddetme sabit önekle yapılır: `RAISE(ABORT, 'DIM_SCOPE:<tablo>')` →
`StorageError::ScopeViolation` → **`E_DB_SCOPE`** (mesaj metni başka bir SQLite
hatasıyla karışmaz; hata işlem içinde oluşur, kısmi satır kalmaz).

**Mevcut veri.** Tetikleyiciler yalnız **yeni** yazmayı engeller; v31'den önce
yazılmış çapraz satırlar silinmez/değiştirilmez. `Database::scope_violations()`
8 tabloyu ayrı ayrı **sayar** (kimlik/içerik yok). Güncelleme anında
(`upgrade_with_backup`) toplam sıfırdan büyükse sayı denetime
(`vault_upgrade` / `scope_violations`) yazılır — taramanın hem ucuz hem anlamlı
olduğu tek yer.

**Testler.** `scope_guard::tests` **+11** (her tablo için çapraz INSERT, sayfa
üyeliği, UPDATE ile taşıma, aynı dosyada türev sürümün **meşru** kaldığı,
taramanın eski satırı saydığı, şemadan türetilen "her kapsam tablosunun kapısı
var mı" testi) + `upgrade_tests::upgrade_reports_cross_case_rows_left_by_the_previous_schema`
→ `dim-storage --lib` **607 → 619**. İki redaction testi (`plan_list_keeps_row_when_version_is_missing`,
`tampered_plan_case_scope_is_not_trusted`) artık bozuk satırı üretmeden önce
**kapıyı bilinçli düşürür**: bu testler eski/kurcalanmış kasadan gelebilecek
satırın **okuma** davranışını ölçer; v31 o durumu SQL üzerinden üretmez
(yorumlarda yazılı).

**Maliyet (ölçüldü; `crates/storage/tests/scope_guard_cost.rs`, `#[ignore]`,
debug, tek makine).** 20.000 segment: tetikleyicili **110,9 µs/satır**,
tetikleyicisiz **105,4 µs/satır** → **+5,5 µs/satır (%5)**.
`scope_violations()` 40.000 segmentte **40,7 ms** (yalnız güncelleme anında).

**Kapı kanıtı (gerçek koşular; betik `~/.hermes/cache/scratch/dim-scope-db-gates.py`,
her mutasyon tek başına, `try/finally` + `os.utime` + sha256 birebir — sonra
taban tekrar yeşil):** **9/9 KIRMIZI, taban 11/11 yeşilken** — M1–M8 her
tetikleyici tek tek düşürülür (ilgili test kırmızı) · M9 tarama sayımının dosya
koşulu bozulur (tarama testi kırmızı).

**Dürüst sınırlar.** (a) Kapı **yeni yazma** içindir; eski aykırı satırlar
sayılır ama **düzeltilmez** (birleştirme ayrı veri işi). (b) `anchor_id` /
`source_anchor_id` / `output_version_id` gibi **kanıt referansı** sütunları bu
turda kapsam dışı (çekirdek kapıları var; DB tetikleyicisi yok). (c)
`document_tags` ve `embeddings` tetikleyicileri bugünkü yazma yolları için
**ileriye dönük** (embeddings tablosuna bugün satır yazan yol yok). (d) SQLite
dosyasını **doğrudan** düzenleyen bir saldırgan (kasa anahtarıyla) tetikleyiciyi
de düşürebilir; bu kapı uygulama hatalarına karşı, dosya kurcalamasına karşı
değil. (e) Tauri komutları komut olarak koşulmadı; davranış gerçek SQLite ile
storage düzeyinde ölçüldü.

**CI (ölçüldü).** Kod `20d7525` (7 dosya, **+999/−1**) → run `43626252354`
**10/10 YEŞİL** (ilk koşu): `dim-storage` **619** iki OS'ta birebir (macos-15
**266,87 s** / ubuntu-latest **301,26 s**; yeni 11 `scope_guard` testi logda tek
tek `ok`), `dim-cli-adapters` **168**, `arayüz` 66 dosya / **1428** test, paket
`index-BPk1_PbH.js` 692,77 kB / gzip 184,88 kB (arayüz değişmedi),
`masaüstü kabuk` 51/0/5 + gerçek worker 5/0/0, `windows güvenli depo` 28,
`SBOM` / `kaynak arşivi` / `sözleşmeler` / `sürüm paketi manifesti` yeşil.
Yerel koşu **619/619** (415,18 s), `clippy -p dim-storage --all-targets
-D warnings` ve masaüstü `cargo check --all-targets` temiz.
