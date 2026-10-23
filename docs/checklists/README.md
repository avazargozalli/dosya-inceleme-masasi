# Kontrol listesi şablonları

Bu dizin, uygulamayla birlikte gelen lisanslı kontrol listelerinin **kaynak
sicilini** tutar. Amaç, bir dosyaya kopyalanan listenin hangi sürümden geldiğini
ve hangi lisansla kullanılabildiğini yıllar sonra da kanıtlayabilmektir
(F08-T11).

## Lisans ayrımı

- Uygulama kodu `AGPL-3.0-or-later` ile lisanslıdır.
- Bu dizindeki **şablon içerikleri** ayrı lisans taşır: `CC-BY-4.0`
  (<https://creativecommons.org/licenses/by/4.0/>). İçerik, bu proje için
  özgün olarak yazılmıştır; üçüncü taraf bir kontrol listesi metni
  kopyalanmamıştır.
- Her şablon dosyası kendi `licenseId`, `licenseUrl`, `author`, `reviewDate` ve
  `sourceUrl` alanlarını taşır. Katalog veritabanına yazılırken bu alanlar
  birlikte saklanır; dosyaya kopyalanan öğeler `template_version_id` ile bu
  künyeye bağlanır.

## Dosya biçimi

```json
{
  "id": "tpl:kira-hukuku:v1",
  "title": "Kira Hukuku — Dosya İnceleme Kontrol Listesi",
  "practiceArea": "Kira Hukuku",
  "version": 1,
  "author": "Dosya İnceleme Masası içerik ekibi",
  "reviewDate": "2026-10-01",
  "licenseId": "CC-BY-4.0",
  "licenseUrl": "https://creativecommons.org/licenses/by/4.0/",
  "sourceUrl": "https://github.com/.../kira-hukuku-v1.json",
  "isCommunity": false,
  "contentSha256": "<64 hex>",
  "items": [{ "category": "Sözleşme", "text": "...", "required": true }]
}
```

- `id` sürümü içerir (`tpl:<slug>:v<N>`); yeni sürüm **yeni kimlik** alır,
  eski sürüm silinmez.
- `contentSha256`, şablonun kanonik içerik özetidir. Hesap
  `dimvault/checklist-content/v1` etiketiyle; `id`, başlık, alan, yazar,
  gözden geçirme tarihi, lisans kimliği, sürüm, topluluk bayrağı ve maddelerin
  sırası/kategorisi/metni/zorunluluğu üzerinden yapılır. URL'ler hash'e girmez.
- Uygulama katalogu yüklerken hem dosyadaki `contentSha256` ile hesaplanan
  değeri hem de veritabanındaki sürüm/hash ve madde metinlerini karşılaştırır.
  Uyuşmazlıkta katalog **yüklenmez** ve kopyalama durur; sessizce farklı metin
  dosyaya yazılmaz.

## Yeni şablon veya sürüm ekleme

1. Yeni JSON dosyasını bu dizine ekleyin (veya sürümü artırıp yeni `id` verin).
2. `cargo test -p dim-storage --lib checklists` çalıştırın. Pinlenmiş hash testi
   kırılır ve `assert_eq!` çıktısında hesaplanan değer (sol taraf) görünür; bu
   değeri `contentSha256` alanına yazın. Hash'i elle veya başka bir araçla
   hesaplamayın: uygulama kanonik biçimi tek yerde (`content_hash`) uygular.
3. Hash'i dosyaya yazın, `bundled_catalog_hashes_are_pinned` testi yeşile
   dönsün.
4. Şablonu `crates/storage/src/checklists.rs` içindeki `CATALOG` listesine
   ekleyin.
5. Lisans bilgisini bu README'ye ve `THIRD_PARTY_NOTICES.md`'ye (üçüncü taraf
   içerik varsa) işleyin.

## Dosyaya kopyalama davranışı

- Kopya **idempotenttir**: aynı şablon sürümü ikinci kez kopyalanmaz.
- Kopya anındaki madde metni, sırası ve kategorisi `case_checklist_items`
  içinde tutulur; şablon sonradan silinse bile dosyadaki liste okunabilir.
- Durum kapıları:
  - `present` (Bulundu): en az bir **aynı dosyaya ait** belge bağı ister.
  - `missing` (İncelenen kapsamda bulunamadı): incelenen kapsam notu ister;
    belge bağı kabul etmez. Bu durum “dosyada yok” iddiası üretmez.
  - `not_applicable` (Uygulanamaz): gerekçe ister; belge bağı kabul etmez.
  - `open` (Açık): belge bağı taşımaz.
- Durum yazımı `row_version` ile korunur; eski sürümle yazan ikinci oturum
  `E_ROW_VERSION` alır ve güncel kaydı yükleyerek çözer.
