# F04 — UX ve dosya çalışma alanları

- **Sürüm etiketi:** `v0.3.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F04-T01 | ✅ | Dosyalar ekranı: liste, oluşturma, detay |
| F04-T02 | ✅ | arşivle / geri al / geri dönüşümlü silme |
| F04-T03 | ✅ | `row_version` çakışmasında `E_ROW_VERSION` |
| F04-T04 | ✅ | saklama kilidi (`retention_hold`) silmeyi engeller |
| F04-T07 | ✅ | **iptal gerçek** (süreç durdurulur, yarım çıktı yazılmaz) + ayrıştırma arayüz iş parçacığını kilitlemez + **canlı aşama/sayım** (worker ilerleme çerçevesi → `parse-progress` olayı → arayüz sayacı) + **maskeleme de kuyruk işi** (kasa kilidi worker boyunca tutulmaz, `redact-progress` aşamaları, iptal) + **yeniden deneme gerçekten çalışır** + **yeniden deneme kaydedilmiş ayarlarla koşar** (2026-10-28) |
| F04-T08 | 🚧 | **merkezi sözlük + biçimlendirme katmanı** — kabuk (`App.tsx`), **14 ekran** + **5 bileşen** (`JobQueueCard`, `WorkerBudgetCard`, `Notes`, `DualReader`, `PageMap`) + **26 yardımcı lib** taşındı → kapı kapsamı **46 dosya**; **kalan yüzey 0 gömülü kullanıcı metni** (3 belgelenmiş istisna: geliştirici iddiası `main.tsx`, dosya adı regex'i `lib/filename.ts`, satır sonu yorumu `lib/backup-schedule.ts`). Kapı **genel** (taşınmış dosyada yorum dışında Türkçe harf kalmaz) + **sözlük bağı** testi. **Okuyucu davranış testi eklendi** (`components/DualReader.test.tsx`, 6 test; mutasyonla kanıtlandı) — 2026-10-31. Açık: **görsel QA** (Tauri penceresi bir kez bile açılmadı) + Vite 500 kB kod bölme kararı (**insan**) |

## Okuyucu davranış testi — `components/DualReader.test.tsx` (2026-10-31)

Önceki turun dürüst sınırı şuydu: “**`DualReader` render/etkileşim testi yok** (eşitlik
lib düzeyinde + JSX'te birebir taşımayla sağlandı)”. Depoda tek bileşen davranış testi
`src/lock-behavior.test.tsx`ti; paylaşılan okuyucu (1640 satır, `Search.tsx` + `Cases.tsx`
tarafından kullanılıyor) hiç render edilmiyordu. Bu turda kapatıldı.

**Ortam:** `@vitest-environment jsdom` + `@testing-library/react` (ikisi de depoda vardı).
`lib/api` modülü kısmen mock'lanır (`documentPages`, `marksForVersion`, `anchorLocate`,
`documentPagePreview`, `documentInventory`); `t()` ve saf yardımcılar **gerçek** çalışır —
yani test, sözlükten gelen gerçek etiketlerle sorgular (metin kopyası yok).

**Ölçülen altı davranış (hepsi ürün kararı):**

1. İki panel **kendi** belgesini açar, kendi sayfa metnini gösterir; her panel yalnız kendi
   sürümü için `documentPages` çağırır (çapraz istek yok, seçiciler `v1`/`v2` ile doğru).
2. Bir panelde sayfa değişince **diğer panel yerinde kalır** (sayfa 2’ye geçen sol panel,
   sağda sayfa 1’i bırakır).
3. **Yakınlaştırma panel başınadır**: iki tık `100% → 125% → 150%` (mevcut değerden
   ilerler, sabit kademeye dönmez), bir geri `150% → 125%`; sağ panel `100%` kalır.
4. **Sayfa haritası yalnız gerçek blok kutusu varken** çizilir: `bbox` taşıyan sayfada
   `role="img"` + `pageMap.aria` etiketi; bloksuz sayfada hiç `img` yok.
5. **Sayfa ölçüsü yoksa harita çizilmez** (`width/height` null + blok kutusu olsa bile) —
   “konum uydurulmaz” kuralının render düzeyinde kanıtı.
6. **Boş panel, diğer panelde açık sürümü seçmez**: `documents` listesinde ilk sırada
   diğer panelin belgesi olsa bile boş panel öteki sürümü değil, kalan belgeyi açar
   (`pickInitialVersion(documents, otherVersionId)`).

**Kapı kanıtı (kırmızı→yeşil, ölçüldü).** Testin süs olmadığı üç mutasyonla ölçüldü;
her mutasyon **1 failed | 5 passed**, geri alma `shasum -a 256` ile **birebir**:

| Mutasyon | Düşen test | Ölçülen hata |
|---|---|---|
| `pickInitialVersion(documents, otherVersionId)` → `(documents, null)` | “boş panel diğer panelde açık sürümü seçmez” | `expected 'v2' to be 'v1'` |
| `PageMap`: `page.width ?? 0` / `page.height ?? 0` → `?? 600` / `?? 800` | “sayfa ölçüsü yoksa … harita çizilmez” | beklenmeyen `role="img"` bulundu |
| `goToPage`: gövde `setView((state) => state)` | “bir panelde sayfa değişince diğer panel yerinde kalır” | `Unable to find … /Sol belge ikinci sayfa/` |

**Ölçülen bir sınır (dürüst):** yalnız `width` mutasyonu (`?? 600`, `height` 0 kalırken)
testi **düşürmedi**, çünkü `PageMap` iki boyutu birlikte istiyor (`width <= 0 || height <= 0`
→ null). Yani kapı iki ölçünün **birlikte** uydurulmasını yakalar; tek boyut zaten
çizilmiyor. Bu, testin değil davranışın sınırı; raporda böyle yazıldı.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **510/510** (37 dosya; 504 → **+6**) ·
`vite build` temiz (**568,50 kB**, 88 modül — önceki turla aynı; sözlük/JSX değişmedi).

**Dürüst sınırlar:** jsdom **tarayıcı değildir** — CSS yerleşimi, gerçek `Tauri` penceresi,
`invoke`/IPC ve görsel QA hâlâ yok; `api` mock'lu olduğu için çekirdek komut yolu bu testte
koşmaz (o yol Rust testlerinde). 1640 satırlık bileşenin yalnız altı davranışı sabitlendi:
klavye gezinmesi (`Alt`+ok/`PageUp`), metin seçimi → bağ/işaret/not oluşturma, bölge
işaretleme, görüntü önizlemesi ve envanter paneli bu turda **kapsam dışı** (ayrı dilimler).
Windows'ta koşulmadı.

## Ölçülen kusur — “Fiziksel sayfa” etiketi **iki kaynaktı** (2026-10-31)

Okuyucu testi yazılırken ölçüldü: `lib/reader.ts` içinde kullanıcıya görünen sayfa etiketi
**gömülü** üretiliyordu (`` `Fiziksel sayfa ${page.pageIndex + 1}` ``), oysa sözlükte
`readerUi.physicalPage` **zaten vardı** ve `DualReader.tsx` sayfa düğmelerinde onu
kullanıyordu. Yani aynı etiket iki kaynaktan geliyordu: sözlük değişse sayfa düğmeleri
güncellenir, **rozetler/`aria-label`/not kaynağı satırı eski metni göstermeye devam ederdi**
(sessiz ayrışma). Harf kapısı bunu göremez: “Fiziksel sayfa” Türkçeye özgü harf
(`ığüşöç`) taşımaz — bilinen sınırın yeni bir vakası.

**Düzeltme:** `pageLabelInfo` artık `t("readerUi.physicalPage", { page: … })` kullanıyor;
üretilen metin **bayt düzeyinde aynı** (mevcut `reader` 62 testi değişmeden geçti).

**Yeni kapı (2 kural, `i18n-bindings.test.ts`):**

1. **Bağ:** `pageLabelInfo({pageIndex})` çıktısı `t("readerUi.physicalPage")` ile birebir
   (`pageIndex` 0, 3, 40).
2. **Kaynak taraması:** `lib/reader.ts`te `t("readerUi.physicalPage"` **var** ve yorumlar
   ayıklandıktan sonra `Fiziksel sayfa` gömülü **yok** — geri gömme sessizce yeşil kalamaz.

**Kırmızı→yeşil (ölçüldü):** etiket şablon literaline geri gömüldüğünde
`i18n-bindings` **1 failed | 53 passed**; geri alma `shasum -a 256` **birebir**
(`4b10db0c…`). Dürüst kayıt: kırmızıyı `toContain('t("readerUi.physicalPage"')` kuralı
verdi; literal taraması, `t()` çağrısı yerinde kalıp metin başka bir satıra kopyalanırsa
devreye giren **yedek** kuraldır.

**Ölçüm (bu adımdan sonra):** `tsc --noEmit` temiz · `vitest` **511/511** (37 dosya) ·
`vite build` temiz (**568,52 kB**, `index-D3zDHRzL.js`, 88 modül).

**CI `41189990335` — 7/7 YEŞİL (ölçüldü):** `arayüz` **37 dosya / 511 test** (30 s;
tsc + vitest + build — yerel/CI **birebir**), `rust (macos-15)` 3 dk 36 s,
`rust (ubuntu-latest)` 8 dk 55 s, `masaüstü kabuk` 40 s, `belge worker'ı` 52 s,
`SBOM` 22 s, `sözleşmeler` 5 s. Koşu **iki commit'i birlikte** kapsıyor
(`4db4eb1` davranış testi + `f2e0b19` etiket düzeltmesi).


## F04-T08 — sözlük taşıması: `components/DualReader.tsx` + `PageMap.tsx` + `lib/reader.ts` + `lib/inventory.ts` (2026-10-31, on ikinci tur)

**Önceki turun açık işi kapandı:** `b1cbb22` + `7b21d99` → CI `41153883057` ve `41154773683`
**7/7 yeşil** (önceki turda doğrulanmıştı; bu turda yeniden koşmadı).

**Neden bu dilim (ölçüldü):** kalan yüzeyin **tamamı** dört dosyadaydı: `components/DualReader.tsx`
(93 Türkçe satır) + `lib/inventory.ts` (32) + `lib/reader.ts` (8) + `components/PageMap.tsx` (3)
= **136 satır**. Import taraması: `DualReader`'ı **`Search.tsx` + `Cases.tsx`**,
`PageMap`'i yalnız `DualReader` import ediyor; **`lib/reader.ts`** ise `DualReader`, `PageMap`,
`Search.tsx` ve `Cases.tsx` tarafından — yani **zaten taşınmış iki ekranın nokta etiketi,
yakınlaştırma alt yazısı ve eski sürüm rozeti hâlâ lib'den** geliyordu (gerçek sızıntı).
`lib/inventory.ts`'i yalnız `DualReader` import ediyor. Dördü **kapalı küme** → birlikte taşındı.

**Yazılanlar**

- `lib/i18n.ts` + **5 bölüm / 200 anahtar** (`readerUi` 122, `inventory` 58, `pageMap` 7,
  `readerLib` 12, `api` 1) → sözlük **1664 → 1864** (`messageKeys()` ile ölçüldü).
  Kapı kapsamı **41 → 46** dosya.
- **Sessiz kırılganlık ölçüldü ve kaldırıldı:** `const paneKey = label === "Sol belge" ? "sol" : "sag";`
  satırı panel kimliğini **görünen metne** bağlıyordu; metin sözlüğe taşınsaydı karşılaştırma
  sessizce `"sag"`a düşecekti (form `id`'leri çakışır, iki panel aynı alanı paylaşır).
  Bu satır **Türkçe harf taşımadığı için harf kapısı onu hiç görmezdi**. Yerine `pane`
  prop'u (`"left" | "right"`) geldi; etiket `t()`den türetiliyor.
- **Etiket haritaları anahtar haritasına çevrildi:** `STRUCTURE_LABELS`/`ATTACHMENT_KIND_LABELS`
  → `STRUCTURE_KEYS`/`ATTACHMENT_KIND_KEYS` (`Record<string, MessageKey>`) + `structureLabel()`;
  `t()` **çağrı anında** çözülür (import anında donma yok). Bilinmeyen anahtar kendi adıyla yazılır.
- **Ayırıcı kararı sürüyor:** `" · "` değerlere gömülmedi; `candidateTitle`, `noteSource`,
  `viewHint` + `rotationSuffix`, `pageMap` alt yazısı, `inventorySummary`, `changeSummary`
  ve ekranın “Seçim: … · karakter …” satırı **parçalardan** kurulur (`join(" · ")` veya şablon).
  `pageButtons*`, `viewHintRest`, `keyboard*` parçaları `<strong>`/`<code>` öğeleri arasına
  `{" "}` ile yerleştirildi (JSX boşluk tuzağı).
- **Ölçülen kusur (belge düzeltmesi):** önceki turların “`api.ts`teki satır **satır sonu
  yorumu**” notu yanlıştı — `lib/api.ts`teki satır `toApiError`nun **kullanıcıya gösterilen**
  yedek iletisiydi (`"Beklenmeyen bir hata oluştu."`, `E_UI_UNKNOWN`). Sözlüğe alındı
  (`api.unexpectedError`) ve `lib/api.ts` **kapıya eklendi**; `main.tsx`teki `"kök düğüm yok"`
  ise gerçekten geliştirici iddiası (kullanıcıya render edilmez) → istisna olarak yazıldı.

**Doğrulama (ölçüldü)**

- **Kırmızı→yeşil (harf kapısı, dört dosya birlikte):** `DualReader`a `_LEAK_DR = "Okuyucu görünümü"`,
  `PageMap`e `_LEAK_PM = "konum haritası"`, `reader.ts`e `_LEAK_R = "yeni üretildi"`,
  `inventory.ts`e `_LEAK_I = "Gömülü paket"` aynı anda → `localization` **1 failed | 8 passed**,
  rapor **dördünü birlikte**:
  `["components/DualReader.tsx:1565","components/PageMap.tsx:9","lib/inventory.ts:60","lib/reader.ts:328"]`;
  geri alındı, `shasum -a 256 -c` **birebir OK** → 9/9.
- **İkinci kapı (iki vaka ayrı ayrı ölçüldü):**
  (a) `runBadge` dönüşü `t("readerLib.oldRunBadge")` yerine **harfsiz** `"eski ayristirma"` →
  `i18n-bindings` **1 failed | 51 passed** (`Expected: "eski ayrıştırma" / Received: "eski ayristirma"`)
  **ve** `localization` kırmızı (sebep harf değil, “kullanılmayan anahtar”).
  (b) `attachmentSummary` dönüşü `t("inventory.noAttachments")` yerine **birebir aynı** `"Ek yok"` →
  `i18n-bindings` **YEŞİL** kaldı (değer eşit), yalnız “kullanılmayan anahtar” kuralı yakaladı —
  bilinen sınır **yeni bir vakayla yeniden ölçüldü** (harfsiz + aynı metin iki kapının da dışında).
- **Yeni 8 test** (`i18n-bindings.test.ts`): önizleme alt yazısı + üç sürüm rozeti; yeniden bağlama
  durumunun **dört vakası**; yapı/ek etiketleri + bilinmeyen anahtarın kendi adıyla yazılması;
  envanter özetleri (ek yok / ek sayısı+boyut / üç parçalı değişiklik izi / iz yok);
  künye satırları + **sıralı** sınır uyarıları + düğme kapısı; **42 parametreli satır `{` bırakmaz**;
  dört dosyanın `t()` anahtarları sözlükte eksiksiz; `toApiError` ham hata metnini sızdırmaz.
- Mevcut `reader` **62** ve `inventory` **14** testi **birebir `.toBe`** karşılaştırıyor ve
  **değişmeden geçti** (76/76, iki test dosyasında `git diff` boş) → metinler byte düzeyinde aynı.
- `tsc --noEmit` temiz · `vitest` **504/504** (36 dosya; 496 → +8) · `vite build` temiz
  (**568,46 kB**; 559,31 → sözlük büyüdü, Vite 500 kB uyarısı sürüyor).
- Taşınmış dört dosyada yorum dışı **0** Türkçe harf satırı. Kalan yüzey: **3 dosyada 1 satır**
  (`main.tsx` geliştirici iddiası, `lib/filename.ts` dosya adı regex'i, `lib/backup-schedule.ts`
  satır sonu yorumu) + sözlüğün kendisi.

**Commit/CI:** `3545f12` (kod, 8 dosya, +791/−221) → **CI `41160138998` 7/7 YEŞİL**: `arayüz`
**36 dosya / 504 test** (tsc + vitest + build, 35 s; yerel/CI **birebir** — aynı derleme özeti
`index-DiHv4aE5.js`, 568,50 kB), `rust (macos-15)` 4 dk 29 s, `rust (ubuntu-latest)` 6 dk 52 s,
`masaüstü kabuk` 41 s, `belge worker'ı` 37 s, `SBOM` 21 s, `sözleşmeler` 5 s. `HEAD == origin/main`.

**Dürüst sınırlar:** F04-T08 kod tarafı **tamam** ama satır hâlâ **🚧**: **görsel QA yok** (Tauri
penceresi hiç açılmadı; iki panelin yeni etiketleri ekranda görülmedi), **DualReader'ın
render/etkileşim testi yok** (eşitlik lib düzeyinde + JSX'te birebir taşımayla sağlandı), Vite
**500 kB** kod bölme kararı **insan** işi, Windows denenmedi. `readerUi` metinleri tek dilde
(Türkçe) — sözlük çeviri katmanı değil, tek kaynak.

**Sıradaki aday:** kalan iş listesindeki kodla ölçülebilir maddeler (F10 izinli argümanlarla CLI,
F16 atomik geri dönüş tatbikatı, F15 benchmark harness genişletmesi) + **Vite kod bölme kararı
(insan)**; F17/F02 Windows, pilot, VoiceOver, notarization insan/donanım bekliyor.

## F04-T08 — sözlük taşıması: `screens/Review.tsx` + `lib/intext.ts` + `lib/reviewTarget.ts` (2026-10-31, on birinci tur)

**Önceki turun açık işi kapandı:** `1949cf5` + `541c962` → CI `41147576596` ve `41148242123`
**7/7 yeşil** (önceki turda doğrulanmıştı; bu turda yeniden koşmadı).

**Neden bu dilim (ölçüldü):** kalan yüzeyin en büyük **kapalı kümesi** İnceleme çevresiydi:
`screens/Review.tsx` (100 Türkçe satır) + metninin bir kısmını taşıyan `lib/intext.ts` (35) +
`lib/reviewTarget.ts` (2) = **137 satır**. Import taraması: `intext`'i **yalnız** `Review.tsx`
(kendi testi dışında) import ediyor; `reviewTarget`'ı `Review.tsx` + **`lib/api.ts`** import
ediyor ama **yalnız tip** (`import type { ReviewTargetInput }` — çalışma zamanı metni yok) →
küme **kapalı** sayıldı, üçü birlikte taşındı. `components/DualReader.tsx` (93) ve
`components/PageMap.tsx` (3) bu dilime **girmedi**: `DualReader`'ı `Search.tsx` ve `Cases.tsx`
de import ediyor (paylaşılan bileşen); ayrı dilim.

**Yazılanlar**

- `lib/i18n.ts` + **3 yeni bölüm / 158 anahtar** (`review` 109, `intext` 45, `reviewTarget` 4)
  → sözlük **1506 → 1664** (`messageKeys()` ile ölçüldü). Kapı kapsamı **38 → 41** dosya.
- **Etiket haritaları sözlükten türetildi:** ekrandaki altı harita (`CLAIM_KIND`, `RELATION`,
  `CLAIM_RELATION`, `ITEM_KIND`, `VERDICT`, `ITEM_STATE`), `intext.ts`teki
  `QUOTE_STATE_LABEL`/`MATCH_KIND_LABEL`/`TEXT_ORIGIN_LABEL` ve `reviewTarget.ts`teki
  `TARGET_KIND_LABEL` artık `t()`den kuruluyor. (Tek dilli ürün: değerler **import anında**
  çözülür, dil değiştirme yok — açık kayıt.)
- `Review.tsx`: başlık (`nav.review`), lead, dosya/iddia formu, iddia listesi + künyesi,
  delil bağı ve **iddia ilişkileri** (iki boş durum, ilişki türleri, gerekçe), **inceleme
  kuyruğu** (özet sayacı, aday formu, çelişki değerlendirmesi, hedef kayıt türü/iddia türü,
  düzenleme, yerine geçme, karar verilmiş listesi) ve **metin içi alıntı denetimi** (kapsam
  seçimi, bulgu kartı, sayfa sınırı notu, eski sürüm rozeti, bağlama düğmesi, denetim
  sınırları) — **125 `t()` çağrısı / 115 benzersiz anahtar**. Üç hata iletisi ve iki
  **denetim metni** (`kullanıcı düzeltmesi`, `kullanıcı onayı`) de sözlükten; değerler birebir.
- `lib/intext.ts`: durum/eşleşme/kaynak etiketleri, özet satırı, sayfa aralığı, bağlanamama
  nedenleri (4), eski sürüm notları (3 biçim), taranan sürüm satırı, sayılmayanlar, kapsam
  satırı — 46 çağrı / 44 anahtar.
- **Ayırıcı kararı sürüyor:** `" · "` parçaları sözlük **değerlerine gömülmedi** (kapı baş/son
  boşluk + çift boşluk yasaklıyor); `positionLabel`, `candidateLabel`, `quoteCheckSummary`,
  `versionsLabel` ve ekrandaki `` `{kapsam} · {sayfa} · {sürüm}` `` satırı `join(" · ")` ile
  **tek yerde** kurulur. Bilinçli istisna: `intext.stateExact` = "Birebir · tek konum" —
  bu bir **etiket metnidir**, ayırıcı değil.

**Doğrulama (ölçüldü)**

- **Kırmızı→yeşil (harf kapısı):** `Review.tsx`e `const _LEAK_SCREEN = "İnceleme kuyruğu";`
  **ve** `intext.ts`e `export const _LEAK_LIB = "Alıntı sayfası";` aynı anda → `localization`
  **1 failed | 8 passed**, rapor **iki dosyayı birlikte**: `["lib/intext.ts:4","screens/Review.tsx:40"]`;
  geri alındı, `shasum -c` **birebir OK** → 9/9.
- **İkinci kapı (ölçüldü):** `intext.ts`te dönüş `t("intext.bindOnlyExact")` yerine
  **Türkçe harf taşımayan** `"Sadece birebir tek konum kaynaga baglanir."` yazıldı →
  `i18n-bindings` **1 failed | 44 passed** (`Expected: "Yalnız birebir tek konum kaynağa
  bağlanır." / Received: "Sadece birebir…"`). Aynı anda `localization` de kırmızıydı ama
  sebep **harf değil**, "kullanılmayan anahtar" (`intext.bindOnlyExact`) — dürüst kayıt:
  harf kapısı harfsiz metni kaçırır, bağ kapısı yakalar.
- **Yeni 7 test** (`i18n-bindings.test.ts`): durum/eşleşme etiketleri; sayfa aralığı + özet
  satırı; konum/aday/eski sürüm satırları (parçalardan **birebir** birleşir); bağlanamama
  nedeni + kapsam + taranan sürüm + sayılmayanlar; `TARGET_KIND_LABEL` eşlemesi; **13
  parametreli satır `{` bırakmaz**; ve **ekran + iki lib'in `t()` çağrıları (175 çağrı /
  163 benzersiz anahtar; Review 125, intext 46, reviewTarget 4) hepsi sözlükte**.
- Mevcut `intext` **13** ve `reviewTarget` **8** testi **birebir `.toBe`** karşılaştırıyor ve
  **değişmeden geçti** (21/21) → metinler byte düzeyinde aynı; `reviewTarget.test.ts`te tek
  satır bile değişmedi.
- `tsc --noEmit` temiz · `vitest` **496/496** (36 dosya; 489 → +7) · `vite build` temiz
  (**559,31 kB**; 552,09 → sözlük büyüdü, Vite 500 kB uyarısı sürüyor).
- Taşınmış üç dosyada yorum dışı **0** Türkçe harf satırı (kapının mantığıyla).
  Kalan yüzey **136 Türkçe satır / 4 dosya** (`DualReader` 93, `inventory` 32, `reader` 8,
  `PageMap` 3) + 4 dosyada satır sonu yorumu (`api.ts`, `backup-schedule.ts`, `filename.ts`,
  `main.tsx`).

**Commit/CI:** `b1cbb22` (kod, 6 dosya, +623/−199) → **CI `41153883057` 7/7 YEŞİL**:
`arayüz` **36 dosya / 496 test** (tsc + vitest + build, 34 s; yerel/CI **birebir**),
`rust (macos-15)` 4 dk 46 s, `rust (ubuntu-latest)` **6 dk 08 s**, `masaüstü kabuk` 31 s,
`belge worker'ı` 37 s, `SBOM` 25 s, `sözleşmeler` 7 s. `HEAD == origin/main`.

**Disk (ölçüldü):** tur başında **4,8 G** (< 5 G eşiği) → güvenli önbellek temizliği (macOS
aerial duvar kağıdı videoları 4,1 G, Chrome `Service Worker/CacheStorage` 821 M, Google
uygulama önbelleği 116 M, cargo registry 368 M) → **10,4 G**; yalnız TS işi koştu (Rust
derlemesi yok); `disk-guard` commit sonrası temiz (**6,9 G** boş).

**Dürüst sınırlar:** `components/DualReader.tsx` (93) + `components/PageMap.tsx` (3) ve
`lib/inventory.ts` (32) + `lib/reader.ts` (8) kaldı → F04-T08 **🚧**. `DualReader` paylaşılan
bileşen (`Search.tsx` ve `Cases.tsx` de kullanıyor). Etiket haritaları **import anında**
`t()` çağırıyor (tek dilli ürün; dil değiştirme yok). Metinler birebir aynı (21 mevcut test
değişmeden geçti), ama **Review ekranının render/etkileşim testi yok** — eşitlik lib
düzeyinde ve JSX'te birebir taşımayla sağlandı; Tauri penceresi açılmadı → **görsel QA yok**;
Windows denenmedi.

**Sıradaki aday:** **`DualReader` kümesi** (`components/DualReader.tsx` 93 +
`components/PageMap.tsx` 3 + paylaştığı `lib/reader.ts` 8 = **104 satır**; `reader.ts`'i
başka ekranlar da kullanıyor — ölçülecek); sonra `lib/inventory.ts` (32). Ardından
F04-T08 kapanışı için Vite 500 kB kod bölme kararı (**insan**).


## F04-T08 — sözlük taşıması: `screens/Providers.tsx` + dört sağlayıcı lib'i (2026-10-31, onuncu tur)

**Önceki turun açık işi kapandı:** `257d3fd` + `901b26e` → CI `41139371524` ve `41140449934`
**7/7 yeşil** (doğrulandı; bu turda ek doğrulama gerekmedi).

**Neden bu dilim (ölçüldü):** kalan yüzeyde en büyük **kapalı küme** Sağlayıcılar çevresiydi:
`screens/Providers.tsx` (110 Türkçe satır) + metninin bir kısmını taşıyan dört lib —
`lib/providers.ts` (42), `lib/arch.ts` (36), `lib/package.ts` (31), `lib/attack.ts` (24) =
**243 satır**. Import taraması: `arch`, `package`, `attack` lib'lerini **yalnız**
`Providers.tsx` (kendi testleri dışında) import ediyor; `lib/providers.ts`'i ayrıca
**`lib/inventory.ts`** import ediyor — ama **yalnız `formatBytes`** (saf yeniden dışa aktarım,
metin yok) → küme **kapalı** sayıldı ve beşi birlikte taşındı. `inventory.ts` bu turda
taşınmadı (kendi metnini taşımaya devam ediyor, açık kayıt).

**Yazılanlar**

- `lib/i18n.ts` + **5 yeni bölüm / 315 anahtar** (`providers` 142, `probe` 65, `arch` 38,
  `attack` 31, `package` 39) → sözlük **1191 → 1506** (`messageKeys()` ile ölçüldü).
- **Modül sabitleri fonksiyona çevrildi:** ekrandaki `AUTH_LABEL` → `authLabel(mode)`,
  `lib/package.ts`'teki `ROLE_LABELS` → `ROLE_KEYS` + `roleLabel(role)`,
  `PROBLEM_LABELS` → `PROBLEM_KEYS` + `problemText`, `PACKAGE_LIMITS` → **`packageLimits()`**
  (sabit harita/sabit metin sözlüğü **import anında** okuyordu — Notlar/Emsal dersinin aynısı;
  `lib/arch.ts`'te 19 kodluk neden haritası da `REASON_KEYS` tablosuna çevrildi).
- `screens/Providers.tsx`: API profili kartı, başlık/lead, "AI kapalı" bildirimi, yetenek
  matrisi (kapı rozeti, `{row.label} · gerekli`, sınırlar), worker paketi (üç sayaç, roller,
  manifestle uyuşmayan/fazla dosya, Tesseract satırı, sınır cümlesi), saldırı testi (fikstür
  özeti + 4 uyarı, vektör kararları, kapatma yolu), mimari denetimi (5 `dt` + yorumlayıcı +
  neden + başlık baytı), CLI kartları (durum rozetleri, 5 `dt`, toplu kullanım yasağı,
  "Bağlantıyı sına"/"İptal", kum havuzu dosyaları, çıktılar) ve **`ResiduePanel`**
  (envanter tablosu 5 sütun, onay kutusu, ölçüm öncesi/sonrası, temizlik, süpürme).
- **Ayırıcı kararı (sürüyor):** `" · "` ve baştaki tek boşluk **değerlere gömülmedi**
  (kapı baş/son boşluk + çift boşluk yasaklıyor); birleştirme kodda tek yerde
  (`join(" · ")`, `{" "}`). Cümle içi ayırıcılar (`{at} · {pages} …`) şablonun parçası —
  metin birebir.

**Doğrulama (ölçüldü)**

- **Kırmızı→yeşil:** `Providers.tsx`e `const _LEAK_SCREEN = "Sağlayıcı profili";` **ve**
  `arch.ts`e `export const _LEAK_LIB = "mimari farklı";` aynı anda → `localization`
  **1 failed | 8 passed**, rapor **iki dosyayı birlikte**:
  `[ "lib/arch.ts:14", "screens/Providers.tsx:87" ]`; geri alındı, `shasum -c` **birebir OK**
  → 9/9.
- **Yeni 6 test** (`i18n-bindings.test.ts`): mimari (rozet/en kötü/özet/uyarı/neden/shebang),
  paket (durum/özet/rol/uyum/uyarı/sınır cümlesi), saldırı (karar/fikstür/skip/ürün/komut/en
  kötü), yoklama (durum/özet/kapsam satırları/kalıntı yaşı/ölçüm özeti/süpürme/yetenek kapısı),
  **10 parametreli satırın hiçbiri `{` bırakmaz**, ve **ekran + dört lib'in `t()` ile çağırdığı
  281 benzersiz anahtarın (295 çağrı) hepsi sözlükte** (dosya bazlı tarama).
- Mevcut `arch` (15), `attack` (26), `package` (18), `providers` (18), `capability` (8)
  testleri **birebir `.toBe`** karşılaştırması yapıyor ve **değişmeden geçti** (yalnız
  `package.test.ts`'te `PACKAGE_LIMITS` → `packageLimits()` çağrısı güncellendi) → metinler
  byte düzeyinde aynı.
- `tsc --noEmit` temiz · `vitest` **489/489** (36 dosya; 483 → +6) · `vite build` temiz
  (**552,09 kB**; 537,32 → sözlük büyüdü, Vite 500 kB uyarısı sürüyor).
- Kalan yüzey **277 Türkçe satır / 11 dosya** (kapının kendi mantığıyla; 1 satırlıklar
  satır sonu yorumu: `api.ts`, `backup-schedule.ts`, `filename.ts`, `main.tsx`).

**Commit/CI:** `1949cf5` (kod, 9 dosya, +1124/−331) · **CI `41147576596` 7/7 yeşil**:
`arayüz` **36 dosya / 489 test** (tsc + vitest + build; yerel/CI **birebir**), `rust (macos-15)`
4 dk 09 s, `rust (ubuntu-latest)` 6 dk 05 s, `masaüstü kabuk` 31 s, `belge worker'ı` 37 s,
`SBOM` 24 s, `sözleşmeler` 7 s. `HEAD == origin/main`.

**Disk:** tur başında **3,6 G** (eşik altı) → güvenli önbellek temizliği (Chrome 355 M,
cargo registry src/cache 326 M, macOS duvar kağıdı videoları 2,0 G, GeoServices/helpd 85 M)
→ **6,2 G**; yalnız TS işi koştu (Rust derlemesi yok); tur sonu `disk-guard` temiz.

**Dürüst sınırlar:** **1 ekran + 1 bileşen** kaldı (`screens/Review.tsx` 100,
`components/DualReader.tsx` 93) + **4 yardımcı lib** (`intext` 35, `inventory` 32, `reader` 8,
`reviewTarget` 2) + `PageMap` 3 → F04-T08 **🚧**. `lib/providers.ts`'i `lib/inventory.ts` de
import ediyor (yalnız `formatBytes`); `inventory.ts` kendi metnini taşımaya devam ediyor.
Tauri penceresi açılmadı → **görsel QA yok** (JSX boşlukları `{" "}` ile açıkça yazıldı ama
ekranda görülmedi). Windows denenmedi.


## F04-T08 — sözlük taşıması: `screens/Precedents.tsx` + üç künye lib'i (2026-10-31, dokuzuncu tur)

**Önceki turun açık işi kapandı:** `edf986b` → CI `41131595392` **7/7 yeşil** (doğrulandı;
macOS işleri `macos-15` migrasyonundan sonra gerçekten koşuyor).

**Neden bu dilim:** kalan yüzeyde en büyük **tek ekran Emsal (136 satır)** ve metninin
**48 satırı** üç yardımcı lib'de duruyordu: `lib/precedents.ts` (26), `lib/precedent-terms.ts`
(15), `lib/precedent-document.ts` (7). ÖLÇÜLDÜ: bu üç lib'i **yalnız** `Precedents.tsx`
(kendi testleri dışında) import ediyor → dördü **kapalı bir küme**; ekranı tek başına taşımak
“temiz görünen ama metni lib'de kalan” dört dosya üretirdi (önceki turların ölçülmüş kusur
sınıfı). Bu turda **dört dosya birlikte** taşındı.

**Yazılanlar:** `lib/i18n.ts` + **4 yeni bölüm, 247 anahtar** (`precedents` 174,
`precedentLib` 37, `terms` 26, `precedentDoc` 10) → sözlük **944 → 1191** (`messageKeys()`
ile ölçüldü). `Precedents.tsx`: sayfa başlığı/lead/üç parçalı ipucu, kütüphane kartı (10 alan
etiketi + 4 yer tutucu + satır künyesi), toplu içe alma (düğmeler, 4 alan, sütun ipucu,
çakışma notu, liste hash'i), toplu korpus dağıtımı (kip, kapı rozeti, gerekçeler, sonuç),
kaynak kullanım koşulu (7 seçenek + 3 etiket + yazılı dayanak uyarısı + sorumluluk notu),
bibliyografya, kullanım notu, tam metin + alıntı doğrulama (aktarma bloğu, 3 seçim kutusu,
uyarılar, koşu etiketi), atıf kontrolü (6 alan + 3 rozet + anlam incelemesi + bağlam) ve
dış bağlayıcılar; 11 hata/dialog metni. **Modül sabitleri fonksiyona çevrildi**
(`EMPTY` → `emptyDraft()`, `MEANING_LABEL` → `meaningLabel(value)`): sabit harita sözlüğü
**import anında** okuyordu (Notlar turundaki `EMPTY_BODY_ERROR` dersinin aynısı).

**Ayırıcı kararı (sürüyor):** `" · "` ve baştaki tek boşluk sözlük **değerlerine** gömülmedi
(sözlük baş/son boşluk ve çift boşluk yasaklıyor); birleştirme kodda tek yerde:
`parts.join(" · ")`, `` ` ${t(...)}` ``. Cümle içi ayırıcılar (`{at} · {pages} sayfa …`)
şablonun parçası olarak kaldı — metin birebir korundu.

**Kapı kapsamı:** `MIGRATED` += 4 → **33** dosya.

**Kanıt (kırmızı→yeşil, ölçüldü):** `Precedents.tsx`e `_leak = "Emsal kütüphanesi"` **ve**
`precedent-terms.ts`e `_LEAK2 = "yeniden dağıtım izinsiz"` aynı anda → `localization.test.ts`
**1 failed | 8 passed**, rapor `["lib/precedent-terms.ts:18","screens/Precedents.tsx:74"]`;
ikisi de geri alındı, `sha256sum -c` **birebir OK** → **9/9**.

**Yeni 7 test** (`i18n-bindings.test.ts`): kullanım koşulu etiketleri; kapsam satırının
**parçalardan birleşmesi** (`join(" · ")` birebir); aktarım sonucu (sayı + hash); **alıntı
durumu cümlesi** (konum + tekrar + eski sürüm eki ile birlikte kurulur); tarama/kaynak/toplu
içe alma etiketleri; kasadan aktarma metinleri (4 engel gerekçesi, özet, 4 uyarı); **ekranda
`t()` ile çağrılan 175 benzersiz anahtarın hepsi sözlükte var** (186 çağrı — bazı anahtarlar
iki yerde). Mevcut `precedents.test.ts` /
`precedent-terms.test.ts` / `precedent-document.test.ts` **birebir** karşılaştırma yapıyor ve
değişmeden geçti → metinlerin byte düzeyinde aynı olduğunun kanıtı.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **483/483** (36 dosya; 476 → +7) · `vite build`
temiz (**537,32 kB**; önceki 525,36 kB — Vite 500 kB uyarısı sürüyor). Taşınmış dört dosyada
yorum dışı **0** Türkçe harf satırı (kapının kendi mantığıyla ölçüldü).

**Commit/push:** `257d3fd` (8 dosya, +1030/−281) → `origin/main == 257d3fd`.

**CI `41139371524` — 7/7 YEŞİL (ölçüldü):** `sözleşmeler` 6 s · `arayüz`
**36 dosya / 483 test** + tsc + build (30 s) · `SBOM` 30 s · `belge worker'ı` 30 s ·
`masaüstü kabuk` 34 s · **`rust (macos-15)` 4 dk 52 s** · **`rust (ubuntu-latest)` 9 dk**.
Yerel ve CI test sayısı **birebir** (483/483, 36 dosya).

**Dürüst sınırlar:** **2 ekran + 1 bileşen** kaldı (Providers 110, Review 100,
`components/DualReader` 93) + **8 yardımcı lib** (`providers` 42, `arch` 36, `intext` 35,
`inventory` 32, `package` 31, `attack` 24, `reader` 8, `reviewTarget` 2) → F04-T08 **🚧**.
Metinler birebir aynı; Tauri penceresi açılmadı → **görsel QA yok** (JSX parçalarının
boşlukları `{" "}` ile açıkça yazıldı ama ekranda görülmedi). Windows denenmedi.


## F04-T08 — sözlük taşıması: `screens/Cases.tsx` + `lib/ocr.ts` (2026-10-31, sekizinci tur)

**Önceki turun açık işi kapandı:** `c669f53` → CI `41120779239` yalnız iki macOS işi
0 s'de düştü (brownout); bu turda **`ci.yml` `macos-14` → `macos-15`** taşındı (matris
satır 52 + `masaüstü kabuk` satır 74). Önceki turların `ci.yml`'a dokunamama gerekçesi
(başka ajanın staged değişikliği) **ortadan kalktı**: o iş `39a1adb` ile commit edildi,
çalışma ağacı temiz.

**Neden bu dilim:** kalan yüzeyde en küçük ekran **Dosyalar/Cases (60 satır)**; önceki
turlar bu dosyayı başka ajanın OCR dil işi yüzünden bilinçli atlıyordu. Ölçüldü:
`Cases.tsx` metninin bir kısmı **`lib/ocr.ts`**ten geliyor (dil adları: Türkçe/İngilizce/
Almanca/Fransızca) ve o lib'i **yalnız** `Cases.tsx` import ediyor → ikisi birlikte taşındı
(yoksa OCR ipucu taşınmış ekranın dışında Türkçe kalmaya devam ederdi).

**Yazılanlar**

- `lib/i18n.ts` + **82 anahtar** (`cases` 78, `ocrLang` 4) → sözlük **862 → 944**
  (`messageKeys()` ile ölçüldü).
- `screens/Cases.tsx`: sayfa başlığı (`nav.files`), lead (INV-05), liste başlığı/alt satırı
  (`{count} kayıt · {scope}` + `arşiv dahil`/`yalnız aktif`), arşiv onay kutusu, yeni
  dosya/vazgeç, form (başlık/tür/esas/alan + iki yer tutucu + tür seçenekleri
  `labels.caseKind.*`), boş liste (iki durum), satır künyesi (`referans yok`/`alan
  belirtilmedi`), seçili dosya künyesi (yedi `dt`), üç mutasyon düğmesi, ilerleme
  `aria-label`, kaynak alıntılar kartı (başlık/alt satır/boş durum/`title`/okuyucuda aç),
  belgeler kartı (başlık/alt satır/toplu ayrıştırma `title`/belge ekle/boş durum/etiket
  `title` + `aria-label` + yer tutucu/Oku/Ayrıştır/OCR), toplu sonuç bloğu, kodlama
  seçimi (4 seçenek + ipucu), PDF parolası (etiket + yer tutucu) ve dipnot.
  Parametreli: `{count}`, `{scope}`, `{name}`, `{version}`, `{status}`, `{pages}`,
  `{segments}`, `{peak}`, `{succeeded}`, `{requested}`, `{tag}`, `{quote}`, `{title}`,
  `{lang}`. **Ayırıcılar (`" · "`) sözlükte değil kodda**: sözlük baş/son boşluk
  yasaklıyor ve parça birleştirme `parts.join(" · ")` ile tek yerde.
- `lib/ocr.ts`: dört dil adı `ocrLang.*`ten; `DEFAULT_OCR_LANG` ("tur+eng") **kaynak
  sabiti olarak kaldı** (Rust `queue.rs` ve Python `engines.py` aynası; `ocr.test.ts`
  üçünü karşılaştırmaya devam ediyor).
- **Kapı kapsamı:** `MIGRATED` += `screens/Cases.tsx`, `lib/ocr.ts` → **29** dosya.

**Kanıt (kırmızı→yeşil, ölçüldü)**

- `Cases.tsx`e `const _leak = "İnceleme dosyaları";` **ve** `ocr.ts`e
  `export const _LEAK2 = "Türkçe";` aynı anda → `localization.test.ts`
  **1 failed | 8 passed**, rapor iki dosyayı birlikte: `["lib/ocr.ts:16",
  "screens/Cases.tsx:35"]`; ikisi de geri alındı → `sha256sum -c` **birebir OK** → **9/9**.
- **Yeni 3 test** (`i18n-bindings.test.ts`): Dosyalar satırları parametreleri doldurur
  (`listSubtitle` iki kapsamla farklı, `parseSummary`/`batchDetail`/`batchResult` yarım
  `{` bırakmaz, `imported`/`removeTagTitle`/`tagAria` birebir) · OCR ipucu dil adını
  sözlükten alır (`Taranmış sayfalar için OCR (Türkçe + İngilizce)`) · **ekranda geçen
  her `cases.*` anahtarı sözlükte var** (78 anahtar; silinen anahtar ekranı çalışma
  zamanında patlatırdı). `ocr.test.ts` dil adlarını artık `t("ocrLang.*")` ile
  karşılaştırıyor (harfsiz "OCR" sözcüğünü kapı yakalayamaz, bağ testi kapatır).
- **Kendi hatam (test yakaladı, uygulama doğruydu):** anahtar-varlık testini önce
  `t(key)` çağırarak yazdım; parametre isteyen anahtarlar bilinçli olarak hata attığı
  için test kırmızı oldu → `messageKeys()` kümesiyle karşılaştırıldı. Kod değişmedi.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **476/476** (36 dosya; 473 → +3 net) ·
`vite build` temiz (**525,36 kB**; önceki 522,08 kB — sözlük büyüdü, Vite 500 kB uyarısı
sürüyor) · taşınmış iki dosyada yorum dışı **0** Türkçe harf satırı (betikle).

**Kalan yüzey:** **3 ekran + 1 bileşen** (Precedents 136, Providers 110, Review 100,
DualReader 93) + **9 yardımcı lib** (providers 42, arch 36, intext 35, inventory 32,
package 31, precedents 26, attack 24, precedent-terms 15, precedent-document 7, reader 8)
→ F04-T08 **🚧**. Metinler birebir aynı; Tauri penceresi açılmadı → **görsel QA yok**;
Windows denenmedi.

**CI (bu tur, ölçüldü):** `1bbe35d` + `9f34ac1` → koşu **`41130514700` 7/7 YEŞİL**
(sözleşmeler 7 s · SBOM 26 s · `belge worker'ı` 31 s · `arayüz` **36 dosya / 476 test**
(34 s) · `rust (ubuntu-latest)` 5 dk · **`rust (macos-15)` 7 dk 48 s** ·
**`masaüstü kabuk (cargo check)` 2 dk 8 s**). macOS işleri **ilk kez gerçekten koştu**:
`macos-14` brownout'u (30 Eki 14:00 – 31 Eki 00:00 UTC) `macos-15`e taşınınca bitti.
Yerel/CI test sayısı **birebir** (476/476, 36 dosya) — önceki turlardaki fark başka ajanın
commit edilmemiş `ocr.test.ts` dosyasıydı, artık `39a1adb` ile depoda.

## F04-T08 — sözlük taşıması: `components/Notes.tsx` + `lib/notes.ts` + `lib/vault-input.ts` (2026-10-30, yedinci tur)

**Neden bu dilim:** kalan yüzeyde en küçük ekran **Notes (35)**; `Cases.tsx` (60) hâlâ
başka ajanın OCR dil işinde değişiklik taşıyor → çakışmamak için seçilmedi. **Ölçüldü:**
`Notes.tsx` kendi metnini taşımıyordu ama taslak/revizyon etiketleri, çakışma uyarısı ve
"bilinmeyen kaynak" **`lib/notes.ts`**ten (5 satır) geliyordu ve o lib'i **yalnız**
`Notes.tsx` import ediyor → ikisi birlikte taşındı. Aynı turda **`lib/vault-input.ts`**
(5 satır) de taşındı: o lib'i **yalnız** `screens/Unlock.tsx` import ediyor ve Unlock
**altıncı turdan beri taşınmış** — yani parola hata iletileri taşınmış ekranın dışında
kalıyordu (gerçek sızıntı, mekanik tamamlama).

**Yazılanlar**

- `lib/i18n.ts` + **54 anahtar** (`notes` 49, `vaultInput` 5) → sözlük **808 → 862**
  (`messageKeys()` ile ölçüldü).
- `components/Notes.tsx`: kart başlığı/alt satırı (`{count} not · …`), boş alıntı uyarısı,
  başlık/gövde etiketleri + yer tutucu, üç taslak durumu satırı, düğmeler (taslağı sil,
  notu ekle, düzenle, sil, kapat, taslağı yükle/koru, geri/ileri al, sürüm geçmişi,
  kaydet = `common.save`), `title` metinleri, çakışma bloğu (başlık, ipucu, sayaç,
  iki karar düğmesi, güncel sürüm satırı), revizyon geçmişi (başlık + boş durum) ve alt
  bilgi notu. Parametreli metinler `t(key,{…})`: `{count}`, `{version}`, `{revision}`,
  `{time}`, `{mine}`, `{theirs}`. **`EMPTY_BODY_ERROR` sabiti fonksiyona çevrildi**
  (`emptyBodyError()`) — modül düzeyi sabit sözlüğü import anında okuyordu, hata iletisi
  çağrı anında kurulur.
- `lib/notes.ts`: `DRAFT_STATUS_LABEL` (4) ve `REVISION_ORIGIN_LABEL` (4) sözlükten
  türetilir; `revisionOriginLabel` bilinmeyen kaynağı da sözlükten verir; `CONFLICT_HINT`
  tek kaynak.
- `lib/vault-input.ts`: beş gerekçe sözlükten; kısa parola iletisi
  `t("vaultInput.tooShort", { min: MIN_PASSWORD_LENGTH })` — **sabit metinden çıktığı
  için uzunluk artık iki yerde ayrışamaz** (önce `` `Parola en az ${MIN_PASSWORD_LENGTH} …` ``
  gömülüydü).
- **Kapı kapsamı:** `MIGRATED` += `components/Notes.tsx`, `lib/notes.ts`,
  `lib/vault-input.ts` → **27** dosya.

**Kanıt (kırmızı→yeşil, ölçüldü)**

- `components/Notes.tsx`e `<p>Gözlem notu</p>` → `localization.test.ts` **1 failed |
  8 passed**, rapor `[ "components/Notes.tsx:784" ]`; geri alındı (sha256 **birebir**) → **9/9**.
- `lib/vault-input.ts`e `_LEAK = "parola çok kısa"` → **1 failed | 8 passed**, rapor
  `[ "lib/vault-input.ts:45" ]`; geri alındı (sha256 **birebir**) → **9/9**.
- **`lib/notes.ts` — kapının sınırı bu turda ikinci kez ölçüldü:** `_LEAK = "taslak
  kaydedildi"` sızıntısı **yakalanmadı** (kapı yeşil): metinde `ığüşöçİĞÜŞÇ` yok (bilinen
  sınıf, altıncı turda da yazıldı). Türkçe harf taşıyan `"sürüm geçmişi"` ile aynı deneme
  → **1 failed | 8 passed**, rapor `[ "lib/notes.ts:150" ]`. **Harf taşımayan gömülü
  metin** için ikinci kapı ölçüldü: `idle: t("notes.draftIdle")` yerine düz
  `idle: "Taslak yok."` → `localization` **kırmızı** ("sözlükte kullanılmayan anahtar:
  `notes.draftIdle`") **ve** `i18n-bindings` **kırmızı** (`'Taslak yok.' ≠ 'Taslak yok'`).
  Yani harf kapısının kaçırdığı sınıfı bu dosyada sözlük-bağı testi kapatıyor; **birebir
  aynı metni geri gömmek** ise yalnız "kullanılmayan anahtar" kuralıyla yakalanır
  (değer eşit olduğu için bağ testi yeşil kalır) — açık kayıt.

**Yeni testler (+4):** `notes.test.ts` etiketleri artık sözlükle karşılaştırıyor
(literal metin yerine `t(key)`), `vault-input.test.ts` beş gerekçenin hepsini sözlükten
doğruluyor (parametreli ileti `{` bırakmaz, uzunluğu taşır), `i18n-bindings.test.ts`e üç
test: (1) iki modül sabit haritası + bilinmeyen kaynak + `CONFLICT_HINT` birebir sözlükten,
(2) altı parametreli Notlar satırı (`{mine}/{theirs}/{version}/{revision}/{time}/{count}`)
doluyor ve `{` kalmıyor, (3) beş parola gerekçesi sözlükten + güçlü parola kabul.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **473/473** (36 dosya; 470 → +3 net) ·
`pnpm build` temiz (**522,08 kB**; önceki 520,11 kB — sözlük büyüdü, Vite 500 kB uyarısı
sürüyor). Taşınmış üç dosyada yorum dışı **0** Türkçe harf satırı (kapının kendi
mantığıyla ölçüldü); kalan yüzey **812 → 767 satır / 25 → 22 dosya**.

**Dürüst sınırlar:** **4 ekran + 1 bileşen** taşınmadı (Precedents 136, Providers 110,
Review 100, DualReader 93, Cases 60 — sonuncusu başka ajanın dosyası) ve **9 yardımcı lib**
kendi metnini taşıyor (providers 42, arch 36, intext 35, inventory 32, package 31,
precedents 26, attack 24, precedent-terms 15, precedent-document 7, reader 8, ocr 3)
→ F04-T08 **🚧**. Metinler birebir aynı; Tauri penceresi açılmadı → **görsel QA yok**;
Windows denenmedi.

**Commit/CI:** aşağıdaki "CI" satırında ölçülen koşu.

## F04-T08 — sözlük taşıması: Ayarlar ekranı + `lib/rollback.ts` (2026-10-30, altıncı tur)

**Önceki turun açık işi kapandı:** `e036ccd` → CI **`41093744784` 7/7 yeşil** (doğrulandı).

**Neden bu dilim:** kalan yüzeyde en küçük ekranlar **Ayarlar (60)** ve **Cases (60)**;
`Cases.tsx` başka ajanın OCR dil işinde değişiklik taşıyor → çakışmamak için **Ayarlar**
seçildi. Ölçüldü: ekranın metninin **17 satırı** `lib/rollback.ts`te ve o lib'i **yalnız**
`Settings.tsx` import ediyor → ikisi birlikte taşındı (yoksa "temiz görünen ama metni
lib'de kalan" dosya kalırdı). `lib/backup-schedule.ts` **bilinçli olarak kapsam dışı**:
tek Türkçe harfi satır **sonu** yorumunda, kapı satır sonu yorumlarını ayıklamıyor.

**Yazılanlar**

- `lib/i18n.ts` + **95 anahtar** (`settings` 76, `rollback` 19) → sözlük **713 → 808**.
- `screens/Settings.tsx`: başlık/lead, Kasa kartı (rozet + kimlik/konum/şema/sürüm),
  güncelleme öncesi yedek bildirimi, geri dönüş kartı (tarama, boş durum, radyo
  rozetleri, onay etiketi, sonuç bloğu), yedek al, otomatik yedek, kullanılmayan
  veriler (GC), geri yükleme, parola, teşhis + **üç hata iletisi** + **üç klasör seçici
  başlığı**. Parametreli metinler `t(key,{…})`: `{version}`, `{count}`, `{size}`,
  `{hash}`, `{dir}`, `{days}`, `{files}`, `{ms}`, `{token}`.
- `lib/rollback.ts`: onay belirteci, rozet, durum satırı, ad uyuşmazlığı uyarısı,
  **altı sınır satırı**, dört düğme gerekçesi ve sonuç cümlesi sözlükten.
  `ROLLBACK_CONFIRMATION` artık `t("rollback.confirmToken")`den türer → arayüz
  tarafında **tek kaynak** (önceden: lib sabiti + ekran etiketi + Rust sabiti).
- **Kapı kapsamı:** `MIGRATED` += `screens/Settings.tsx`, `lib/rollback.ts` → **24** dosya.

**Ölçülen kusur #76 (yanlış sürüm etiketi):** Ayarlar "Sürüm" satırı gömülü
`0.2.0 (pre-alpha)` yazıyordu; uygulama `package.json` **0.1.0** ve kabuk `v0.1.0`
gösteriyor → aynı ürün iki sürüm yazıyordu. Düzeltme:
`import { version } from "../../package.json"` + `settings.versionValue`. Kapıya test
eklendi: Ayarlar sürümü paketten okur, gövdesinde `0.2.0` **yok**. (Kök `0.2.0` ↔
uygulama `0.1.0` uyuşmazlığı **ayrı iş** olarak duruyor; bu tur onu hizalamadı.)

**Kanıt (kırmızı→yeşil, ölçüldü)**

- `lib/rollback.ts`e `_LEAK2 = "doğrulanmadı"` → `localization.test.ts`
  **1 failed | 8 passed**, rapor `[ "lib/rollback.ts:13" ]`; geri alındı → **9/9**.
- `Settings.tsx`e `_leak = "Güncelleme öncesi yedek"` → **1 failed | 8 passed**,
  rapor `[ "screens/Settings.tsx:34" ]`; geri alındı (sha256 **birebir**) → **9/9**.
- **Kapının dürüst sınırı (ölçüldü):** aynı deneme `"Ayarlar ve yedek"` metniyle
  yapıldığında kapı **yeşil kaldı** — o metinde `ığüşöçİĞÜŞÖÇ` yok. Kapı harf tabanlı
  olduğu için Türkçe harf taşımayan gömülü metni kaçırır (bilinen sınıf; `i18n-bindings`
  yalnız **üretilen** metnin sözlükten geldiğini bağlar). Açık kayıt.

**Yeni 5 test:** rozet/durum satırı sözlükten; altı sınır + dört gerekçe + sonuç cümlesi
sözlükten (parametreler dolar, `{` kalmaz); **onay belirteci `commands.rs`teki
`ROLLBACK_CONFIRMATION` sabitiyle birebir** (test Rust dosyasını okur — sözlük kayması
geri dönüşü sessizce öldüremez); Ayarlar parametreli metinleri; Ayarlar sürümü paketten.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **470/470** (36 dosya; 465 → +5) ·
`vite build` temiz (**520,11 kB**; önceki 515,86 kB — Vite 500 kB uyarısı sürüyor).

**Dürüst sınırlar:** **4 ekran + 2 bileşen** taşınmadı (Precedents 136, Providers 110,
Review 100, DualReader 93, Cases 60, Notes 35) ve **12 yardımcı lib** kendi metnini
taşıyor (providers 42, arch 36, intext 35, inventory 32, package 31, precedents 26,
attack 24, precedent-terms 15, reader 8, precedent-document 7, vault-input 5, notes 5)
→ F04-T08 **🚧**; kalan yüzey **812 satır / 25 dosya** (bir kısmı satır sonu yorumu:
`api.ts`, `main.tsx`, `filename.ts`, `backup-schedule.ts`). Metinler birebir aynı;
Tauri penceresi açılmadı → **görsel QA yok**; Windows denenmedi.

**Commit/CI:** `5697d9e` (6 dosya, +432/−119) push edildi; `HEAD == origin/main`.

- CI **`41107936580`**: **5 iş yeşil** — `arayüz (tsc+vitest+build)` 35 dosya /
  **462 test** (yeni `i18n-bindings` **19**, `localization` **9**), `rust (ubuntu-latest)`,
  `belge worker'ı`, `SBOM`, `sözleşmeler`. **2 iş (rust macos-14 + masaüstü kabuk)
  başlamadan (0 s) düştü** — neden **GitHub macOS-14 brownout'u** (30 Eki 14:00 –
  31 Eki 00:00 UTC; imaj **2 Kasım 2026**'da emekli). Yani CI **kırmızı ama sebep
  altyapı**; bu değişikliğin kapsadığı TS işi **yeşil**. Düzeltme `macos-14` →
  `macos-15`/`macos-latest`; `ci.yml` başka ajanın staged değişikliğiyle **aynı dosyada**
  olduğu için **dokunulmadı** (TEK AJAN KURALI) → sıradaki turların ilk maddesi.
- **Yerel/CI test sayısı farkı (dürüst kayıt):** yerel 470/36 dosya, CI 462/35 dosya;
  fark **başka ajanın commit edilmemiş `src/lib/ocr.test.ts`** dosyası (8 test).

**Dokunulmayanlar:** başka ajanın **18 dosyalık** OCR dil işi (staged) aynen duruyor;
commit path-limited (yalnız bu turun 6 dosyası).

**Disk:** tur başında **5,9 G**, tur sonu **5,6 G**; `disk-guard` temiz.

## F04-T08 — sözlük taşıması: Arama ekranı + iki kapsam lib'i (2026-10-30, beşinci tur)

**Önceki turun açık işi kapandı (kırmızı CI düzeltildi):** dördüncü turun `0696aac`
commit'i CI `40989194507`'de **kırmızıydı** — `arayüz (tsc + vitest + build)` işi
`i18n-bindings.test.ts(172-174) TS2532` ile düştü: `noUncheckedIndexedAccess`
altında `rows[0].label`, `rows[rows.length-1].label` ve `COVERAGE_EMBED_TARGETS[0].label`.
Yani dördüncü turun "tsc temiz" kaydı **yanlıştı** (test dosyası yazıldıktan sonra
tsc koşulmamış). Düzeltme `c94dd30`: indeks erişimleri `?.` ile, üstüne **boş liste
kapısı** (`rows.length > 1`, `COVERAGE_EMBED_TARGETS.length > 0`) → CI
**`41091865894` 7/7 yeşil** (doğrulandı). Hata yerelde **birebir** yeniden üretildi
(`npx tsc --noEmit` → aynı üç satır) ve düzeltmeden sonra temizlendi.

**Neden bu dilim:** kalan ekranların en küçüğü **Arama** (45 Türkçe satır) ama metnin
**32 satırı** iki yardımcı lib'de: `lib/search.ts` (24) + `lib/search-scope.ts` (8).
Ölçüldü: ikisini **yalnız** `Search.tsx` import ediyor (başka ekran yok) → üçü birlikte
taşındı; yoksa "temiz görünen ama metni lib'de kalan" bir dosya kalırdı.

**Yazılanlar**

- `lib/i18n.ts` + **115 anahtar** (`searchFilters` 39, `search` 64, `searchScope` 12)
  → sözlük **598 → 713**.
- `screens/Search.tsx`: başlık `nav.search`dan; kapsam düğmeleri ve kalıcı kapsam
  uyarısı, dosya/metin/kip alanları, filtre etiketleri, etiket çipleri (ipucu dahil),
  iki boş durum, "sonuçlar önceki ayara ait" uyarısı, sonuç ve emsal kartı başlıkları,
  düğme/`title` metinleri, `E_SEARCH_VERSION` ve `E_CLIPBOARD` iletileri sözlükten.
  Parametreli metinler `t(key,{…})`: `{size}`, `{count}`, `{title}`, `{page}`,
  `{segment}`, `{version}`, `{summary}`, `{mode}`, `{query}`, `{label}`, `{position}`,
  `{date}`.
- `lib/search.ts`: tarih rolü / OCR durumu / metin kaynağı etiketleri, filtre açıklama
  satırları, sonuç özeti, ertelenmiş toplamın **dört** durumu, aday penceresi satırı,
  kip etiketleri ve kip ipuçları `t()`den.
- `lib/search-scope.ts`: kapsam etiketleri + uyarıları, künye satırı (`E.`/`K.`/tarih),
  eşleşme kaynağı, sonuç özeti ve **kopyalama metni** — üç satırlık şablon **tek**
  anahtar (`{citation}\n…{snippet}\n(kaynak: emsal kütüphanesi, sürüm {version})`).
- **Kapı kapsamı:** `MIGRATED` += `screens/Search.tsx`, `lib/search.ts`,
  `lib/search-scope.ts` → taşınmış **22** dosya taranıyor.

**Kanıt (kırmızı→yeşil, ölçüldü):** aynı anda `Search.tsx`e
`const _leak = "Arama sonuçları";` ve `search.ts`e `export const _LEAK2 = "eşleşme";`
→ `localization.test.ts` **1 failed | 7 passed**, rapor **iki dosyayı birlikte**:
`[ "lib/search.ts:16", "screens/Search.tsx:49" ]`; geri alındı → **8/8**.

**Yeni 7 test** (`i18n-bindings.test.ts`): kip/filtre etiketleri (bilinmeyen metin
kaynağı sabit metne düşer), filtre satırları (parametreler dolar, `{` kalmaz), sonuç/
toplam/doğrulama sayaç satırları (bayat sorgu sayı göstermez), aday penceresi
(kesildi/tükendi), kapsam + künye + eşleşme kaynağı, emsal özeti + kopyalama şablonu
(**üç satır** korunur), kip ipucunun otomatik çözümü.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **465/465** (36 dosya; 458 → +7) ·
`vite build` temiz (**515,86 kB**; önceki 510,43 kB — Vite'ın 500 kB uyarısı sürüyor).

**Kendi kusurlarım (testler yakaladı; uygulama doğruydu):** (a) `describeFilters`
türleri `canonicalFilters` yüzünden **sıralı** döndürür — beklentiyi sırasız yazmıştım;
(b) `SearchVerification.windows` zorunlu alanı fixture'da yoktu; (c) "Künye eksik"
vakasında yalnız mahkeme alanını boşaltmıştım, künye numaraları dolu kaldığı için satır
yine yazıldı. Üçü de **test literali** hatasıydı, kod değişmedi.

**Kendi hatam (kayıt):** kanıt için yedek alırken `/tmp/Search.bak` ile `/tmp/search.bak`
macOS'un **büyük/küçük harf duyarsız** biriminde **aynı dosya** oldu → `Search.tsx`
`search.ts` içeriğiyle ezildi. Dosya `git show HEAD:`dan geri alındı ve **39 parça**
birebir (her parça "tam 1 kez bulundu" kapısıyla) yeniden uygulandı; sonuç `tsc`,
`vitest` ve Türkçe harf sayımıyla yeniden doğrulandı. Ders: yedek adı **ayırt edici**
olmalı (`SearchScreen.bak` / `searchLib.bak`).

**Dürüst sınırlar:** hâlâ **5 ekran + 2 bileşen** taşınmadı (Precedents 136, Providers 110,
Review 100, DualReader 93, Cases 60, Settings 60, Notes 35) ve **8 yardımcı lib**
(`providers` 42, `arch` 36, `intext` 35, `inventory` 32, `package` 31, `precedents` 26,
`attack` 24, `rollback` 17) kendi metnini taşıyor → F04-T08 **🚧**. Kalan yüzey
**966 → 889** Türkçe satır (`i18n.ts` hariç, kapının kendi mantığıyla). Metinler
**birebir aynı**; ayırıcılar (`" · "`, `{" "}`) JSX'te kaldı. Tauri penceresi açılmadı →
**görsel QA yok**; Windows denenmedi. `t()` **import anında** çözülür (tek dilli ürün
kararı). `search-scope` kopyalama şablonundaki `\n` sözlük değerinde; test üç satırı
doğruluyor.


## F04-T08 — sözlük taşıması: iş kuyruğu/bütçe kartları + beş yardımcı lib (2026-10-30, dördüncü tur)

**Neden:** önceki turun sıradaki adayı "Search (45) / Settings (60) / Cases (60)" ya da
"küçük aile" idi. Küçük aile seçildi çünkü asıl açık, **taşınmış ekranların metni hâlâ
yardımcı lib'lerden almasıydı**: `Exports.tsx` kapsam cümlelerini `lib/export-coverage.ts`ten,
`Tasks.tsx` hatırlatıcı etiketlerini `lib/reminders.ts`ten, `Redaction.tsx` ilerleme
aşamalarını `lib/parseProgress.ts`ten okuyordu — kapı bu dosyaları taramıyordu.

**Ölçüm (kapının kendi mantığı, yorumlar ayıklanmış):** kalan toplam **1079** Türkçe satır /
37 dosya. Bu dilimin yüzeyi: `lib/jobs.ts` **42**, `components/JobQueueCard.tsx` **19**,
`lib/export-coverage.ts` **17**, `components/WorkerBudgetCard.tsx` **13**, `lib/budget.ts`
**8**, `lib/parseProgress.ts` **7**, `lib/reminders.ts` **7** = **113**.

**Bağ (ölçüldü, varsayılmadı):** `lib/jobs.ts` yalnız `JobQueueCard`'ı besliyor;
`lib/parseProgress.ts` → `jobs.ts` + **`Redaction.tsx`** (taşınmış) + `Cases.tsx`;
`lib/budget.ts` → `WorkerBudgetCard` + `Cases.tsx`; `lib/export-coverage.ts` → `Exports.tsx`;
`lib/reminders.ts` → `Tasks.tsx`. Yani beş lib birlikte taşınınca **üç taşınmış ekranın**
metin kaynağı da sözlüğe girdi.

**Yazılanlar**

- `lib/i18n.ts` + **179 anahtar**: `jobs` (76), `exportCoverage` (30), `budget` (21),
  `jobQueue` (17), `workerBudget` (16), `parseProgress` (12), `reminders` (7).
  Sözlük **419 → 598** anahtar.
- `lib/jobs.ts`: 14 durum etiketi (`succeeded`/`cancelled`/`leased` eski adları dahil),
  **küçük harfli** özet biçimleri (`stateLower` — `toLocaleLowerCase("tr")` çağrısı kalktı,
  biçim sözlükte), tür/engel/eylem etiketleri, iptal·dikkat·kurtarma·yeniden deneme
  cümleleri `t()`den.
- `lib/parseProgress.ts`: 9 aşama etiketi + "Ayrıştırılıyor" + sayaç satırı.
- `lib/budget.ts`: MiB/sn/ms metinleri `t()`den; **ondalık biçimi artık
  `lib/format.ts`** (`formatDecimal`, gruplamasız) — burada `replace(".", ",")` kalmadı.
- `lib/export-coverage.ts`: 10 satır etiketi, özet, 3 rozet + ayrıntı, gömme hedefleri.
- `lib/reminders.ts`: bildirim gövdesi + ilke/etiket haritaları.
- `components/JobQueueCard.tsx`, `components/WorkerBudgetCard.tsx`: tüm başlık, alt yazı,
  tablo başlığı, düğme, boş durum, sınır paragrafı ve `aria-label` sözlükten (JSX içindeki
  `<code>` parçaları korundu; cümle parçaları `subLead/subTail`, `limitLead/Middle/Tail`).
- **Kapı kapsamı:** `MIGRATED` += 7 dosya → taşınmış **19** dosya taranıyor. Ayrıca yeni
  `lib/i18n-bindings.test.ts` (**8 test**): üretilen metnin **birebir sözlükten** geldiğini
  doğrular (kapı yalnız Türkçe *harf* aradığı için `bellek`, `dosya`, `OCR` gibi harfsiz
  sözcükler kaçardı).

**Kanıt (kırmızı→yeşil, ölçüldü)**

- Aynı anda `JobQueueCard.tsx`e `const _leak = "Kuyruk okunamadı";` ve `lib/budget.ts`e
  `export const _LEAK2 = "Bütçe aşıldı";` → `localization.test.ts` **1 failed | 7 passed**,
  rapor iki dosyayı birlikte gösterdi: `[ "components/JobQueueCard.tsx:28", "lib/budget.ts:17" ]`;
  satırlar geri alındı → **8/8**.
- **Kendi kusurum (test yakaladı):** `profileLimits` içindeki "süre" satırından birimi
  (`sn`) düşürmüştüm → `budget.test.ts` **kırmızı** (`"süre 300 sn"` beklenirken
  `"süre 300"`). Sözlük değeri `süre {value} sn` yapıldı → yeşil. Metin taşırken birim
  kaybı sınıfı: **sözlük bağı testi** bunu kalıcı olarak yakalıyor.
- **Ölçüm:** `tsc --noEmit` temiz · `vitest` **458/458** (36 dosya; 450 → +8) ·
  `vite build` temiz (**510,43 kB**; önceki 502,48 kB) · taşınmış 7 dosyada yorum dışı
  **0** Türkçe harf satırı (betikle; toplam kalan **1079 → 966**).

**Dürüst sınırlar:** hâlâ **6 ekran + 2 bileşen** taşınmadı (Precedents 136, Providers 110,
Review 100, DualReader 93, Cases 60, Settings 60, Search 45, Notes 35) → F04-T08 **🚧**.
`lib/providers.ts` (42), `lib/arch.ts` (36), `lib/intext.ts` (35), `lib/inventory.ts` (32),
`lib/package.ts` (31) taşınmış ekranların bazı satırlarını hâlâ besliyor ve kapı o dosyaları
taramıyor. Metinler **birebir aynı** (yalnız kaynak değişti; JSX'teki `<code>` parçaları
korundu, cümle parçaları ayrı anahtarlarda). `formatDecimal` yeni: MiB satırının ondalık
biçimi artık `Intl` üzerinden (önce `String(...).replace(".", ",")`); **100 MiB ve üstünde
tam sayı** kuralı korundu. Sözlük modül düzeyinde kurulduğu için `t()` **import anında**
çözülür (tek dilli ürün kararı). Tauri penceresi açılmadı → **görsel QA yok**; Windows
denenmedi. Vite'ın **500 kB** chunk uyarısı sürüyor (hata değil; kod bölme insan kararı).


## F04-T08 — sözlük taşıması: AI ekranı + aday/kapsam yardımcıları (2026-10-30, üçüncü tur)

**Neden:** kalan iş listesindeki diğer maddeler insan/donanım bekliyor (F17/F02 Windows,
Kimi/Antigravity hesabı, notarization, pilot). Kodla ölçülebilir tek sınıf F04-T08'di.
Sıradaki en küçük ekran **AI** ölçüldü (578 satır / **51** Türkçe satır) ama ekran tek
başına taşınsaydı metnin çoğu iki yardımcı lib'de kalırdı: `lib/candidate.ts` **38**,
`lib/transferScope.ts` **7** Türkçe satır — ikisi de yalnız bu ekran tarafından
kullanılıyor (ölçüldü: başka ekran import etmiyor). Üçü birlikte taşındı.

**Yazılanlar**

- `lib/i18n.ts` + **121 yeni anahtar**: `ai` (63), `candidate` (51), `transferScope` (7).
  Sözlük toplamı **298 → 419** anahtar.
- `screens/Ai.tsx`: başlık `nav.aiReview`dan; tüm alan etiketi, düğme, tablo başlığı,
  uyarı, boş durum, bildirim ve "doğrulamanın sınırları" metni sözlükten. Parametreli
  metinler `t(key,{…})`: `{count}`, `{page}`, `{documents}`, `{segments}`, `{bytes}`,
  `{state}`, `{id}`, `{provider}`.
- `lib/candidate.ts`: durum/etiket/konum/özet/engel metinleri `t()`den;
  `CANDIDATE_KINDS` (tür etiketi + ipucu + **örnek JSON şablonu**) modül düzeyinde
  sözlükten kurulur.
- `lib/transferScope.ts`: kapsam dışı neden etiketleri, "Gönderilmeyen N belge: …" ve
  onay kapsamı satırı sözlükten (`formatSize` çağrısı korunur — biçim tek katmanda).
- **Kapı kapsamı:** `localization.test.ts` `MIGRATED` += `screens/Ai.tsx`,
  `lib/candidate.ts`, `lib/transferScope.ts` → taşınmış **12** dosya taranıyor.

**Kanıt (kırmızı→yeşil, ölçüldü)**

- Aynı anda `Ai.tsx`e `const _leak = "Aday doğrulama paneli";` ve `candidate.ts`e
  `export const _LEAK2 = "geçerli oldu";` → `localization.test.ts` **1 failed | 7 passed**,
  rapor iki dosyayı birlikte gösterdi: `[ "lib/candidate.ts:80", "screens/Ai.tsx:34" ]`;
  satırlar geri alındı → **8/8**.
- Yeni **3 test** (toplam +5 assert grubu): (a) `candidate.ts` etiket/ipucu/şablon
  metinlerinin **birebir sözlükten** geldiği (metin lib'e geri gömülürse kırmızı),
  (b) `transferScope.ts` neden etiketleri + parametreli satırlarda `{` kalmadığı,
  (c) modül düzeyi sabitlerin (`CANDIDATE_KINDS`) sözlükten kurulduğu.

**Test/derleme:** `tsc --noEmit` temiz · `vitest run` **450/450** (35 dosya; 445 → +5) ·
`vite build` temiz (**502,48 kB**; önceki 497,44 kB — sözlük büyüdü, Vite'ın 500 kB
uyarısı **artık tetikleniyor**, hata değil). Taşınmış **12** dosyanın tümünde yorum dışı
**0** Türkçe harf satırı (betikle sayıldı: 12/12).

**Dürüst sınırlar**

- **6 ekran + 4 yardımcı bileşen taşınmadı:** Dosyalar/Cases (60 Türkçe satır), Arama (45),
  İnceleme (100), Emsal (136), Sağlayıcılar (110), Ayarlar (60); `components/DualReader.tsx`
  (93), `Notes.tsx` (35), `JobQueueCard.tsx` (19), `WorkerBudgetCard.tsx` (13). F04-T08
  **🚧 kaldı**. (Ölçüm: yorumlar ayıklanıp Türkçe harf taşıyan **yorum dışı** satır sayıldı;
  önceki turların sayıları ham `grep`ten geldiği için daha yüksekti.)
- Yardımcı lib metinleri hâlâ kendi dosyalarında: `lib/jobs.ts` (42), `lib/providers.ts` (42),
  `lib/reader.ts` (8 — diğer 76 satır yorum), `lib/search.ts` (24), `lib/arch.ts` (36),
  `lib/intext.ts` (35), `lib/reminders.ts` (7), `lib/export-coverage.ts` (17), `lib/attack.ts`
  (24), `lib/rollback.ts` (17) → taşınmış ekranların bazı satırları hâlâ oradan geliyor ve
  kapı o dosyaları taramıyor.
- Metinler **birebir aynı** (yalnız kaynak değişti); tek istisna `AI kapalı.` uyarısının
  JSX bölünmesi (ayırıcı nokta/boşluk JSX'te, cümle sözlükte) — görünür metin aynı.
- Tauri penceresi açılmadı → **görsel QA yok**; Windows denenmedi.
- `CANDIDATE_KINDS` modül düzeyinde kurulduğu için `t()` **import anında** çözülür (sözlük
  `as const`, dil değiştirme yok — tek dilli ürün kararı sürüyor).


## F04-T08 — sözlük taşıması: Kronoloji + Kontrol listesi (+ `lib/checklist.ts`) (2026-10-30, ikinci tur)

**Neden:** önceki turun açık kaydı "9 ekran taşınmadı"ydı. Sıradaki en küçük iki ekran
(Kronoloji 494 satır/37 Türkçe satır, Kontrol listesi 498/33) seçildi; Kontrol listesi
**kendi metnini taşımayan** bir yardımcıya (`lib/checklist.ts`, 23 satır) bağlı olduğu için
o dosya da taşındı — yoksa ekran "temiz" görünürken etiketler lib'de kalırdı.

**Yazılanlar**

- `lib/i18n.ts` + **104 yeni anahtar**: `timeline` (50: tarih rolü 7, kesinlik 6, ilişki 4,
  düğme/etiket/ipucu 33) ve `checklists` (54: durum etiketi 4, durum yardımı 4, kural
  iletisi 5, lisans künyesi 2, katalog/öğe metinleri 39). Sözlük toplamı **298** anahtar.
- `screens/Timeline.tsx`: `DATE_ROLE_LABEL`/`PRECISION_LABEL` haritaları sözlükten türetilir
  (`t()`); başlık `nav.timeline`dan. Tüm düğme, alan etiketi, yer tutucu, uyarı ve boş
  durum metni sözlükten.
- `screens/Checklists.tsx` + **`lib/checklist.ts`**: `CHECKLIST_STATE_LABEL`,
  `CHECKLIST_STATE_HELP`, `MISSING_SCOPE_NOTE`, `validateChecklistEntry` iletileri,
  `checklistLicenseLabel` ve `copiedLabel` artık `t()` ile sözlükten okur. Böylece ekran
  metni ile **yardımcı lib metni tek yerde**.
- **Kapı kapsamı büyüdü:** `localization.test.ts` `MIGRATED` listesine
  `screens/Timeline.tsx`, `screens/Checklists.tsx`, `lib/checklist.ts` eklendi → kapı artık
  **lib dosyalarını da** tarıyor (önceki turun açık kaydı "kapı yalnız ekranları tarıyor"un
  ilk adımı).

**Ölçülen kusur (mevcut kapı yakaladı):** `i18n.test.ts` sözlükte **baş/son boşluk**
bırakmayı yasaklıyor; `checklists.noTemplateMeta` değerini `" · şablon künyesi yok …"`
diye yazmıştım → **1 failed** (`checklists.noTemplateMeta: boşluk`). Ayırıcı (`{" · "}`)
JSX'e taşındı, değer kırpıldı → yeşil. Yani kapı, "metni sözlüğe taşırken biçimi bozma"
sınıfını gerçekten yakalıyor.

**Kanıt (kırmızı→yeşil, ölçüldü)**

- `screens/Timeline.tsx`e geçici `const _leak = "Kronoloji başlığı";` →
  `localization.test.ts` **1 failed | 7 passed**, rapor:
  `expected [ 'screens/Timeline.tsx:506' ] to deeply equal []`; satır geri alındı → **8/8**.
- Aynı anda `lib/checklist.ts`e `export const _LEAK2 = "İncelenen kapsam";` →
  rapor iki dosyayı birlikte gösterdi (`lib/checklist.ts:111` + Timeline satırı) → lib
  kapsamı **ölçülerek** doğrulandı.

**Test/derleme:** `tsc --noEmit` temiz · `vitest run` **445/445** (35 dosya; +3:
sözlük bağı 2 + parametreli sayaç 1) · `vite build` temiz (**88** modül, 497,44 kB).
Dokuz taşınmış dosyanın **tümünde** (`App`, `Unlock`, `Workspace`, `Tasks`, `Exports`,
`Redaction`, `Timeline`, `Checklists`, `lib/checklist.ts`) yorum dışı **0** Türkçe harf
satırı (betikle sayıldı: 9/9 temiz).

**Dürüst sınırlar**

- **7 ekran taşınmadı:** Dosyalar/Cases, Arama, İnceleme, Emsal, AI, Sağlayıcılar, Ayarlar.
  F04-T08 **🚧 kaldı**.
- Yardımcı lib metinleri hâlâ kendi dosyalarında: `lib/jobs.ts` (80 Türkçe satır),
  `lib/reader.ts` (84), `lib/providers.ts` (77), `lib/search.ts` (61), `lib/arch.ts` (52),
  `lib/candidate.ts` (47), `lib/intext.ts` (45), `lib/precedents.ts` (41),
  `lib/attack.ts` (41), `lib/rollback.ts` (30), `lib/export-coverage.ts` (30). Yani
  taşınmış ekranların bazı satırları hâlâ bu lib'lerden geliyor; kapı onları taramıyor.
- Metinler **birebir aynı** (yalnız kaynak değişti) → görsel fark beklenmiyor; Tauri
  penceresi açılmadı → **görsel QA yok**. Windows denenmedi.
- `t()` modül yüklenirken değil **çağrı anında** çalışır; `CHECKLIST_STATE_LABEL` gibi
  modül düzeyi sabitler import anında çözülür (sözlük `as const`, dil değiştirme yok —
  tek dilli ürün kararı sürüyor).

## F04-T08 — sözlük taşıması: Görevler, Çıktılar, Maskeleme (2026-10-30)

**Neden:** F04-T08'in açık kaydı "11 ekran taşınmadı"ydı ve taşıma mekanik olduğu
için sürekli erteleniyordu. Bu turda üç ekran **birebir metinle** sözlüğe taşındı
(metinler değişmedi; yalnız kaynakları tek yere alındı).

**Yazılanlar**

- `lib/i18n.ts`: yeni `tasks` (26), `exports` (27) ve `redaction` (37) bölümleri +
  `common` içine `case/noCase/none/save/select`. Parametreli metinler
  (`{count}`, `{page}`, `{scanned}`, `{files}`) `t(key, params)` ile doldurulur —
  eksik parametre yine **hata atar** (yarım cümle gösterilmez).
- `screens/Tasks.tsx` (30 değişiklik), `screens/Exports.tsx` (29),
  `screens/Redaction.tsx` (39): tüm gömülü metin, etiket, düğme, uyarı ve hata
  iletisi sözlükten geliyor.
- **Yeni `lib/filename.ts`** (`fileStem`): dışa aktarma dosya adı gövdesi. Ekranda
  kalan **tek Türkçe harf kümesi** buydu (`/[^\w\- ığüşöçİĞÜŞÖÇ]+/g`) ve kapı onu
  "gömülü metin" sayacaktı; yardımcı saf fonksiyona taşındı, davranış birebir.
  4 birim testi (Türkçe harf korunur, izinli küme dışı karakter kalmaz, tire/alt
  çizgi/boşluk korunur, boş başlık boş gövde verir).

**Kapı genelleştirildi (ölçülen kusur sınıfı kapandı)**

- Eski kapı taşınmış ekranlarda **sabit bir metin listesi** arıyordu
  (`"Kasa açık"`, `"Kasa kilitli"`, …) → yeni bir ekranda yeni bir metin sızarsa
  **sessiz** kalırdı (aynı sınıf #62'de yorumlarda yaşanmıştı).
- Yeni kural: `MIGRATED` listesindeki her dosyada, **yorumlar ayıklandıktan sonra
  Türkçe harf (`ığüşöçİĞÜŞÖÇ`) bulunamaz**. Gömülü Türkçe metin = kırmızı; sözlük
  anahtarı (`t("tasks.listTitle")`) = temiz.
- **Kanıt (kırmızı→yeşil, ölçüldü):** `Tasks.tsx`e geçici olarak
  `const _leak = "Görev listesi";` eklendi → `localization.test.ts`
  **1 failed | 7 passed**; satır geri alındı → **8/8 yeşil**. Dedektör ayrıca
  "canary" testiyle kanıtlandı (`Görev listesi` yakalanır, `t(...)` yakalanmaz,
  yorumdaki Türkçe yakalanmaz).
- Kapsam: `App.tsx`, `Unlock.tsx`, `Workspace.tsx`, `Tasks.tsx`, `Exports.tsx`,
  `Redaction.tsx` — altısında da **0** Türkçe harf (ölçüldü).

**Test/derleme:** `tsc --noEmit` temiz · `vitest run` **442/442** (35 dosya; +5:
`filename` 4 + canary 1) · `vite build` temiz (**88** modül, 492,62 kB).

**Dürüst sınırlar**

- Sözlüğe **9 ekran taşınmadı**: Dosyalar/Cases, Arama, İnceleme, Emsal, AI,
  Sağlayıcılar, Ayarlar, Kronoloji, Kontrol listesi. F04-T08 **🚧 kaldı**.
- Yardımcı **lib metinleri** hâlâ kendi dosyalarında: `lib/reminders.ts`
  (`REMINDER_TEXT`, `REMINDER_POLICY_LABEL`, `reminderLabel`) ve
  `lib/export-coverage.ts` (kapsam satırları/uyarı cümleleri). Yani "Görevler" ve
  "Çıktılar" **ekran dosyaları** taşındı; bu iki ekranın bazı satırları hâlâ lib'den
  geliyor. Kapı bu dosyaları taramıyor (yalnız `MIGRATED` ekranları).
- Metinler **birebir aynı**: davranış değişmedi, görsel fark beklenmiyor; Tauri
  penceresi açılmadı → **görsel QA yok**. Windows denenmedi.
- `fileStem` davranışı eskisiyle aynı (Türkçe harf korunur, diğerleri `_`); dosya
  adı çakışması/uzunluk sınırı bu yardımcının işi değil.

## Ayrıştırma ayarları yeniden denemede korunur (2026-10-28, F04-T07 kapanışı — kusur #58)

**Neden:** 2026-10-28 turu F04-T07'yi "yeniden deneme gerçekten çalışıyor" diye
kapatmıştı; ama ölçülen sınır şuydu: *"yeniden deneme **varsayılan ayarlarla**
koşar (OCR kapalı, kodlama `auto`); özgün OCR/parola iş satırında saklanmaz."*
Bu, "çalışıyor gibi görünen düğme"nin daha sessiz bir biçimidir: düğme gerçekten
çalışır, **ama başka bir iş yapar**.

- **ÖLÇÜLEN KUSUR #58 (işlevsel, sessiz yanlış çıktı):** `job_retry`, `run_parse`a
  `ParseOptions { ocr: None, ocr_lang: None, text_encoding: None }` veriyordu;
  kullanıcının seçtiği ayarlar hiçbir yere yazılmıyordu. Ölçüm (gerçek kasa +
  gerçek worker + **metin katmanı olmayan** fikstür `scanned-page.pdf`, 1 sayfa):
  - OCR **açık** koşu → **1 sayfa / 1 segment**, metin `MADDE 1 / Kira
    sozlesmesi 2026 / Odeme yapilmadi.`
  - **varsayılan** (OCR kapalı) koşu → **1 sayfa / 0 segment**, metin **boş**,
    yalnız uyarı: *"sayfa 1: metin katmanı yok (OCR gerekli)"*.
  Yani taranmış bir belgede yeniden deneme, kullanıcıya "başladı" diyor ve
  **metinsiz** bir sonuç üretiyordu; iş yine `completed` görünüyordu.
- **Şema (migration 0023):** `jobs.parse_ocr`, `parse_ocr_lang`,
  `parse_text_encoding`, `parse_requires_password`. Yol **`ALTER TABLE ADD
  COLUMN`** — tablo yeniden yazılmaz (0021'de ölçülen FK/CASCADE tuzağı
  tekrarlanmaz). `parse_ocr IS NULL` = "bu işte ayar kaydı yok" (0023 öncesi
  işler); varsayılan **uydurulmaz**.
- **Parola saklanmaz:** yalnız `parse_requires_password` bayrağı yazılır.
  Parolayı iş satırında tutmak onu kasa kilidinin dışına çıkarırdı. Bayrak
  `true` ise `job_retry` işi **kendiliğinden başlatmaz**:
  `skipped="password_required"`, satır `failed` kalır (asılı `queued` yok),
  arayüz "parolayı Dosyalar ekranından girin" der.
- **Çekirdek (`crates/storage/src/queue.rs`):**
  `set_job_parse_options` / `job_parse_options`. Kapılar: iş var olmalı,
  `parse_document` türünde olmalı (`E_JOB_OPTIONS`), dil/kodlama **biçim**
  denetiminden geçer (harf/rakam/`+`/`-`/`_`, dil ≤64, kodlama ≤40) — serbest
  metin (yol, belge adı) satıra yazılmaz. Kira aranmaz: bu bir sonuç değil,
  isteğin kendi kaydıdır. `retry_failed_job` ayarları **silmez**.
- **Tauri (`apps/desktop/src-tauri/src/commands.rs`):**
  - `run_parse`, işi sahiplendikten sonra **uygulanan** ayarları (OCR, dil,
    **etkin** kodlama) satıra yazar; yazım ikincildir (düşerse ayrıştırma
    düşmez, yeniden deneme "kayıt yok" görür ve bunu söyler).
  - `plan_retry` artık `RetryPlan { job, versionId, options, skipped }` döner:
    ayarları okur, parola bayrağını **geçişten önce** denetler, denetime
    `optionsRestored` (yalnız bayrak) yazar.
  - `retry_parse_options` (**saf**) kayıtlı ayarı `ParseOptions`a çevirir;
    kayıt yoksa varsayılan + `restored=false`.
- **Arayüz:** `retryOutcomeLine` kaynağı yazar — *"kaynak: kaydedilmiş ayarlar ·
  OCR açık (tur), kodlama auto"* ya da *"kaynak: varsayılanlar (özgün ayar bu
  işte kayıtlı değil)"*; `password_required` için ayrı dürüst satır. `applied`
  tipi `restored` taşır.
- **ÖLÇÜM (gerçek kasa + gerçek Python worker + tesseract 5.5.3, M2;
  `retry_reuses_the_recorded_ocr_setting_on_a_scanned_document`, `--ignored --nocapture`):**
  `1) OCR açık: 1 sayfa / 1 segment` · `2) varsayılan (eski davranış): 1 sayfa /
  0 segment, metin ""` · `3) kayıtlı ayar (yeni davranış): 1 sayfa / 1 segment`,
  metin ilk koşuyla **aynı**.
- **Fikstür:** `fixtures/synthetic/scanned-page.pdf` (1 sayfa, **33.380 bayt**,
  yalnız görüntü; `get_text()` boş). Üretici `generate.py::build_scanned_pdf`
  — diğerlerinden farklı olarak **PyMuPDF** ister (glif çizimi + 300 dpi
  raster); dosya depoda durduğu için testler üretimi koşmaz (ADR-05).
- **Test:** `cargo test -p dim-storage` **369/369** (+3: yaz/oku + biçim kapısı
  + yeniden deneme ayarı silmez) ve entegrasyon 8/8 (5 ignored); tauri lib
  **29 geçti / 5 ignored** (+2 saf/plan testi, +1 gerçek ölçüm); Web
  `tsc --noEmit` temiz, `vitest run` **413/413** (+2 `jobs` iddiası: kaynak
  satırı ve `password_required`). Clippy `-p dim-storage --all-targets` ve tauri
  `--all-targets` `-D warnings` temiz; rustfmt iki manifest temiz.
- **Commit/CI:** `db118fb` → CI **`40724481987` 7/7 yeşil** (sözleşmeler,
  arayüz tsc+vitest+build, rust macos-14 + ubuntu-latest, masaüstü kabuk,
  belge worker'ı, SBOM).
- **Dürüst sınırlar:** 0023 öncesi işlerde ayar **kayıtlı değildir**; yeniden
  deneme varsayılanlarla koşar ve arayüz bunu yazar (sessizce "aynı iş"
  denmez). Parola ile açılmış iş **elle** yeniden ayrıştırılmalıdır. Maskeleme
  işi ayar taşımaz (uydurulmuş istek üretilmez; davranış değişmedi). Kodlama
  ayarı yalnız düz metin belgelerde etkilidir (PDF/DOCX'te `auto`ya düşer) ve
  içe aktarma bugün **UTF-8 olmayan `.txt`ü kabul etmiyor** — bu yüzden
  `windows-1254` gibi bir kodlama seçimi ürün yolunda henüz kullanılamıyor
  (ayrı iş: `detect_kind` yalnız geçerli UTF-8 metni tanıyor). Tauri penceresi
  açılmadı → yeni satırın **görsel QA'si yok**. Windows'ta denenmedi.


## Merkezi sözlük ve biçimlendirme katmanı (2026-10-28, F04-T08 — kısmi)

**Neden:** şartname F04-T08 "Türkçe metinleri merkezi sözlüğe taşı; tarih/sayı
gösterimini veri türünden ayır" diyor. Ölçüm, ikinci yarısının **hiç
yapılmadığını** ve arayüzde üç ayrı biçimin yan yana yaşadığını gösterdi.

- **ÖLÇÜLEN KUSUR #54 (tutarsız biçim):** aynı bayt değeri üç ayrı yerde üç ayrı
  biçimde yazılıyordu — `providers.ts` `"1,5 KiB"` (virgül, doğru), `Settings.tsx`
  ve `Ai.tsx` `(b/1024).toFixed(1) + " KB"` (**nokta**), `reader.ts`
  `toFixed(1) + " MB"`. Türkçede ondalık ayracı virgüldür; ayrıca 1024'e bölen
  birim `KB` değil `KiB`'dir. Aynı ekranda "1.5 KB" ile "1,5 KiB" birlikte
  görünüyordu.
- **ÖLÇÜLEN KUSUR #55 (uydurma boyut):** `reader.ts` küçük önizlemeyi
  `Math.max(1, Math.round(b/1024))` ile **1 KB tabanına** yuvarlıyordu; 20 baytlık
  bir önizleme kullanıcıya "1 KB" görünüyordu. Kullanıcıya olmayan bir boyut
  söylenmez (ürün değişmezi: "ölçülmeyen değer uydurulmaz").
- **ÖLÇÜLEN KUSUR #56 (gün kayması sınıfı):** takvim tarihi `new Date("2026-01-01")`
  ile ayrıştırılırsa **UTC** gece yarısı olur; eksi saat diliminde gün bir önce
  kayar. Ölçüm (Node 22, `America/New_York`): naif ayrıştırma **31.12.2025**,
  yerel kurulum **01.01.2026**; `Europe/Istanbul`'da ikisi de 01.01.2026 (kusur
  +03'te görünmez). Hukuki tarihte bir gün kayması kabul edilemez.
- **Kapanan kusur #57 (bayat etiket):** kenar çubuğu gömülü `"v0.1.0 · F02"`
  yazıyordu; uygulama F04'te. Artık sürüm **tek kaynaktan** (`package.json`) gelir.

**Yazılanlar**

- **`src/lib/i18n.ts` (yeni, tek sözlük):** `tr` ağacı `as const`; `MessageKey`
  yaprak anahtar birleşimi → yazım hatası **derleme hatası**. `t(key, params?)`
  `{ad}` yer tutucusunu doldurur; **eksik parametre hata atar** (ekranda yarım
  cümle görünmez). `messageKeys()` denetim/test için.
- **`src/lib/format.ts` (tek biçimlendirme katmanı):** `formatDateTime` (anlık
  zaman), **`formatLegalDate`** (takvim tarihi: `23.10.2026`, `10.2026`, `2026` —
  eksik gün uydurulmaz, zaman bileşeni taşıyan değer "tarih biçimi
  desteklenmiyor" döner), `formatUnixSeconds` (0/negatif → "—", 1970 uydurulmaz),
  `formatNumber`, `formatPercent`, **`formatSize`** (bayt/KiB/MiB, virgül),
  `formatDuration`. `formatTimestamp` geriye uyum için aynı fonksiyondur.
- **Taşınan ekranlar (tam):** `App.tsx` (gezinme + kabuk + "hazır değil" ekranı),
  `screens/Unlock.tsx` (kasa açma/oluşturma/kurtarma, 40+ metin),
  `screens/Workspace.tsx`. Etiket haritaları (`CASE_KIND_LABEL`,
  `SENSITIVITY_LABEL`, `AI_POLICY_LABEL`) artık sözlükten **türetilir**.
- **Biçimlendirme çağrı yerleri tek katmana alındı (7 dosya):** `arch.ts`,
  `precedents.ts`, `providers.ts` (`formatBytes` = `formatSize`), `reader.ts`,
  `rollback.ts`, `Ai.tsx`, `Precedents.tsx`, `Providers.tsx`, `Settings.tsx`.
  `src` altında `toLocaleString`/`new Intl`/`"tr-TR"` **yalnız** `lib/format.ts`'te.
- **`src/lib/localization.test.ts` (yeni kapı):** (a) biçimlendirme başka dosyaya
  sızarsa, (b) sözlükte kullanılmayan anahtar kalırsa, (c) taşınmış ekranlara
  gömülü metin geri gelirse, (d) kabukta gömülü sürüm etiketi olursa **kırmızı**.

**Commit/CI:** `4d5c2da` → CI **`40605471296` 7/7 yeşil** (sözleşmeler, arayüz
tsc+vitest+build, rust macos-14 + ubuntu-latest, masaüstü kabuk, belge worker'ı,
SBOM). `HEAD == origin/main`; `git status`ta yalnız iki `Cargo.lock` (başka ajanın,
dokunulmadı).

**Test**

- Web `tsc --noEmit` temiz · `vitest run` **413/413** (30 dosya; önceki 384 → +29:
  `i18n` 6, `format` 19, `localization` 4) · `vite build` temiz (83 modül, 698 ms).
- Biçim testleri **iki saat diliminde** koşuldu: `TZ=UTC` 23/23 ve
  `TZ=America/New_York` 33/33 (takvim tarihi kaymıyor).
- Ölçüm/kanıt: 20 bayt → `"20 bayt"` (eskiden "1 KB"); 1.536 bayt →
  `"1,5 KiB"`; 0 saniye → `"—"`.

**Dürüst sınırlar**

- Sözlüğe **11 ekran taşınmamıştı** (Dosyalar/Cases, Arama, İnceleme, Emsal, Maskeleme,
  Çıktılar, AI, Sağlayıcılar, Ayarlar, Kronoloji, Görevler, Kontrol listesi).
  **2026-10-30 itibarıyla 9 ekran kaldı**: Maskeleme, Çıktılar ve Görevler taşındı
  (bkz. yukarıdaki "F04-T08 — sözlük taşıması" bölümü). F04-T08 hâlâ **🚧**;
  kalan taşıma mekanik ama dosya dosya yapılmalı.
- Sözlük **tek dilli**: dil değiştirme düğmesi yok, ürün Türkçe sabit. Sözlüğün
  değeri çeviri değil, **tek kaynak + derleme zamanı anahtar denetimi**.
- `package.json`/`tauri.conf.json`/`Cargo.toml` sürümü hâlâ **0.1.0**, F04 belgesi
  `v0.3.0` diyor; sürüm yükseltmesi (updater/SBOM'u etkiler) bu turda **yapılmadı**
  — kenar çubuğu artık paket sürümünü gösterdiği için ekranda "v0.1.0" yazar.
- Tauri penceresi açılmadı → sözlükten gelen metinlerin **görsel QA'si yok**
  (metinler birebir aynı; yalnız kaynak değişti).
- `formatLegalDate` bugün **çağrılmıyor**: takvim tarihi alanları (F05+ belge
  tarihleri) geldiğinde kullanılacak; şimdilik altyapı + test.

## Yeniden deneme gerçekten çalışıyor (2026-10-28, F04-T07 kapanışı)

**Neden:** şartname F04-T07 "gerçek aşama, sayım, iptal ve **yeniden deneme**
denetimlerini sun" diyor. Önceki turun açık kaydı "arayüzde yeniden deneme
denetimi gerçek mi?" sorusuydu; **ölçüldü: değildi**.

- **ÖLÇÜLEN KUSUR #52 (işlevsel, kritik):** `job_retry` yalnız
  `retry_failed_job` + `append_audit` çağırıyordu (12 satır). Kuyrukta **arka
  plan yürütücüsü yok**: `claim_job_by_key` yalnız `document_parse`,
  `prepare_redaction` ve `documents_parse_pending` içinden çağrılıyor. Yani
  düğme `failed → queued` yazıyor, iş **sonsuza kadar "Bekliyor"** kalıyordu;
  arayüz mesajı "yeni deneme numarası açılacak" diyordu ama hiçbir deneme
  açılmıyordu — kullanıcının en çok istemediği "çalışıyor gibi görünen düğme".
- **Düzeltme — çekirdek:** yeni `Database::job_idempotency_key(job_id)`; iş
  satırı hangi girdiye ait olduğunu buradan çözer (arayüze **verilmez**).
- **Düzeltme — Tauri:** `job_retry` artık **async** ve iki aşamalı:
  - **A (`plan_retry`, kısa kilit):** `failed → queued`, deneme bütçesi sıfır,
    `parse:<sürüm>` anahtarından sürüm çözümü, denetim
    `{state, jobType, started}` (metin/ad/yol yok).
  - **B (kilitsiz):** `run_parse` `spawn_blocking` ile **gerçekten koşar**;
    arayüz iş parçacığı beklemez, ilerleme `parse-progress` olayı **ve**
    `jobs.progress_*` sütunlarından gelir.
  - **Erken düşüş kapısı:** `prepare_parse` artık gövdeyi sahiplenmeden
    **önce** okur; sürüm/blob yoksa iş `queued` kalır ve `finalize_failed_retry`
    satırı `failed` yazar — "Sırada"da asılı kalmaz. Satır `running` ise (başka
    bir yürütücü sahiplendi) **dokunulmaz**.
  - **Bilinçli sınır:** maskeleme işi otomatik **başlatılmaz**
    (`skipped="type_not_runnable"`): terimler/bölgeler iş satırında saklanmaz,
    uydurulmuş bir istekle yeniden maskelemek yanlış çıktı üretirdi.
- **Arayüz:** `JobRetryView` (`started`, `skipped`, `versionId`, `applied`);
  `retryOutcomeLine` "sıraya alındı" ile "başladı"yı **ayırır** ve uygulanan
  ayarı yazar ("OCR kapalı, kodlama auto"). `shouldPollQueue` sayesinde
  yeniden denemesi başlayan iş terminal olana kadar kart 1,2 sn'de bir tazelenir
  (`jobIsTerminal` eski `succeeded`/`cancelled` adlarını da tanır).
- **YAN BULGU #53 (ölçüldü, test kırığı):** `ParsePool::acquire_noticing`
  `Some(süre)`yı yalnız **bildirim eşiği (150 ms)** aşıldığında döndürüyordu;
  yani "sıraya girdi mi" sorusu zamanlamaya bağlıydı. Yüklü makinede
  `large_document_runs_alone` bu yüzden düştü (100 ms bekleyen büyük iş
  `None` aldı). Artık `Some` = **kapı en az bir kez kapalıydı**; test sabit
  100 ms uyku yerine "büyük iş beklemeye girdi" kapısıyla ilerliyor.
- **ÖLÇÜM (gerçek kasa + gerçek Python worker, debug, Apple M2;**
  `DIM_WORKER_PYTHON=workers/document/.venv/bin/python ... --ignored --nocapture`**)**: `retrying_a_failed_parse_job_really_parses_the_document`
  → **4 sayfa, 4 segment, 1,21 s**, kareler `start → open → pages ×4 → store ×2`,
  iş **`completed`**, `progress_stage=store`, `attempt_count=1` (yeniden
  deneme gerçekten yeni bir denemedir).
- **Test:** `cargo test -p dim-storage --lib queue::` **28/28**; tauri lib
  **27 geçti / 4 ignored** (+3: girdi çözümü, plan, gerçek ölçüm); Web
  `tsc --noEmit` temiz, `vitest run` **384/384** (+2).
- **Dürüst sınırlar:** yeniden deneme **varsayılan ayarlarla** koşar (OCR
  kapalı, kodlama `auto`); özgün istekteki OCR/parola iş satırında saklanmaz →
  taranmış belgede OCR'sız deneme boş metin üretebilir (uyarı olarak görünür).
  Parola korumalı belge yine `blocked` olur; parola belge ekranından girilir.
  Maskeleme yeniden denemesi otomatik başlamaz. Tauri penceresi açılmadı →
  düğme/mesaj satırının **görsel QA'si yok**. Arka plan görevi pencere kapansa
  da sürer; kasaya yazma `commit_parse` kapısından geçer (kasa değiştiyse
  yazmaz). Ölçüm tek makine + kurgu belge.


## Doğrulama

`cargo test -p dim-storage --lib` — **338** test (iş kuyruğu 28, canlı sayaç 6 yeni).
`cargo test -p dim-ingest --lib` — **44** test (worker 22; ilerleme ayrıştırma/sınır 5).
`cargo test -p dim-ingest --test worker_progress` — **2** test (gerçek Python worker,
4 sayfalık kurgu PDF: `open 0/4` + `pages 1..4/4`).
`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` — **15** test
(+1: `JobProgressSink` gerçek bellek içi kasayla kuyruk satırına yazıyor).
Python `pytest workers/document/tests` — **140 passed, 1 skipped** (+7 ilerleme testi).
Web `tsc --noEmit` + `vitest run` — **362** test (+11 `parseProgress`, +3 kuyruk
sayaç satırı), `vite build` yeşil.
clippy: `-p dim-ingest` ve tauri `--all-targets -D warnings` temiz; rustfmt iki manifest temiz.

**2026-10-28 (yeniden deneme turu):** `cargo test -p dim-storage --lib queue::` **28/28**;
tauri lib **27 geçti / 4 ignored** (yeni: girdi çözümü, plan, gerçek worker ölçümü);
Web `tsc --noEmit` temiz + `vitest run` **384/384**; rustfmt iki manifest temiz.

## Canlı aşama/sayım (2026-10-27, F04-T07 kapanışı)

**Neden:** önceki turun açık kaydı "canlı aşama/sayım yok — worker tek istek/tek yanıt
çalışır, ilerleme çerçevesi göndermez; kullanıcı 'çalışıyor' görür, sayfa sayacı görmez"di.

**Protokol (geriye uyumlu):** ilerleme **ayrı çerçevedir**, yanıt değildir:

```json
{"v":1,"id":"req-1","progress":{"stage":"pages","done":12,"total":40}}
```

- `ok`/`result` alanı yoktur; istemci bunu sonuç sanmaz. Aşamalar: `start` (Tauri),
  `open` (belge açıldı, toplam biliniyor), `pages` (her sayfa bitti), `store`
  (kasaya yazma: 0 → N).
- Çerçeve belge metni, dosya adı, yol veya kasa bilgisi taşımaz (denetim sınırıyla aynı).
- Eski worker ilerleme göndermezse istemci yalnız `start` gösterir: **sayaç uydurulmaz**.

**Yapılan:**

- `workers/document/dim_worker/protocol.py`: `progress_frame()`; `engines.extract_pdf` /
  `extract_image` sayfa başına `notify(stage, done, total)` çağırır (geri çağrı yoksa
  hiçbir şey yapılmaz — sonuç değişmez); `worker.serve` ilerlemeyi yazar, yazma düşerse
  ayrıştırma düşmez.
- `crates/ingest/src/worker.rs`: yeni `ParseProgress`, `parse_progress()` (tutarsız sayaç
  — ör. 5 / 3 — **protokol hatası**, sessizce gösterilmez), `read_response` ilerleme
  çerçevelerini ayırır (kimlik kapısı + `MAX_PROGRESS_FRAMES = 10_000` sınırı; sonsuz
  akıtan süreç isteği kilitleyemez), yeni `extract_text_with_progress()`
  (`extract_text` aynı yolu geri çağrısız kullanır).
- Tauri `document_parse`: `parse-progress` olayı yayımlar; `start` olayı **kasadan
  okuma başlamadan** gider, worker çerçeveleri aynen iletilir; olay kaybı ayrıştırmayı
  etkilemez. `ParseProgressEvent` yalnız 4 alan taşır (test bunu tutuyor).
- Arayüz (Dosyalar → belge satırı): ayrıştırma sürerken "Sayfalar okunuyor: 12 / 40 · %30"
  satırı + `role="progressbar"`; olay yalnız **etkin sürüm** için gösterilir; toplam
  yoksa yüzde yazılmaz, toplamı aşan sayaç toplamda kırpılır. `lib/parseProgress.ts`
  4 saf yardımcı + 11 test.

**Ölçüm (bu tur, gerçek worker):**

- 4 sayfalık kurgu fikstür (`fixtures/synthetic/progress-pages.pdf`, 1.431 bayt):
  `open 0/4` → `pages 1/4 … 4/4`, toplam **93,5 ms** (süreç başlatma + ayrıştırma).
- Yalnız worker (bellek içi, 120 sayfa): **121** ilerleme çerçevesi; `open` **0,5 ms**,
  sayfa 1 **4,1 ms**, sayfa 30 **7,8 ms**, sayfa 60 **11,6 ms**, sayfa 120 **19,3 ms**,
  yanıt **20,5 ms** — çerçeveler gerçekten **artımlı**, sonuçla birlikte toplu değil.

**CI'ın yakaladığı kusurlar (#47):** ilk koşu `40409362853` **kırmızı** (5/7) iki ayrı
sebeple:

1. `arayüz`: `parseProgress.test.ts` içinde bir beklenti hâlâ `ocr` etiketini arıyordu
   (etiket kaldırılmıştı) → 359 testte 1 kırmızı. Düzeltildi (`store` etiketi).
2. `belge worker'ı`: **duman testi** (`tests/smoke/worker_smoke.py`) tek satır okuyup
   yanıt sanıyordu; artık ilk satır `progress` çerçevesi → `KeyError: 'ok'` ve süreç
   `shutdown` alamayınca 10 sn zaman aşımına düştü. Bu gerçek bir istemci kusuruydu:
   duman testi ve `scripts/verify-worker-offline.py` ilerleme çerçevelerini **atlar**
   (yanıt değildir); duman testi ayrıca `open 0/1` + `pages 1/1` çerçevelerini doğrular.

**Dürüst sınırlar:**

- Tauri penceresi açılmadı: sayacın **görsel** QA'sı yok; kanıt protokol testleri +
  gerçek worker ölçümü + arayüz birim testleri.
- `store` aşaması 0 → N tek çağrıda yazıldığı için **ara değer yok**; okuma aşaması
  kadar akıcı değil.
- İlerleme yalnız **ayrıştırma** yolunda; kuyruktaki diğer iş türleri yürütücüsüz.
  Aynı kareler artık kuyruk satırına da yazılır (aşağıdaki 0022 bölümü) — kuyruk
  kartındaki sayaç ayrıştırma sırasında **canlı** güncellenir.
- OCR aşaması ayrı ad olarak yayımlanmaz: OCR'lı görüntüde de `pages` sayacı kullanılır.
- İlerleme çerçevesi kaybı ayrıştırmayı etkilemez ama sayaç donabilir; sonuç yine
  döner (olay bilgilendiricidir).

## Kuyruk satırında canlı sayaç (2026-10-27, migration 0022)

**Neden:** önceki turun açık kaydı "`jobs.progress` sütunu güncellenmiyor — kuyruk
kartındaki sayı değişmez"di. Ayrıştırma sırasında sayaç yalnız arayüz olayında
(`parse-progress`) yaşıyordu; kasa kapanıp açıldığında ya da başka bir ekran
kuyruğa baktığında "hangi aşamada, kaç sayfa" bilgisi **kayıptı**.

**Şema (migration 0022):** `jobs.progress_stage TEXT`, `jobs.progress_done INTEGER`,
`jobs.progress_total INTEGER`. Yol **`ALTER TABLE ADD COLUMN`** — tablo yeniden
yazılmaz, 0021'de ölçülen FK/CASCADE tuzağı tekrarlanmaz. Sütunlar arası kural kodda:
`done <= total`, `total > 0`, toplam yoksa `done = 0`.

**Çekirdek (`crates/storage/src/queue.rs`):**

- `update_job_progress(job_id, owner, stage, done, total, now)` — yalnız **kira
  sahibi** yazar (`E_JOB_LEASE`); yalnız `running|validating` iş sayar; iş bu arada
  iptal/engel/bitiş olduysa `Ok(None)` (geç gelen sayaç terminal satırı bozmaz).
  Oran (`progress`) sayaçtan **türetilir** — ayrı hesaplanmaz.
- `ProgressCounts::new` — tutarsız sayaç **reddedilir** (`E_JOB_PROGRESS`): `5/3`,
  `total=0`, toplam yokken sayı. Aşama adı protokol adıdır (küçük harf/rakam/alt
  çizgi, ≤24); serbest metin (dosya adı, yol) sütuna sızamaz.
- `claim_jobs` / `claim_job_by_key` / `retry_failed_job` / kurtarma yeniden sıraya
  alma yolları sayacı **sıfırlar**: yeni denemede "12/40" kalıntısı görünmez.
- `ProgressGate` (saf kural) — arayüz olayı her karede gider, **veritabanı yazımı
  seyrekleştirilir**: ilk kare, aşama değişimi, sayacın 0'a dönüşü ve toplamı bulma
  her zaman yazılır; aradakiler yalnız 300 ms dolunca.

**Tauri (`document_parse`):** yeni `JobProgressSink` olayı ve yazımı tek yerden
yürütür: olay **önce** yayımlanır (arayüz DB'yi beklemez), yazım düşerse ayrıştırma
**düşmez** ama sessizce yutulmaz — yazılan/atlanan/başarısız sayıları
`document_parse`/`progress` denetim kaydına geçer (yalnız sayı, metin/ad/yol yok).

**Arayüz:** Ayarlar → İş kuyruğu kartında yeni **İlerleme** sütunu
("Sayfalar okunuyor: 12 / 40 · %30" + `role="progressbar"`). Biçim ayrıştırma
satırıyla **aynı** kaynaktan gelir (`lib/parseProgress.ts`); iki yerde farklı dil
olmaz. Kart, yürüyen iş varken 1,2 sn'de bir **sessiz** tazelenir (düğmeler
kilitlenmez) ve iş bitince kendiliğinden durur. Bitmiş işte sayaç gösterilmez —
önceki denemenin kalıntısı yanıltıcı olurdu.

**Test:** storage **338** (+6: yazım/oran, terminal kapısı, sayaç doğrulama, kira
kapısı, yeniden sahiplenme sıfırlaması, kapı kuralı), tauri **15** (+1: gerçek
bellek içi kasa + gerçek kuyruk satırı ile sink), Web **362** (+3 `jobs`).

**Dürüst sınırlar:**

- Ölçüm "tek yazım maliyeti" düzeyinde; gerçek pencerede uçtan uca süre ölçülmedi.
- Çok kısa belgede (kareler 300 ms'den hızlı) sayaç 0 → %100 atlar: kapı sayaç
  yazımını zamana bağlar, "her sayfa" garantisi vermez.
- `store` aşaması hâlâ tek çağrıda yazıldığı için ara değer yok (0 → N).
- Sayaç yalnız `parse_document` yolunda yazılır; kuyruktaki diğer iş türlerinin
  yürütücüsü yok.
- Kartın görsel QA'sı hâlâ yok (Tauri penceresi açılmadı).

## Maskeleme kuyruk işi olarak koşar (2026-10-28, F04-T07 genişletmesi)

**Neden:** maskeleme `state.lock()` altında çalışıyordu — uzun maskede bütün arayüz
kilitleniyor, iptal düğmesine basılamıyordu. Ayrıca kuyrukta `parse_document` dışında
yürütücüsü olan iş türü yoktu.

**Yapılan (üç aşamalı `run_redaction`):** (A) kilit altında sürüm + blob + iş satırı
(`prepare_redaction`), (B) **kilit yokken** worker süreci ve doğrulama, (C) kilit altında
çıktı/denetim yazımı (`commit_redaction`). Yeni iş türü `redaction` (idempotent anahtar
`redact:<sürüm>:<parmak izi>`, aynı niyet ikinci kez `E_REDACT_BUSY`), iptal yuvası artık
iş türü taşır (`RunningParse` → `RunningWorker` + `kind`), yeni `redact-progress` olayı;
`redaction` / `redaction_regions` komutları async (`spawn_blocking`).

### ÖLÇÜLEN KUSUR #50 — ürün zarfı OCR'ı çökertiyordu (kritik, işlevsel)

`worker_env()` yalnız `PATH/LANG/LC_ALL` veriyordu; `TMPDIR` yoktu. Bu makinede
`/tmp` bir **sembolik bağ**tır ve pytesseract geçici çıktıyı okuyamıyor:

| zarf | sonuç |
|---|---|
| `PATH/LANG/LC_ALL` (ürün) | **`UnicodeDecodeError`** (`byte 0x89`, PNG imzası) |
| `+ HOME` | aynı hata (çözüm değil) |
| `+ TMPDIR=/tmp` | **aynı hata** (sembolik bağ) |
| `+ TMPDIR=/private/tmp` | ✅ `Sayfa 2 kurgu` |
| `+ TMPDIR=<gerçek dizin>` | ✅ |

Eski kodda doğrulayıcı istisnası **sızıntı** sayılıyordu (aşağıdaki #51) → ürün
`E_REDACT_VERIFY` veriyor ve **maskeli dosya hiç kaydedilmiyordu**. Yani tesseract
kurulu bir makinede maskeleme ürün yolunda sessizce çalışmıyordu; önceki turlar bunu
görmedi çünkü testler tam ortamla (`spawn_dev`) koşuyordu.

**Düzeltme:** `worker_env()` artık `TMPDIR/TMP/TEMP` verir; değer kabuk ortamından
devralınmaz, `std::env::temp_dir()`in **çözülmüş** yoludur (`/tmp` → `/private/tmp`).
Regresyon testi: `worker_env_hands_a_real_temp_directory` (mutlak + var + sembolik bağ
değil + üç ad aynı değer).

### ÖLÇÜLEN KUSUR #51 — doğrulayıcı çökmesi "sızıntı" diye raporlanıyordu

`_verify_with_pypdf`, `_verify_with_ocr`, `_region_ocr_leaks` istisna yolunda
`list(terms)` dönüyordu: doğrulayıcı hatası "terim hâlâ var" gibi görünüyordu. Artık
üçlü döner — `(bulunan, çalıştı_mı, hata)`; çökme `remaining`e **girmez**, uyarı olarak
yazılır ("pypdf/OCR doğrulaması yapılamadı"), `engines` listesine o doğrulayıcı eklenmez
ve `ocr_verified=false` kalır. Bölge maskesinde "bölgede metin kaldı" ile "doğrulanamadı"
ayrıldı; "OCR ile doğrulandı" uyarısı yalnız OCR gerçekten koştuysa yazılır.

### ÖLÇÜM (gerçek worker + gerçek kasa, debug, Apple M2)

`redaction_runs_off_the_vault_lock_and_reports_pages` (4 sayfalık fikstür, 3 koşu):
maskeleme **5,27 / 2,91 / 3,00 s** (ilki soğuk), **en uzun kasa kilidi beklemesi
15,4 / 47,5 / 40,6 ms** (eşik 150 ms), kareler birebir
`start → redact 0..4 → pack 0..1 → verify 0..4 → scan 0..1`, iş satırı `completed`,
son aşama `scan`. `redaction_cancel_stops_the_worker_and_writes_nothing`
(120 sayfa, `fixtures/synthetic/cancel-pages.pdf`): iptal **153 / 211 ms**'de döndü,
hata `E_WORKER_CANCELED`, iş `canceled`, **hiçbir belge yazılmadı**.

**Fikstür:** `fixtures/synthetic/cancel-pages.pdf` (120 sayfa, 33.144 bayt) —
`fixtures/synthetic/generate.py` üretir (`build_cancel_pdf`). Test **süreç başlatmaz**
(ADR-05); ölçüm her koşuda aynı belgeyi kullanır.

**Test:** `cargo test -p dim-ingest --lib` **67** (+1 ortam testi, 1 ignored);
tauri `--lib` **25** (+2 iş testi, 3 ignored); ignored ölçüm **2/2**; Python
`pytest workers/document/tests` **147 passed, 1 skipped** (+3 doğrulayıcı dürüstlüğü;
pymupdf 1.24.14 / pypdf 5.1.0 / pytesseract 0.3.13 — CI pinleriyle);
Web `tsc --noEmit` temiz, `vitest run` **382/382**. clippy `-p dim-ingest` ve tauri
`--all-targets -D warnings` temiz; rustfmt iki manifest temiz.

**Dürüst sınırlar:**

- Takas bilinçli: doğrulayıcı çökerse dosya **kaydedilir** ama uyarı + `ocr_verified=false`
  ile; eskiden her şeyi reddediyordu (ürün fiilen çalışmıyordu). Uyarı arayüzde
  "Doğrulama notları" altında görünür.
- OCR doğrulaması yalnız tesseract **kuruluysa** koşar (bu makinede 5.5.3; ürün
  paketinde tesseract dağıtımı ayrı iş — F01-T02).
- Kilit beklemesi örnekleyen bir yoklamayla ölçüldü (2 ms aralık), garanti değil;
  ölçüm **debug** derleme, tek makine, 4 ve 120 sayfalık kurgu belgeler.
- `TMPDIR` düzeltmesi macOS'ta ölçüldü; Windows'ta `TMP/TEMP` verilir ama **denenmedi**.
- Tauri penceresi açılmadı → aşama satırının ve iptal düğmesinin **görsel QA'si yok**.

**Commit/CI:** `7ebc21b` → CI `40583920367` **7/7 yeşil** (sözleşmeler, arayüz
tsc+vitest+build, rust macos-14 + ubuntu-latest — `cargo clippy --workspace
--all-targets -D warnings` dahil, masaüstü kabuk — tauri clippy `-D warnings` dahil,
belge worker'ı pytest+duman, SBOM). `HEAD == origin/main`; `git status`ta yalnız iki
`Cargo.lock` (başka ajanın, **dokunulmadı**).

## İş kuyruğu iptali (2026-10-26, F04-T07)

**Ölçülen kusur (#43):** `job_cancel` yürüyen ayrıştırmaya yalnız `canceling` işareti
yazıyordu. Süreç çalışmaya devam ediyor, `document_parse` sayfaları ve
`finish_parse_run("succeeded")` kaydını yazıyor, **sonra** iş satırı `canceled`
oluyordu: kullanıcı "iptal edildi" görürken çıktı kasadaydı (şartname §17.2
`canceled`: "tamamlanmamış çıktı uygulanmadı"). Ayrıca `document_parse` senkron
komuttu; ayrıştırma boyunca arayüz kilitlendiği için **iptal düğmesine basılamıyordu**.

**Düzeltme:**

- `crates/ingest/src/worker.rs`: `WorkerKiller` — çocuk süreç `Arc<Mutex<Child>>`
  olarak paylaşılır, iptal bayrağı `AtomicBool`. `cancel()` süreci öldürür ve
  toplar (zombi bırakmaz); "iptali ben başlattım" bilgisi bayraktan gelir, çünkü
  ölmüş süreçte `kill()` de başarılı görünür. Yeni `WorkerError::Canceled`
  (`E_WORKER_CANCELED`, `retryable=false`): kopma ve zaman aşımından ayrı kod.
- `apps/desktop/src-tauri/src/state.rs`: `AppState.vault` `Arc<Mutex<…>>` oldu
  (uzun komutlar `spawn_blocking` içinde kilidi tutar); yeni `AppState.parse`
  yuvası süren ayrıştırmanın iptal tutamağını taşır; `ParseSlotGuard` yuvayı
  hata/panik yolunda da temizler.
- `commands.rs`: `document_parse` **async** + `spawn_blocking` (arayüz iş
  parçacığı beklemez). İptal, çıktı yazılmadan **önce** denetlenir; yakalanırsa
  koşu `cancelled` kapanır ve hiçbir sayfa yazılmaz. İptal yolu kuyrukta
  `running → canceling → canceled` olarak biter (`fail_job` iptal isteğini öne alır).
  `job_cancel` **önce** süreci durdurur (kasa kilidini beklemeden), sonra kuyruk
  kaydını yazar ve `workerStopped` alanıyla gerçekleşen sonucu bildirir.
- Arayüz: `cancelOutcomeLine` — süreç durdurulduysa "bekleniyor", durmadıysa
  "durduruldu" denmez; `E_WORKER_CANCELED` mesajı "yarım çıktı kasaya yazılmadı".

**Ölçüm:** iptal, 30 sn zaman aşımını beklemeden **197,8 ms**'de döndü (öldürme →
`E_WORKER_CANCELED`; `worker::tests::cancel_kills_the_running_worker_and_reports_canceled`).

**CI'ın yakaladığı kusur (#45):** aynı test **Ubuntu'da 30,0001 sn** sürdü —
`sh` torun süreç bırakınca borunun yazma ucu açık kalıyor, okuyucu EOF görmüyordu.
Bekleme **50 ms dilimlere** bölündü (`recv_frame`): iptal artık EOF'tan bağımsız
fark edilir. Yeni regresyon testi torun süreci taklit eder
(`cancel_does_not_wait_for_eof_when_a_grandchild_holds_the_pipe`, iptal < 2 sn).

**Yan bulgu (#44):** `E_DISK_FULL` otomatik tekrar listesindeydi; §17.4 "disk
yetersizliği otomatik tekrar döngüsüne girmez" — listeden çıkarıldı.

**Dürüst sınırlar:**

- Tauri penceresi açılmadı: **arayüzün kilitlenmediği görsel olarak doğrulanmadı**;
  kanıt komutun async olması + `spawn_blocking` + derleme/clippy/test.
- Canlı **aşama/sayım** yok: worker tek istek/tek yanıt çalışır, ilerleme çerçevesi
  göndermez; kullanıcı "çalışıyor" görür, sayfa sayacı görmez.
- İptal yalnız **ayrıştırma** yolunda gerçek; CLI sınama iptali ayrı bayrakla
  çalışır (F10). Kuyruktaki diğer iş türleri için yürütücü yok.
- Öldürme `SIGKILL` (çocuk süreç): worker'a graceful "dur" isteği gönderilmez;
  worker'ın bellek içi yarım çıktısı zaten kasaya yazılmadığı için veri kalıntısı
  oluşmaz, ama worker geçici dosya yazdıysa temizlenmez.
- Windows'ta iptal testleri koşmaz (`#[cfg(unix)]`, `/bin/sh` vekil süreç);
  Windows'ta süreç öldürme yolu denenmedi.

## §17.3 belge worker paralelliği (2026-10-27)

**Neden:** şartname §17.3 "aynı anda **en fazla iki** belge worker işi; büyük
dosyada **bir** iş" diyor. Kuyruk yürütücüsü seriydi ve `document_parse` kasa
kilidini **bütün** ayrıştırma boyunca tutuyordu: iki belge paralel istense bile
ikincisi kilidi beklerdi. Yani "iki worker" yalnız kâğıt üzerinde vardı.

**Yapılan (dikey dilim):**

- **`parse_pool.rs` (yeni):** eşzamanlılık kapısı. Normal iş **2** eşzamanlı,
  `LARGE_DOCUMENT_BYTES` (32 MiB) üstü iş **tek başına** koşar. Küçük iş büyük
  işi beklemez; sonsuz açlığa karşı `LARGE_FAIRNESS_WAIT` (2 sn) sonrası yeni
  küçük iş alınmaz. `QUEUE_NOTICE_WAIT` (150 ms) aşılırsa arayüze **`queued`**
  karesi gider (sessiz bekleyiş yok).
- **Üç aşamalı ayrıştırma (`run_parse`):** (A) kilit altında sürüm + kuyruk
  sahiplenmesi + blob çözümü; (B) **kilit yokken** worker süreci; (C) kilit
  altında koşu/sayfa/segment yazımı. Kasa kilidi artık ayrıştırma boyunca
  tutulmuyor → iki belge **gerçekten** paralel.
- **Tek yazıcı korunur:** ilerleme yazımları kilit dışında **kısa** kilitlerle,
  sonuç yazımı zaten kilit altında; iki ayrıştırma aynı anda yazmaz.
- **Kira sahibi tekil:** `parse-<pid>-<sıra>`; iki iş aynı sahibi taşısaydı
  kuyruk "kim kiraladı" ayrımını kaybederdi.
- **§17.5 uyumu:** `vault_lock` artık **önce** süren ayrıştırmaları durdurur
  (kilit bırakılarak çalışıldığı için gerekli oldu). `job_cancel` iş kimliğiyle
  eşleşen süreci durdurur; diğer paralel ayrıştırma etkilenmez. Yazma aşamasında
  iş "bizim + `running`" değilse **çıktı yazılmaz** (§17.2 `canceled`).
- **Toplu ayrıştırma:** `documents_pending_parse` (güncel sürümü var, başarılı
  ayrıştırması yok) + `documents_parse_pending` (en fazla 4 belge, havuz 2).
  Arayüz: **Belgeler → "Bekleyenleri ayrıştır"** düğmesi; sonuç kartı belge
  başına gerçek sonucu, sayfayı ve **gözlenen eşzamanlılık tepesini** yazar.
  `jobs` tarafında toplu iş için tek denetim satırı (yalnız sayılar).

**ÖLÇÜM (gerçek kasa + gerçek Python worker, debug derleme, Apple M2):**
`two_parses_run_concurrently_on_a_real_vault` (elle: `--ignored --nocapture`),
iki belge (`progress-pages.pdf` 4 sayfa, `redaction-hidden.pdf` 1 sayfa),
ısınma koşusundan sonra:

| Koşu | Duvar saati (1. ölçüm) | Duvar saati (2. ölçüm) | Not |
|---|---|---|---|
| Seri (2 belge sırayla) | 1,235 s | 1,233 s | belge başına ≈ 0,62 s |
| Paralel (2 worker) | **0,721 s** | **0,699 s** | **1,71× / 1,76×** |
| Çakışma | 0,720 s | 0,698 s | iki aralık neredeyse tamamen kesişiyor |

Ayrıştırma süresinin baskın kısmı **süreç başlatma + PyMuPDF importu** (≈ 0,4 s);
belge 4 sayfa olduğu için kazanç üst sınırı süreç sayısıyla sınırlı. Küçük
belgede iki worker **daha yavaş değil**, ama kazanç 2× değil: ölçülen 1,71×.

**İlk ölçüm tuzağı (kayda değer):** ilk denemede paralel koşu **önce**, seri
koşu **sonra** yapıldı ve paralel 1,816 s / seri 1,389 s çıktı — yani "paralel
daha yavaş" göründü. Sebep paralellik değil **soğuk önbellekti** (ilk iki süreç
PyMuPDF'i diskten okuyor). Isınma koşusu eklendikten sonra sıra tersine döndü.
A/B sırası ısınma olmadan ölçümü yanıltıyor.

**Test:** tauri **21** (+5 `parse_pool`: iki iş sınırı, büyük iş tek başına,
açlık önleyici, kuyruk bildirimi, boş havuz; +1 gerçek kasa/worker ölçümü
`#[ignore]`), storage **339** (+1 `documents_pending_parse`), Web **363**
(+1 `queued` etiketi). clippy tauri `--all-targets -D warnings` ve
`-p dim-storage --all-targets -D warnings` temiz; rustfmt iki manifest temiz.

**Dürüst sınırlar:**

- Ölçüm **debug** derleme ve **küçük** belgelerle; release ve büyük belgede
  kazanç daha yüksek olabilir — **ölçülmedi** (türetilmiyor).
- `LARGE_DOCUMENT_BYTES` = 32 MiB **seçilmiş** bir eşik; gerçek büyük belgeyle
  "tek başına koşma" davranışı ölçülmedi (birim testte 1 byte'lık sınırla
  doğrulanıyor).
- Toplu ayrıştırma **arayüzde görsel QA edilmedi** (Tauri penceresi açılmadı);
  komut katmanı ve havuz testlerle doğrulandı.
- Toplu ayrıştırma **kullanıcı seçimlerini** (OCR, parola, kodlama) taşımaz:
  varsayılanlarla çalışır. Parola korumalı belge `blocked` olur, kullanıcı tek
  belge akışından açar.
- Windows'ta süreç/paralellik yolu denenmedi.

## §17.3 worker kaynak bütçeleri (2026-10-27)

**Neden:** şartname §17.3 "süreç başına CPU/bellek/çıktı bütçesi; salt bir
timeout değeri bütün belgeler için kullanılmaz; bütçe aşımı **ayrı hata kodu**
taşır" diyor. Önceki durum tek bir `DEFAULT_TIMEOUT` (30 sn) idi: render,
ayrıştırma, OCR, redaksiyon ve AI çağrısı aynı sınırı paylaşıyordu, bellek/çıktı
sınırı yoktu ve aşım `E_WORKER_TIMEOUT` (transport arızası gibi, **tekrar
denenebilir**) olarak raporlanıyordu.

**Yapılan (dikey dilim):**

- **`crates/ingest/src/budget.rs` (yeni):** beş profil — `render`, `extract`,
  `ocr`, `redaction`, `ai` — × beş sınır: CPU saniyesi, bellek, yanıt/çıktı
  baytı, açık dosya, duvar saati. Profiller **birbirinin yerine geçmez** (test
  ağırlık sırasını ve AI profilinde CPU sınırının bilinçli yokluğunu tutuyor).
- **Sınır uygulama yolu:** POSIX `sh` zarfı (`ulimit -t`, `ulimit -n` +
  `exec "$0" "$@"`). Argümanlar kabuk tarafından **yorumlanmaz** (konumsal
  parametre): `$(whoami)`, `a b`, `tek'tırnak` birebir geçiyor (test). `ulimit`
  uygulanamazsa zarf `exit 126` verir → `E_WORKER_LIMIT` ("bütçeli" görünen
  sınırsız süreç yok).
- **Ayrı hata kodları:** `WorkerError::Budget` → **`E_WORKER_BUDGET`**,
  `retryable=false` (§17.4: bütçe aşımı otomatik tekrar döngüsüne girmez);
  sınır uygulanamadı → `E_WORKER_LIMIT`. Duvar saati aşımı bütçeli işte
  `E_WORKER_TIMEOUT` **değil** `E_WORKER_BUDGET(Time)`; bütçesiz istemcide eski
  davranış korunur (protokol testleri).
- **Ölçüm yolu:** çıktı sayacı okuyucu iş parçacığında (istek başına sıfırlanır,
  parçalı yanıtta parçalar toplanır); bellek gözcüsü RSS örnekler (macOS
  `ps -o rss=`, Linux `/proc/<pid>/status`; `libproc` `unsafe` ister, kullanılmaz)
  ve sınır aşılınca süreci öldürür; `SIGXCPU`(24)/`SIGXFSZ`(25) sinyal yolu da
  ihlali rapora yazar (`BudgetUsage::mark_violation` — ilk neden korunur).
- **Tauri:** `spawn_worker(kind)`; 8 çağrı yeri gerçek türle bağlandı
  (önizleme/rapor → `render`, ayrıştırma OCR açıksa `ocr` değilse `extract`,
  redaksiyon → `redaction`, envanter/toplu içe aktarma → `extract`).
  `ParseResultView.budget` iş sonrası **uygulanan** profili ve gözlenen tepe
  değerleri taşır; yeni `worker_budgets` komutu profilleri çekirdekten okur
  (arayüz kendi sabitini yazmaz).
- **Arayüz:** Ayarlar → **"Kaynak bütçeleri"** kartı (beş profil × beş sınır,
  tablo + tek satır özet), ayrıştırma sonuç satırında gerçek tepe bellek/yanıt/
  süre. `lib/budget.ts` 10 saf yardımcı + 8 vitest.
- **Bulunan ve düzeltilen üç kusur (bu tur):**
  1. **Sinyal yolundan gelen ihlal rapora yazılmıyordu** (`violated: None` iken
     hata `Cpu`) → `mark_violation`.
  2. **Kısa işte "tepe bellek 0 MiB"** raporlanıyordu: gözcü 500 ms'de bir
     uyanıyor, 433 ms'lik iş hiç örnek almıyordu. Düzeltme: gözcü **ilk örneği
     hemen** alır + `budget_report()` rapor anında son örneği alır (`note_peak`;
     süreç yaşıyorsa, `try_wait` ile toplanmış pid'e örnek alınmaz). Regresyon
     testi: `report_samples_peak_memory_even_when_the_job_ends_before_the_watchdog_wakes`
     (bellek bütçesi kapalı → gözcü koşmaz, tek örnek rapor anındadır).
  3. **Linux'ta CPU bütçesi yanlış kodla raporlanıyordu (CI yakaladı).**
     `ulimit -t N` soft **ve** hard tavanı birlikte N yapar; Linux çekirdeği bu
     durumda `SIGXCPU` yerine **doğrudan `SIGKILL`** gönderir → süreç sinyalle
     ayırt edilemez, iş `E_WORKER_DISCONNECTED` (**retryable** — §17.4 ihlali)
     olarak kapanır. macOS'ta aynı kod `SIGXCPU` verdiği için kusur yerelde
     görünmüyordu. Düzeltme: zarf önce **hard** tavanı indirir (`ulimit -H -t
     soft+5`), sonra **soft** tavanı (`ulimit -S -t N`) — `SIGXCPU` garanti,
     SIGXCPU'yu yok sayan süreç için 5 sn sonra `SIGKILL` geri çekilmesi.
     `SIGXFSZ` (25) eşlemesi **kaldırıldı** (`RLIMIT_FSIZE` uygulanmıyor; "çıktı
     bütçesi aşıldı" demek yanlış olurdu). Kanıt: aynı zarf Ubuntu 24.04
     (dash, Docker) içinde — **eski** biçim `ulimit -t 1` → çıkış **137**
     (`SIGKILL`); **yeni** biçim → çıkış **152** (`128+24`, `SIGXCPU`). Regresyon
     testleri: `cpu_hard_limit_stays_above_the_soft_limit`,
     `a_worker_killed_by_the_cpu_signal_reports_the_cpu_budget`.

**ÖLÇÜM (gerçek Python worker + PyMuPDF, `progress-pages.pdf` 4 sayfa, debug,
Apple M2, bu turda yeniden koşuldu):** süreç süresi **115,9 ms**, tepe bellek
**57 MiB**, yanıt **1.431 B**, ihlal **yok**; `extract` profili (120 sn CPU,
1024 MiB bellek, 32 MiB çıktı, 256 dosya, 300 sn) aşılmıyor → bellek tavanının
~%5,6'sı kullanılıyor. Bütçe sınırları sınama yoluyla doğrulandı: sonsuz döngü
`SIGXCPU` ile ~1 sn'de durdu, 200 MiB ayıran süreç 64 MiB sınırında
`Budget(Memory)` verdi, 200 KB yanıt 64 KiB çıktı bütçesinde reddedildi, 1 sn
duvar saati `E_WORKER_BUDGET(Time)` döndü.

**Test:** `dim-ingest` lib **65 geçti / 1 ignored** (+9 `budget`: profil
sıralaması, sınır arama, zarf metni/hard-soft sırası, sayaç/ilk-neden, sıfırlama;
+7 worker: CPU, CPU sinyal eşlemesi, bellek, çıktı, duvar saati, rapor-anı
örneklemesi), tauri **23 geçti / 1 ignored** (+2: bütçe görünümü çekirdek
raporuyla birebir, beş profil çekirdekten gelir); arayüz `budget.ts` üzerinde
**39 gerçek assert** (vitest'siz node tip-sıyırma koşucusu; `node_modules` yok).
clippy `-p dim-ingest --all-targets -D warnings` temiz, tauri clippy
`--all-targets -D warnings` temiz, rustfmt iki manifest temiz.

**Dürüst sınırlar:**

- `sh`/`ulimit` **yalnız POSIX**: Windows'ta CPU ve açık dosya sınırı
  **uygulanmaz** (süre ve çıktı bütçesi uygulanır). Windows'ta denenmedi.
- Bellek gözcüsü **aralıklı** örnekler (500 ms): süreç örnekler arasında sınırı
  aşabilir; aşım bayrağı yazılır ama "hiç aşmadı" garantisi yoktur. macOS'ta her
  örnek bir `ps` süreci başlatır (bütçe yoksa gözcü hiç koşmaz).
- **Hard CPU tavanı "en iyi çaba"**: kısıtlı ortamda `ulimit -H` indirilemezse
  yalnız soft sınır uygulanır (`SIGXCPU` yine gelir; süreç sinyali yok sayarsa
  duvar saati bütçesi devreye girer). Süreç `SIGKILL` ile ölürse neden
  **atfedilmez** — iş `E_WORKER_DISCONNECTED` olarak raporlanır (sinyalden neden
  uydurulmaz).
- **AI profili süreç değil:** CPU/dosya sınırı yok, yalnız duvar saati ve yanıt
  boyutu ölçülür; modelin bellek kullanımı bu bütçeye girmez.
- Profil değerleri **başlangıç ayarları** (ölçümle gerekçelendirilmiş ama tek
  belge/tek makine): büyük ve taranmış belgelerde tavanlar F15 ölçümüyle
  güncellenmeli.
- Bütçe aşımı işi `failed` yapar; **kısmi çıktı uygulanmaz** (worker zaten
  kasaya yazmaz) ama yarım ayrıştırma raporlanmaz — kullanıcı yeniden başlatır.
- Arayüzün **görsel QA'si yok** (Tauri penceresi açılmadı); kartın okunabilirliği
  ve tablo taşması denenmedi.

## Bilinen sınırlar

- Erişilebilirlik (klavye/ekran okuyucu) tam denetimi F15'te.
- Çoklu pencere yok; tek yazıcı kuralı korunur.
