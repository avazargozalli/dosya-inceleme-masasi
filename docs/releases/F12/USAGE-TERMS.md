# F12-T11 — Kaynak kullanım koşulu ve korpus dağıtım kapısı

**Şema sürümü:** v16 (`0016_precedent_usage_terms.sql`)
**Komutlar:** `precedent_terms_get`, `precedent_terms_set`, `precedent_corpus_coverage`,
`precedent_corpus_export` (Tauri)

## Neden

Emsal kütüphanesine karar metni almak, metni **yeniden dağıtma** hakkını vermez.
Resmî yayın, açık lisans ve yazılı izin birbirinden farklı dayanaklardır; yalnız
iç kullanım için alınmış bir karar kütüphanede tutulabilir ama toplu korpus
olarak dışarı verilemez.

Bu adım iki ayrı şeyi kaydeder:

1. **Kullanım dayanağı** (`usage_basis`): `official_public` | `licence` |
   `permission` | `internal_only` | `unknown`.
2. **Yeniden dağıtım izni** (`redistribution`): `allowed` | `not_allowed` |
   `unknown`.

Ayrıca yazılı dayanak (`terms_url`, `terms_note`) ve kaydın alındığı zaman
(`terms_recorded_at`) tutulur. `terms_recorded_at` bir **hukuki inceleme
tarihi değildir**; kaydın düşüldüğü zamandır.

## Kapı kuralları (Rust, `crates/storage/src/precedent_terms.rs`)

- **Varsayılan `unknown`.** Yeni ve eski kayıtlar `unknown` ile başlar; kayıt
  yokluğu izin sayılmaz. Arayüzde de "izin bilinmiyor" diye yazılır.
- **`allowed` için yazılı dayanak zorunlu.**
  - Dayanak `official_public`, `licence` veya `permission` olmalı.
  - `internal_only` ve `unknown` ile `allowed` işaretlenemez (`E_TERMS`).
  - `terms_url` veya `terms_note` boşsa kayıt reddedilir (`E_TERMS`).
  - Bağlantı verilirse `http://`/`https://` ile başlamalı; kontrol karakteri
    içeremez; 2.000 karakteri aşamaz. Not 4.000 karakteri aşamaz.
- **Geçersiz değer yazılmaz.** Reddedilen kayıt önceki durumu değiştirmez.
- **Denetim:** her kayıt `precedent` / `usage_terms` olarak denetim zincirine
  düşer; denetime **not metni ve bağlantı yazılmaz**, yalnız dayanak türü,
  izin değeri ve yazılı dayanağın var olup olmadığı.

## Korpus dağıtım kapısı

`precedent_corpus_coverage()` kütüphanenin dürüst tablosunu verir: toplam kayıt,
en yeni tam metni olan kayıt, izinli / izinsiz / izin bilinmiyor sayıları ve
engelli kayıt listesi (en çok 100; toplam ayrıca `blockedTotal`).

`build_precedent_corpus(mode)` iki kip:

- **`permitted_only`** — yalnız `redistribution = allowed` olan tam metinler
  alınır. Dışarıda kalan her kayıt `excludedItems` listesinde **gerekçesiyle**
  (`redistribution_not_allowed` / `redistribution_unknown`) döner.
- **`all`** — metinli kayıtların **hepsi** izinli değilse `E_CORPUS_NOT_PERMITTED`
  döner ve **hiçbir metin** seçilmez. Kısmi korpus sessizce üretilmez.

Tam metni olmayan kayıtlar dağıtılacak metin taşımadığı için kapıyı kapatmaz;
sayıdan gizlenmez, `notes` içinde "N kaydın tam metni yok" diye yazılır.

Korpus yalnız **tam metinleri** taşır; künye alanları kimlik için yanındadır.
`contentHash` = `items` dizisinin kanonik JSON'u üzerinden SHA-256; sıra
`precedent.id`'ye göre sabittir.

## Dosya ve ağ

- `precedent_corpus_export` dosyayı **kullanıcının seçtiği konuma** yazar
  (`write_export`), yazılan baytların SHA-256'sını denetime kaydeder.
- İstek reddedilirse (`E_CORPUS_NOT_PERMITTED`) **ret denemesi** denetime
  `precedent_corpus` / `export_denied` olarak yazılır; hiçbir metin yazılmaz.
- Dışa aktarılacak izinli metin yoksa `E_CORPUS_EMPTY` döner; boş dosya
  oluşturulmaz.
- Bu katmanda ağ isteği yoktur (F12-T06 bağlayıcı sicili hâlâ tamamen kapalı).

## Arayüz

- Kütüphane listesinde her satırda **izin rozeti**: `izinli` / `izinsiz` /
  `izin bilinmiyor`. Kayıt yokluğu uyarı rengiyle görünür, "sorun yok" gibi
  görünmez.
- Seçili emsal için **Kaynak kullanım koşulu** kartı: dayanak, izin, bağlantı,
  not, kaydet düğmesi, kayıt zamanı. `allowed` seçilip dayanak boşsa arayüz
  nedeni yazar; asıl kapı Rust'tadır.
- **Toplu korpus dağıtımı** kartı: kapsam sayıları, engelli kayıtlar ve
  gerekçeleri, kip seçimi, "Korpusu dışa aktar…" (kaydetme iletişim kutusu),
  sonuçta yol/bayt/sha256. "Tümü" kipi kapalıyken nedeni yazılır.

## Doğrulama

- Rust storage **212** test (9 yeni `precedent_terms` testi): varsayılan
  `unknown`, yazılı dayanak zorunluluğu, geçersiz değerlerin yazılmaması,
  `all` kipinin kapıyı kapatması, `permitted_only` kipinin izinsiz metni
  **sızdırmaması** (JSON'da belirteç yok), izinsiz/bilinmiyor ayrımı, metinsiz
  kaydın kapıyı kapatmaması, hash kararlılığı.
- Web **175** test (10 yeni `precedent-terms` testi); `tsc` temiz.
- clippy `-D warnings` (storage + tauri) ve rustfmt iki manifest.

## Dürüst sınırlamalar

- İzin kaydı **kullanıcı beyanıdır**; ürün kaynağa gidip lisansı doğrulamaz.
  `terms_url` yalnız kanıt bağlantısıdır, erişilebilirliği denetlenmez.
- Kapı **tam metin** dağıtımını denetler. Künye/özet alanlarının tek tek
  paylaşılması (örn. dilekçeye künye kopyalama) bu kapının kapsamı dışındadır;
  F12-T12 bibliyografik çıktı bilinçli olarak metin taşımaz.
- `permitted_only` kipi bilinçli bir daraltmadır; dışarıda kalan kayıtların
  **sayısı ve kimliği** raporlanır ama bu, izinlerinin alınması için bir iş
  akışı değildir.
- Kütüphane düzeyinde toplu izin verme (tek tıkla "hepsi resmî yayın") yoktur;
  her kayıt ayrı kaydedilir. Bu bilinçli bir sınır: toplu işaretleme yanlış
  izin üretmenin en kolay yoludur.
- Tauri penceresi bu ortamda açılmadı → görsel QA yok; arayüz typecheck, birim
  testleri ve üretim derlemesiyle doğrulandı.
