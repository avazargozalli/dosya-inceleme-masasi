# F08 — Manuel kronoloji, iddialar, inceleme ve görevler

- **Sürüm etiketi:** `v0.7.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F08-T01 | ✅ | olay kaydı: tarih türü, kesinlik, ham tarih |
| F08-T02 | ✅ | bilinmeyen tarih sıralamaya sokulmaz, ayrı grupta |
| F08-T03 | ✅ | onay (`draft → confirmed`) ve denetim kaydı |
| F08-T04 | ✅ | iddia–delil matrisi, inceleme kuyruğu, kabul/ret/düzelt geçmişi (**arayüzde görünür**: kalem başına "Geçmişi göster") |
| F08-T05 | ✅ | görevler: kullanıcı tarihi, tamamlama, silme |
| F08-T05 *(REQ-028 yerel bildirim)* | 🚧 | **hatırlatıcı bildirimi gerçekten teslim edilir**: `outbox_events.delivered_at`ı yazan kod **yoktu** (her satır sonsuza kadar “bekliyor”); `pending_notifications`/`mark_notifications_delivered`/`record_notification_failures` + Tauri `notifications_deliver` (`tauri-plugin-notification`) + Tasks “Bildirimleri gönder” düğmesi; metin görev/dosya/taraf adı taşımaz; alan olayları bildirim sayılmaz; 12/12 mutasyon (2026-11-11). **Kalan:** gerçek OS bildirimi bu ortamda koşulamadı (pencere/izin) — insan |
| F08-T06 | ✅ | not ↔ kaynak alıntı bağı (`source_anchor_ids`) |
| F08-T01 *(şartname numarası)* | ✅ | **kişi/kurum + dosya rolü** ve **geri alınabilir birleştirme**: `actors` / `case_participants` tabloları ilk kez kullanılır; `actor_merges` (şema **v25**); arayüzde “Dosya tarafları” kartı — 14/14 mutasyon (2026-11-08) |
| F08-T03 *(şartname numarası)* | ✅ | **kesin/yaklaşık/aralık/bilinmeyen tarih sıralaması, gün uydurmadan**: ay/yıl kesinliğinde gün **saklanmaz ve gösterilmez**, aralık iki uçlu tek değer (`başlangıç/bitiş`, başlangıçla sıralanır), ham tarih metni düzenlemede **korunur**, CSV kesinliğe uyar; `E_EVENT_DATE`; 11/11 mutasyon (2026-11-10) |
| F08-T04e | ✅ | **kanıt ve iddia referansları dosya sınırını aşamaz** (şema **v32**): `claims`, `evidence_links`, `event_sources`, `claim_relations`, `tasks` × BEFORE INSERT/UPDATE = **10 tetikleyici**; ölçülen kusur: B dosyasının belgesi silinince A'nın kanıt bağı `CASCADE` ile **sessizce** yok oluyordu; `scope_violations()` 13 tabloyu sayar; 7/7 mutasyon (2026-11-14) |
| F08-T04f | ✅ | **JSON dizi kanıt listeleri dosya sınırını aşamaz** (şema **v33**): `notes`, `note_revisions`, `review_items`, `note_drafts` → `source_anchor_ids` × BEFORE INSERT/UPDATE = **8 tetikleyici** (`json_each`); ölçülen kusur: v33 kapıları yokken A notuna B çapası yazılıyor, B belgesi silinince çapa gidiyor, notta kimlik kalıyor, denetim 0; `scope_violations()` 17 tabloyu sayar; 9/9 mutasyon (2026-11-15) |
| F08-T04g | ✅ | **kalan referanslar dosya sınırında** (şema **v34**): soy ağacı (`parent_version_id`), maskeleme çıktısı (`output_version_id`), gönderim sicili (`ai_context_items` sürüm/çapa/`segment_ids`), giden segment sicili (`transfer_segments`), çapa/işaret sayfası, aday kökeni, taslağın notu × BEFORE INSERT/UPDATE = **14 tetikleyici** (7 tablo); ölçülen kusur: v33'te A dosyasının AI çalıştırmasına B dosyasının sürümü+çapası yazılıyor (`Ok(1)`) ve ekran **B'nin belge adını** gösteriyordu; plan çıktısı ve soy ağacı da doğrudan SQL ile çapraz yazılabiliyordu; `scope_violations()` **19 tabloyu** sayar (+`aiContextItems`, `transferSegments`); 13/13 mutasyon (2026-11-15) |
| F08-T04h | ✅ | **belgenin güncel sürümü kendi belgesi olmalı** (şema **v35**): `documents.current_version_id` × BEFORE INSERT/UPDATE + ters yön (`document_versions.document_id` taşıması) = **3 tetikleyici**; ölçülen kusur: tek bir doğrudan `UPDATE` belgeyi **başka dosyanın** sürümüne çeviriyordu — kapsam raporu B'nin 2 sayfasını A'da saydı, A'nın «gizli metin» alıntısı B'nin metninde **`exact`** döndü (INV-05); `scope_violations()` **20 tabloyu** sayar (+`documents`); 7/7 mutasyon (2026-11-15) |
| F08-T04i | ✅ | **güncel sürüm okuması kendi belgesine bağlı kalır** (şema **v36**): `current_document_versions` görünümü — yedi okuma yolu (belge künyesi, ayrıştırma kuyruğu, kapsam raporu, alıntı taraması, gönderim kapsamı, plan kapsamı, çapa/plan okumaları) **tek tanımdan** geçer; ölçülen kusur: v35 öncesi çapraz işaretçi okumada **yabancı metni** A'nın kapsamına sokuyordu (rapor 1 sayfa/1 segment, alıntı `exact`, önizleme örneği «gizli tutanak»); 8/8 mutasyon (2026-11-15) |
| F08-T04j | ✅ | **sürüm izi sicilleri kendi dosyasına bağlanır** (şema **v37**): `citation_checks.case_id` + göç, 2 tablo × BEFORE INSERT/UPDATE = **4 tetikleyici** (`export_items` dosyayı `exports`tan okur); ölçülen kusur: `record_citation_check` çapraz sürümü kabul ediyor, `list_citation_checks('v9')` **B dosyasının ekranında** A'nın metnini gösteriyordu (INV-05), doğrudan SQL A'nın dışa aktarmasına B'nin sürümünü yazabiliyor ve `scope_violations()` ikisinde de **0** diyordu; liste **iki anahtarlı** (dosya + sürüm); 11/11 mutasyon (2026-11-15) |
| F08-T04k | ✅ | **giden segment sicili kendi segmentine bağlıdır** (şema **v38**): 0034'ün iki `transfer_segments` tetikleyicisi genişletildi — satırın **iki segment kolonu** (`outbound_segment_id`, `source_segment_id`) **var olan** bir segment ise gösterilen sürümün segmenti olmalı (sürüm `NULL` ise çalıştırmanın dosyasında); ölçülen kusur: doğrudan SQL ile A'nın çalıştırmasına A'nın sürümü + **B'nin segmenti** yazılıyor, A'nın gönderim sicili B'nin parçasını gösteriyordu ve `scope_violations()` **0** diyordu (kör); `transfer_segments` sayımı genişledi (yeni tablo yok); 6/6 mutasyon (2026-11-15) |
| F08-T04l | ✅ | **aktarım onayı kendi dosyasının belgelerini gösterir** (şema **v39**): `transfer_consents` üzerinde **2 tetikleyici** — `selected_document_ids` geçerli JSON dizi, elemanları metin, liste boş olamaz ve her eleman `case_scope` dosyasında **silinmemiş** bir belge olmalı; ölçülen kusur: doğrudan SQL ile A'nın onayına **B'nin belgesi** yazılıyor, `list_transfer_consents('c1')` onu A'nın onaylanmış kapsamı diye sayıyor, boş/bozuk liste sessizce “0 belge” okunuyor ve `scope_violations()` **0** diyordu (kör); `transfer_consents` sayımı **eklendi** (23. tablo, türetim kuralı `case_scope` çiftini kapsar); 8/8 mutasyon (2026-11-15) |
| F08-T04m | ✅ | **karar hedefi kendi dosyasında olmalı** (şema **v40**): 0033'ün iki `review_items` tetikleyicisi genişletildi — `target_entity_id` doluysa kimlik `claims`/`notes`/`tasks`/`events`ten **birinde var** ise satırın `case_id`'si aynı olmalı, `UPDATE OF` listesine `target_entity_id` eklendi; ölçülen kusur: doğrudan SQL ile A'nın öğesine **B dosyasının** iddiası/notu yazılıyor, `list_review_items('c1')` onu A'nın öğesi diye döndürüyor (arayüzde hedef rozeti), öğe yanlış hedefle **karara kilitleniyor** ve `scope_violations()` **0** diyordu (kör); tarama `review_items` alanı genişledi (yeni tablo yok, 23 kalır); 8/8 mutasyon (2026-11-16) |
| F08-T04n | ✅ | **yerine geçen kayıt kendi dosyasında olmalı** (şema **v41**): 0033/0040'ın iki `review_items` tetikleyicisi genişletildi — `superseded_by_item_id` doluysa `superseded_at` dolu olmalı, satır kendini gösteremez, kimlik `review_items`ta **başka dosyada** var ise yazılmaz; `UPDATE OF` listesine iki sütun eklendi; ölçülen kusur: A'nın öğesine B dosyasının öğesi yazılıyor, `list_review_items('c1','superseded')` çapraz kaydı A'nın öğesi diye döndürüyor ve çekirdeğin gerçek `supersede` UPDATE'i ilk işareti **sessizce eziyordu**; tarama 23 kalır; 10/10 mutasyon (2026-11-16) |
| F08-T04o | ✅ | **kontrol listesi kanıt bağları kendi dosyasında olmalı** (şema **v42**): `case_checklist_items` üzerinde **2 tetikleyici** — `linked_document_ids` geçerli JSON dizi (≤50 eleman), her eleman `case_id` dosyasında **silinmemiş** belge olmalı; `present` en az bir bağ ister, `open`/`missing`/`not_applicable` bağ kabul etmez; ölçülen kusur: doğrudan SQL ile A'nın maddesine **B'nin belgesi** yazılıyor, ekran "Bulundu" rozetiyle çıplak UUID gösteriyordu; bozuk JSON **tüm listeyi** okunamaz kılıyordu; `scope_violations()` tabloyu hiç saymıyordu (24. tablo); 11/11 mutasyon (2026-11-16) |
| F08-T04p | ✅ | **çalıştırmanın yasal dayanağı kendi dosyasında olmalı** (şema **v43**): `ai_runs` üzerinde **2 tetikleyici** — `consent_id` doluysa gösterdiği onayın `case_scope`u çalıştırmanın `case_id`si olmalı; çalıştırmanın `case_id`/`consent_id`si kurulduktan sonra **değişmez**; ölçülen kusur: doğrudan SQL ile A'nın çalıştırmasına **B'nin onayı** yazılıyor, çalıştırma başka dosyaya taşınıyor (mevcut gönderim sicili çaprazlaşıyor) ve `scope_violations()` `ai_runs`ı hiç saymıyordu (25. tablo); 9/9 mutasyon (2026-11-16) |
| F08-T04q | ✅ | **emsal sürümünün çakışma işareti kanıtlanabilir olmalı** (şema **v44**): `precedent_versions` üzerinde **2 tetikleyici** — `identity_conflict` 0/1, rozet⇔işaret birlikte, hedef kendisi olamaz, var olan hedef aynı emsalde ve **farklı** `body_hash`te olmalı, ters yön dahil; `id`/`precedent_id`/`body_hash`/bayrak/işaret **değişmez**; ölçülen kusur: doğrudan SQL ile A emsalinin sürümü **B emsalinin sürümüyle** çakıştığını iddia ediyor, "aynı künye, farklı metin" rozeti **aynı** hash ile yazılıyor ve işaret UPDATE ile çevrilebiliyordu; tablo `case_id` taşımaz → kapsam taraması değişmedi (25 kalır); 13/13 mutasyon (2026-11-16) |
| F08-T04r | ✅ | **gönderim sicilinin ve onayın dönüşüm bağı kanıtlanabilir olmalı** (şema **v45**): `transfer_segments` + `transfer_consents` üzerinde **4 tetikleyici** — dolu `transform_manifest_id`/`outbound_transform_id` **var olan** ve `kind='mask_terms'` dönüşümü göstermeli; sicil satırının bağı çalıştırmanın onayıyla **aynı** olmalı (0 terimli onayda `NULL`); sicilin `id`/`ai_run_id`/`transform_manifest_id` ve onayın maske bağı **değişmez**; ölçülen kusur: doğrudan SQL ile onaylanan A maskesi yerine B yazılıyor, uydurma/`identity`/NULL bağ kabul ediliyor ve onayın bağının UPDATE ile çevrilmesi maske kapısını atlatıyordu; tablolar `case_id` taşımaz → kapsam taraması değişmedi (25 kalır); 13/13 mutasyon (2026-11-16) |

## Doğrulama

`cargo test -p dim-storage timeline::` — 5 test. CI: **bekliyor**.

## Bilinen sınırlar

- Olay kaynakları (`event_sources`) tablosu var; olay–kaynak bağlama UI'sı açık.
- İnceleme kabulü kalıcı domain kaydı (olay/iddia/not) üretmez; domain komutu F11'de.
- Hukuki süre hesabı yok (ADR-0008).

## Olay–kaynak bağlama (ek)

- `event_sources` üzerinden olay → kaynak anchor bağı: `states`, `supports`,
  `contradicts`, `context`.
- Aynı bağ ikinci kez eklenmez; farklı dosyadaki anchor reddedilir.
- Kronoloji ekranında her olay için kaynak paneli: alıntı seç, ilişki seç, bağla/kaldır.
- Bağ `user_confirmed=1` ile yazılır; otomatik bağlama yok.

## İnceleme ekranı — yazma yolları ve kapsam kapıları (ek)

`screens/Review.tsx` (1127 satır, 21 api çağrısı) ekranının bugüne dek **7 testi**
vardı ve bunlar kararları ölçüyordu (hedefsiz kabul yok, bayat alıntı hükmü yok).
Aynı turda ekranın **uçuştaki** davranışı ölçüldü: hangi dosyaya/iddiaya hangi
yanıtın yazıldığı (INV-05), bayat hata ve kullanıcı girdisinin sessizce
silinmesi. **19 yeni test** (7 → 26).

Ölçülen ve düzeltilen kusurlar (düzeltme öncesi ham koşu: **16 failed | 7 passed**;
on altısı da gerçek kusur; sonradan eklenen üç test — çapa bağlama hatası ve iki
taslak testi — mutasyonla (M15/M17/M18) doğrulandı):

1. **`load` kapısı yok — çapraz dosya yazımı (en ağırı):** dört listeyi (iddia,
   aday, çapa, belge) koşulsuz yazıyordu. A dosyası yüklenirken B'ye geçilince
   A'nın **geç** yanıtı B'nin listesini eziyordu ("1 kayıt" → "2 kayıt").
   Düzeltme: `loadSeq` + `caseRef`; yanıt yalnız aynı dosya ve en son istekse yazılır.
2. **Düşen yükleme önceki dosyanın verisini bırakıyordu:** hata dalında hiçbir
   alan yazılmıyordu → okunamayan B dosyasının altında A'nın iddiaları ve adayları
   "B'nin verisi" gibi duruyordu. Düzeltme: hata dalı listeleri boşaltır.
3. **`runQuoteCheck` kapısı yok:** denetim uçuşta dosya değişince eski dosyanın
   alıntı hükmü yeni dosyanın altında çiziliyordu.
4. **`bindQuote` kapısı yok:** uçuştaki `anchor_create` yanıtı eski dosyanın
   çapasını **yeni dosyanın** kaynak listesine ekliyordu (`["", "an-99999999", "b1"]`).
5. **Seçili iddia yarışı (iki yüz):** `evidenceLinkList`/`claimRelationList`
   yanıtları sıraya bağlı değildi ve seçim değişince liste hemen kalkmıyordu →
   önceki iddianın delil bağı yeni iddianın panelinde görünüyor, geç yanıt yeni
   paneli eziyordu. Düzeltme: `linksSeq` + iddia kimliği değişince anında temizle.
6. **`addLink` iddia kapısı yok:** bağlama uçuşta başka iddiaya geçilince eski
   iddianın bağ listesi yeni iddianın paneli olarak yazılıyordu.
7. **Dosya değişince kaynak seçimi taşınıyordu:** `draftAnchor`, `draftAnchor2`,
   `linkAnchor` birer **çapa kimliği**; taşındığında yeni dosyaya **eski dosyanın
   çapasıyla** aday/bağ yazılabiliyordu (düğme bayat kimlikle açık kalıyordu).
8. **Bayat hata (beş yol):** `load`, `addLink`, `runQuoteCheck`, `bindQuote`,
   `assess` başında `setError(null)` yoktu → düşen işlemden sonra başarılı işlem
   kırmızı bandı ekranda bırakıyordu.
9. **`decide` düzenleyiciyi koşulsuz kapatıyordu:** karar uçuşta başka öğenin
   düzenleyicisi açılıp metin yazılırsa `setEditingId(null)` + `setEditJson("")`
   onu kapatıyor ve **yazılan metni siliyordu**.
10. **Girdi sessizce siliniyordu:** `addClaim`/`addCandidate` yanıtı, uçuşta
    yazılan yeni metni (ve yeni dosyanın taslağını) koşulsuz temizliyordu.

Kapı kanıtı: **18/18 mutasyon** en az bir testi düşürdü (her mutasyon tek satır/
tek blok; geri alma sha256 `23018c2d…` birebir). İlk denemede `addCandidate`
dosya kapısı **yeşil kaldı** — test metni yeniden yazdığı için kimlik kapısı tek
başına yetiyordu; test, metin değiştirmeden dosya değiştirecek şekilde düzeltildi.

Ölçüm: `tsc --noEmit` temiz · `vitest` **841/841** (57 dosya; 822 → **+19**) ·
`pnpm build` temiz (576,85 kB, gzip 155,48 kB).

**Dürüst sınırlar:** jsdom tarayıcı değil — CSS, gerçek Tauri penceresi ve görsel
QA yok; `api` mock'lu olduğu için `claim_create`, `evidence_link_create`,
`review_decide`, `review_commit`, `in_text_quote_check` SQL'i ve Rust **koşmaz**.
`busy` tek bayrak: aynı anda iki yazma başlarsa ilk biten kilidi erken açar
(ölçüldü ve kapatıldı — aşağıdaki "uçuş kilidi ve geç yanıt kapıları" bölümü). Kapsam dışı:
`claimRelationCreate` için ayrı test, klavye, Windows.


## Kronoloji ekranı — uçuş yolları (ek)

`screens/Timeline.tsx` (549 satır, 10 api çağrısı) ekranının 15 testi kararları ve
INV-05'in **dosya değişimi** yüzünü ölçüyordu; istek **uçuştayken** dosya/olay
değişimini ve çift tıklamayı ölçen test yoktu. **15 yeni test** (15 → 30).

Ölçülen ve düzeltilen kusurlar (düzeltme öncesi ham koşu: **15 failed | 15 passed**;
on beşi de gerçek kusur, mevcut 15 test yeşil):

1. **`load` kapısı yok — çapraz dosya yazımı (INV-05):** `eventList` yanıtı koşulsuz
   yazılıyordu; p1 yüklenirken p2'ye geçilince p1'in **geç** yanıtı p2'nin listesini
   eziyordu (tarihsiz bölüm tarihliye dönüyordu).
2. **Geç hata:** aynı kapı hata yolunda da yoktu → okunamayan eski dosyanın `E_IO`'su
   yeni dosyanın altında görünüyordu.
3. **Yazma sonrası tazeleme:** `add`/`saveEdit`/`confirm`/`remove` tazelemesi **tıklama
   anındaki** dosya kimliğiyle koşuyordu → uçuşta dosya değişince eski dosyanın listesi
   yeni dosyaya yazılıyordu (ayrıca boşuna çekirdek okuması yapılıyordu).
4. **Kaynak paneli yanıtı:** `openLink` sıra/kimlik taşımıyordu → e1'in geç yanıtı
   sonradan açılan e2 panelinde çiziliyor, "Bağla" e1'in çapasıyla açılıyordu; aynı
   panel kapatılıp yeniden açıldığında eski isteğin yanıtı yenisini eziyordu.
5. **Kaynak paneli hatası:** geç `E_ANCHOR`, hiç istek yapılmamış panelde görünüyordu.
6. **Bağlama/kaldırma tazelemesi:** `eventLinkSource`/`eventUnlinkSource` sonrası
   `eventSources` tazelemesi panel değiştiyse **yeni olayın** bağlantı listesini
   yazıyordu (e2 paneli boşalıyor; kaldırmada eski olayın bağı yeni panelde beliriyordu).
7. **Bağlama hatası:** geç hata yeni panelde görünüyordu.
8. **Çift tık (dört yol):** onay, sil, bağla ve kaldır düğmelerinde uçuş kilidi yoktu →
   iki tık iki çekirdek yazımı gönderiyordu (ikinci istek yinelenen bağ/`E_NOT_FOUND`
   üretip başarılı sonucun üstüne kırmızı yazabiliyordu).
9. **Yazma uçuşunda form:** besteci ve düzenleyici alanları açıktı, yanıt taslağı
   koşulsuz temizliyordu → uçuşta yazılan metin sessizce siliniyordu.
10. **Kaydetme yanıtı:** `setEditing(null)` koşulsuzdu → uçuşta **başka** satırın
    düzenleyicisi açılırsa yanıt onu kapatıyordu.

Düzeltme (`Timeline.tsx` +65/−27): `caseRef` (dosya kimliği) ve `load` içinde iki kapı
(istek öncesi kimlik, yanıt sonrası kimlik — liste **ve** hata yolu); kaynak paneli için
tek `linkSeq` nesil sayacı (her açılışta artar; `openLink` başarı/hata ile `attach`/
`detach` tazeleme ve hata yolları yalnız kendi neslinde yazar); `confirm`/`remove`/
`attach`/`detach` tek `busy` yazma kilidine alındı (düğmeler `disabled`); besteci ve
düzenleyici alanları yazma uçuşunda kilitli; `saveEdit` yalnız **kendi** kaydının
düzenleyicisini kapatır.

Kapı kanıtı: **17/17 tek-kapı mutasyonu** en az bir testi düşürdü (M4 nesil artışı 6,
M5 yanıt kapısı 2, diğerleri 1). Geri yükleme sha256
`5082303c9efd32ce8c6337ef4099d2830ec6cd6889c7cb57fff506dd7c0e5a4c` birebir.
**Dürüst:** ilk tasarımda kaynak paneli kapısı **panel kimliği** (`linkForRef`)
üzerindeydi; aynı panel yeniden açıldığında eski yanıt yenisini ezebildiği için kimlik
yetmiyordu → nesil sayacına geçildi. İlk mutasyon betiği vitest çıktısını yanlış
ayrıştırdı (0 kırmızı raporladı) ve `detach` blok sınırını bulamadı; betik düzeltilip
17/17 **tek başına** yeniden ölçüldü.

Ölçüm: `tsc --noEmit` temiz · `vitest` **997/997** (60 dosya; 982 → **+15**) ·
`pnpm build` temiz (**581,77 kB**, `index-eLVoT5UJ.js`, gzip **156,84 kB**).
Rust değişmedi; rustfmt iki manifestte `--check` temiz.

**Kimlik:** şartnamedeki **F08-T04** (çok kaynaklı manuel olay oluşturma, düzenleme ve
kaynak açma akışı). README tablosundaki numaralandırma şartnameden kayıktır; bu turda
şartname numarası kullanıldı.

**Dürüst sınırlar:** jsdom tarayıcı değil — gerçek Tauri penceresi, gerçek işaretçi
çift tıklaması ve **görsel QA yok**; `api` mock'lu olduğu için `event_list`,
`event_link_source`, `event_unlink_source` SQL'i ve Rust **koşmaz**. `busy` tek bayrak:
yazma uçuşunda besteci/düzenleyici alanları kilitlenir (sessiz kayıp yerine kısa
bekleme), bu sırada **dosya seçici açık kalır** (kapılar ölçüldü). Panel nesli yalnız
**açılışta** artar; paneli kapatmak tek başına eski yanıtı geçersiz kılmaz (kapalı panel
çizilmediği için görünür etkisi yok; yeniden açılışta liste zaten temizlenir) — açık
kayıt. Bağlama/kaldırma başarılı olup tazeleme düşerse arayüz kırmızı hata yazar ve
liste eski kalır (yazma gerçekleşmiş olabilir; dürüst, yeniden denenebilir). Kapsam
dışı: klavye, Windows.

## Görevler ekranı — yazma yolları ve uçuş kapıları (ek)

`screens/Tasks.tsx` (299 satır, 17 test) bugüne dek kararları, çizimi ve bayat durumları
ölçüyordu; **yazma yollarının uçuş davranışı** (hangi dosyaya/süzgeçe hangi yanıtın
yazıldığı, uçuşta yazılan form metni, çift tık) hiç ölçülmemişti. **19 yeni test**
(17 → 36).

Ölçülen kusurlar (düzeltme öncesi bileşen, nihai 36 test: **19 failed | 17 passed** —
on dokuzu da gerçek kusur; mevcut 17 test düzeltme öncesinde de yeşildi):

1. **Sessiz kayıp (oluşturma):** `add()` başarısı başlık/tarih/politika alanlarını
   **koşulsuz** temizliyordu → yanıt beklerken yazılan yeni görev taslağı siliniyordu.
2. **Çapraz dosya yazımı (INV-05):** oluşturma sonrası tazeleme **tıklama anındaki**
   dosyayla koşuyordu → uçuşta dosya değişince eski dosyanın listesi yeni dosyanın
   ekranına yazılıyordu.
3. **Geç hata:** oluşturma hatası dosya değiştikten sonra yeni dosyada görünüyordu.
4. **Çift yazma ("Bitti"/"Sil"):** iki düğmede de uçuş kilidi yoktu → çift tık iki
   `task_complete` / `task_delete` gönderiyordu.
5. **Enter kilidi atlıyor:** `onKeyDown` `busy`yi sormuyordu → uçuşta ikinci
   `task_create`.
6. **Satır düğmeleri form yazması sürerken açıktı:** oluşturma uçuşunda "Bitti"/"Sil"
   ikinci yazma gönderiyordu.
7. **Donmuş süzgeç:** yazma sonrası tazeleme yakalanan `includeDone` ile koşuyordu →
   uçuşta "Tamamlananlar" değişince liste eziliyordu.
8. **Kilit devri yok:** dosya değişimi `busy`yi bırakmıyordu → yeni dosyanın "Ekle"
   düğmesi eski isteğin dönmesini bekliyordu.
9. **Erken açılan kilit:** eski yazmanın `finally`si koşulsuz `setBusy(false)`
   çağırıyordu → yeni dosyada başlatılan yazmanın kilidini açıyordu.
10. **Kuyruk yanıtı/hatası:** `taskRemindersEnqueue` yanıtı ve hatası dosya kapısı
    taşımıyordu → geç yeşil "N hatırlatıcı yazıldı" ya da `E_DB` yeni dosyada.
11. **Onay (tarih) yanıtı/hatası:** `taskConfirmDate` aynı kapıları atlıyordu.
12. **Silme yanıtı/hatası:** `taskDelete` aynı kapıları atlıyordu.
13. **Tamamlama yanıtı/hatası/kapanışı:** `taskComplete` aynı kapıları atlıyordu.

Düzeltme (`Tasks.tsx` +65/−18): `caseRef` (dosya kimliği) + `doneRef` (o anki süzgeç);
beş yazma yolu (oluştur/onayla/kuyruk/tamamla/sil) tek `busy` kilidine ve nesil
sayacına (`busySeq`) bağlandı — yanıt yalnız isteğin dosyası hâlâ seçiliyse yazar,
tazeleme o anki süzgeçle koşar, kapanış yalnız kendi neslinde kilidi bırakır;
dosya/süzgeç değişimi kilidi devreder; oluşturma formu yalnız **gönderilen** değerler
hâlâ ekrandaysa temizlenir; Enter yolu işleyici kapısından geçer; "Bitti"/"Sil"
düğmeleri uçuşta kapalı.

Testler: `Tasks.test.tsx` **17 → 36** (+19). Ham koşu (düzeltme öncesi bileşen, nihai
36 test): **19 failed | 17 passed**.

Kapı kanıtı: **19/19 tek kapı mutasyonu kırmızı**; geri yükleme sha256
`4ae79c07fec6e7cdb934d78108c92e978baf8bf08f20ffd7e3f1f5e2acdcc204` birebir.
M1 taslak koruması, M2 oluşturma başarı dosya kapısı, M3 oluşturma hata kapısı,
M4/M5 "Bitti"/"Sil" düğme kilidi (2'şer test), M6 Enter kilidi, M7 tamamlama süzgeci,
M8 kilit devri (**3**), M9 oluşturma kapanış nesli, M10/M11 kuyruk başarı/hata,
M12/M13 onay başarı/hata, M14/M15 silme başarı/hata, M16/M17/M18 tamamlama
başarı/hata/kapanış, M19 oluşturma tazeleme süzgeci. **Dürüst:** ilk yazımda
`confirmDate`/`complete`/`remove`/`enqueue` işleyicilerinde ayrıca `if (busy) return;`
ve effect'te `busySeq.current += 1` vardı; ikisi de tek başına mutasyonla **yeşil
kaldı** (düğme `disabled` ve her yazmanın kendi nesli zaten kapsıyordu) → koddan
çıkarıldı, yedekli kapı bırakılmadı. `doneRef` beş yolda aynı ifadedir; ikisi
(oluşturma, tamamlama) ayrı testlerle ölçüldü, kalan üç yol aynı kodu taşır.

Ölçüm: `tsc --noEmit` temiz · `vitest` **1062/1062** (60 dosya; 1043 → **+19**) ·
`pnpm build` temiz (**585,05 kB**, `index-C8PZAXn7.js`, gzip **157,83 kB**; CSS
`index-CPW4QqlF.css`). Rust değişmedi; rustfmt gerekmedi (Rust dosyası yok).

**Kimlik:** şartnamedeki **F08-T10** (kullanıcı onaylı görev tarihi ve yerel
hatırlatıcı); README tablosundaki numaralandırma şartnameden kayıktır.

**Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek işaretçi
çift tıklaması ve **görsel QA yok**; `api` mock'lu olduğu için `task_list`,
`task_create`, `task_complete`, `task_delete`, `task_confirm_date`,
`task_reminders_enqueue` SQL'i ve Rust **koşmaz**. `busy` **tek bayrak**: beş yazma
yolu birbirini bekler (kuyruk değil; ikinci tıklama düğmede kapalı). Kilit devri
**arayüz düzeyinde** — doğrudan IPC bu kapıları atlar. Çift tık `disabled` + işleyici
kapısıyla ölçülür; gerçek işaretçi zamanlaması ölçülmedi.

**Açık kayıt:** (a) dosya değişiminde form alanları korunur (kullanıcı girdisi) — eski
dosya için yazılan başlık yeni dosyada durur; (b) aynı başlık yeniden yazıldıysa
"değişmedi" sayılır ve alan temizlenir (bilinçli); (c) kuyruk başarısı yazılıp tazeleme
düşerse yeşil satır + kırmızı hata birlikte görünür (ikisi de doğru); (d) görsel QA
insan bekliyor.

## İnceleme ekranı — uçuş kilidi ve geç yanıt kapıları (ek)

`Review.tsx` (1239 satır) ekranında önceki iki tur **dosya/iddia kimliği** ve **taslak
koruması** yüzünü kapatmıştı; defterde açık kalan şey yazılıydı: *"`busy` tek bayrak: aynı
anda iki yazma başlarsa ilk biten kilidi erken açar (ölçülmedi)"* ve karar/kabul/yerine
geçme yollarının **geç hata** kapıları. Bu tur onu ölçtü. **9 yeni test** (26 → 35).

Ölçülen ve düzeltilen 9 kusur (düzeltme öncesi bileşen + nihai test dosyası:
**9 failed | 26 passed**; dokuzu da gerçek kusur, mevcut 26 test düzeltme öncesinde de
yeşildi):

1. **Kilit devri yok (en ağırı):** yazma uçuştayken dosya değişince `busy` bırakılmıyordu
   → yeni dosyanın "Ekle" düğmesi eski isteğin yanıtına kadar, yanıt hiç gelmezse
   **süresiz** kapalı kalıyordu. Kırmızı: beklenen `disabled=false`, gelen `true`.
2. **Erken açılan kilit:** sekiz yazma yolunun `finally`si **koşulsuz** `setBusy(false)`
   çağırıyordu → devredilen isteğin kapanışı, yeni dosyada başlatılan yazmanın kilidini
   açıyordu (ikinci yazma kapısı kalkar).
3. **Geç hata — karar:** `decide` hata yolunda dosya kapısı yoktu → eski dosyanın `E_DB`'si
   yeni dosyanın altında kırmızı bant oluyordu.
4. **Geç hata — kabul:** `commit` aynı kapıyı atlıyordu (`review_commit` reddi).
5. **Geç hata — yerine geçme:** `supersede` de kapısızdı (`review_supersede` reddi).
6. **Sessiz kayıp — değerlendirme:** `assess` başarısı hükmü **ve** gerekçeyi koşulsuz
   temizliyordu → uçuşta seçilen yeni hüküm ve yazılan yeni gerekçe yanıtla siliniyordu.
7. **Sessiz kayıp — kabul hedefi:** hedef kayıt türü seçicisi uçuşta açıktı, yanıt türü
   koşulsuz sıfırlıyordu (iddia türü alanı dahil).
8. **Sessiz kayıp — yerine geçme hedefi:** uçuşta başka eski kayıt seçilirse yanıt seçimi
   sıfırlıyordu.
9. **Bayat tazeleme hatayı siliyordu:** `load`ın başındaki `setError(null)` kapısızdı →
   eski dosyanın **geç** tazelemesi (yazma sonrası) yeni dosyanın okuma hatası bandını
   ekrandan siliyordu.

Düzeltme (`Review.tsx` +77/−45): `busySeq` + `beginWrite()/endWrite(seq)` — kapanış yalnız
**kendi** neslinde kilidi bırakır; dosya değişimi kilidi **devreder** (nesil artar);
`decide`/`commit`/`supersede` istek anındaki dosyayı (`asked`) saklar ve hata yalnız o
dosya hâlâ seçiliyse yazılır; değerlendirme hükmü/gerekçesi, kabul hedef türleri ve
yerine geçme hedefi yalnız **gönderilen** değer hâlâ yazılıysa temizlenir; `load`
başındaki hata temizliği `fresh()` kapısına alındı.

Testler: `Review.test.tsx` **26 → 35** (+9; yeni `describe` "inceleme uçuş kilidi ve geç
yanıt kapıları", `vi.resetAllMocks()` — bir kezlik kuyruk taşınmasın; `setupTwoCases()`,
`switchToC2()`, `fillClaimForm()`).

Kapı kanıtı (**11/11 tek kapı mutasyonu kırmızı**; geri yükleme sha256
`1c7cb4e658ec85ce3decfc80ab2be0fae3b477146cb8215e5380373318f4154f` birebir): M1 kilit
devri (**2** test), M2 `endWrite` kapısı (1), M3 `load` hata temizliği (1), M4 `decide`
hata kapısı (1), M5a `commit` hata kapısı (1), M6a `supersede` hata kapısı (1), M7 hüküm
temizliği (1), M8 gerekçe temizliği (1), M9 hedef türü (1), M10 iddia türü (1), M11
yerine geçme hedefi (1). **Dürüst:** (a) M1'in düşürdüğü ikinci test **bağımlı** — kilit
devri olmadan ikinci yazma hiç başlamıyor; asıl erken-açılma kapısını M2 ölçer. (b) İlk
mutasyon koşusunda M5/M6 **iki kapıyı birden** değiştirdi (temizlik + hata kapısı) →
tek kapı olarak M5a/M6a yeniden ölçüldü, her biri 1 kırmızı.

**Dürüst ortam notu (CI):** kod commit'inin ilk koşusu (`41920314563`) `arayüz` işinde
**kırmızı** düştü, ama düşen test bu ekranın değil `Ai.test.tsx`in **zamana bağlı
yarışıydı**: künye okuması bir `effect`te başlıyor, seçicinin değeri ise render olur
olmaz hazır — yüklü CI koşucusunda assertion istek gitmeden koştu (`0 calls`). Test
yerelde 3/3 ve tam pakette **1080/1080** yeşildi; başarısız iş `gh run rerun --failed`
ile **7/7 yeşile** döndü. Ayrı commit'te (`e35407d`) assertion `waitFor`a alındı;
kapısı ölçüldü: ürün kodundan `aiOverview` çağrısı kaldırılınca o test **kırmızı**
(1 failed | 37 skipped), geri yükleme sha256 `f7b4a2c6…4fafb6` birebir.

Ölçüm: `tsc --noEmit` temiz · `vitest` **1080/1080** (60 dosya; 1071 → **+9**) · `pnpm
build` temiz (**585,85 kB**, `index-B68vL9xk.js`, gzip **158,09 kB**; CSS
`index-CPW4QqlF.css`). Rust değişmedi; rustfmt gerekmedi (Rust dosyası yok).

**Kimlik:** şartnamedeki **F08-T04** (iddia–delil matrisi, inceleme kuyruğu,
kabul/ret/düzelt geçmişi); ekranın yazma yolları bu maddenin arayüz yüzüdür.

**Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek işaretçi
çift tıklaması, gerçek klavye ve **görsel QA yok**; `api` mock'lu olduğu için
`review_decide`, `review_commit`, `review_supersede`, `contradiction_assess`,
`claim_create` SQL'i ve Rust **koşmaz**. Kilit **arayüz düzeyinde**: doğrudan IPC bu
kapıları atlar. `busy` **tek bayrak** (kuyruk değil): uçuştaki ikinci yazma düğmede
kapalı. Seçiciler uçuşta **bilinçli olarak açık** (yazılan kaybolmasın diye); ölçülen şey
yanıtın **gönderilen değeri** ezmemesidir. Yanıt **sırası** elle çözülen sözlerle
denetlenir, gerçek gecikmeyle değil.

**Açık kayıt:** (a) `addLink`/`addClaimRelation` hata yolu yalnız **dosya** kapısı taşır —
aynı dosyada başka iddiaya geçilirse eski iddianın bağ hatası yeni iddianın panelinde
bant olarak görünür (ölçülmedi, kapatılmadı; bağ listesinin kendisi kapılı); (b)
`assess`in `setAssessments` yazımı dosya kapısı taşımaz — öğe kimlikleri dosyalar arasında
çakışmadığı için görünür etkisi yok (bilinçli); (c) dosya değişiminde form alanları
(iddia metni, aday metni) korunur — kullanıcı girdisi; (d) görsel QA **insan bekliyor**.


## Kronoloji ekranı — yazma kilidi devri (ek)

`Timeline.tsx`in 30 testi kararları, dosya kimliğini ve panel sırasını ölçüyordu; **yazma
kilidinin dosya değişiminde devri** ve **yazma hatalarının dosya kapısı** hiç ölçülmemişti.
**6 yeni test** (30 → 36). Düzeltme öncesi ham koşu (yeni testler, eski bileşen):
**6 failed | 30 passed** — altısı da gerçek kusur; mevcut 30 test yeşil.

Ölçülen kusurlar:

1. **Kilit devri yok (en ağırı):** yazma uçuşunda dosya değişince `busy` bırakılmıyordu →
   yeni dosyanın "Olay ekle" düğmesi ve besteci alanları eski isteğin yanıtına kadar —
   yanıt hiç gelmezse **süresiz** — kilitli kalıyordu.
2. **Erken açılan kilit:** altı yazma yolunun `finally`si koşulsuz `setBusy(false)`
   çağırıyordu → devredilen isteğin kapanışı, yeni dosyada başlatılan yazmanın kilidini
   açıyordu (ikinci yazma kapısı kalkar).
3. **Sessiz kayıp (taslak):** `add()` başarısı taslağı koşulsuz `EMPTY` yapıyordu → kilit
   devredildikten sonra yeni dosyada yazılan taslak eski isteğin yanıtıyla siliniyordu.
4. **Geç hata (yazma yolları):** `add`/`saveEdit`/`confirm`/`remove` hata yolları dosya
   kapısı taşımıyordu → eski dosyanın `E_IO`'su yeni dosyanın altında kırmızı bant.
5. **Geçici hatada düzenleyici kapanıyordu:** `saveEdit` çakışma dışındaki hatada da
   paneli kapatıp listeyi yüklüyordu → kullanıcının yazdığı başlık/açıklama **sessizce
   kayboluyordu** (yalnız `E_CONFLICT` kaydın sunucuda değiştiğini gösterir).
6. **Kaynak paneli nesli dosya değişiminde artmıyordu:** uçuştaki bağlama/kaldırma hatası
   (ve yanıtı) yeni dosyanın ekranına yazılabiliyordu.

Düzeltme (`Timeline.tsx` +78/−32): `busySeq` + `beginWrite()/endWrite(seq)` — kapanış
yalnız **kendi** neslinde kilidi bırakır; dosya değişimi kilidi **devreder**
(`busySeq += 1`, `setBusy(false)`) ve kaynak paneli neslini artırır (`linkSeq += 1`);
`add` gönderilen dosyayı ve taslağı (`sameDraft`) dondurur, taslağı yalnız gönderilen
taslak hâlâ ekrandaysa temizler; `add`/`saveEdit`/`confirm`/`remove` hata yolları
`caseRef.current === asked` kapısına alındı; `saveEdit` paneli yalnız `E_CONFLICT`te
kapatır (geçici hatada metin korunur, hata görünür).

**Kapı kanıtı 6/6** (her koşu tek kapı; geri yükleme sha256
`abb6f11a5300f4a4989816e7f17f4cb188ed94d1c0f3564ce155febe0887fa74` birebir):
M1 kilit devri (2), M2 `endWrite` kapısı (1), M3 taslak koruması (1), M4 onay hatası
dosya kapısı (1), M5 çakışma kapısı (1), M6 panel nesli (1). **Dürüst:** M1'in ikinci
kırmızısı **bağımlı** — devir olmadan yeni dosyada ikinci yazma hiç başlamıyor; asıl
erken-açılma kapısını M2 ölçer.

Ölçüm: `tsc --noEmit` temiz · `vitest` **1136/1136** (60 dosya; 1130 → **+6**) ·
`vite build` temiz (**592,34 kB**, `index-BwkBa1eR.js`, gzip **159,81 kB**; CSS
`index-CPW4QqlF.css` 7,28 kB). Rust değişmedi; rustfmt gerekmedi (Rust dosyası yok).

**Kimlik:** şartnamedeki **F08-T04** (çok kaynaklı manuel olay oluşturma, düzenleme ve
kaynak açma akışı); bu tur ekranın yazma kilidi yüzüdür.

**Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek işaretçi
çift tıklaması ve **görsel QA yok**; `api` mock'lu olduğu için `event_create`,
`event_update`, `event_confirm`, `event_delete`, `event_link_source`,
`event_unlink_source` SQL'i ve Rust **koşmaz**. `busy` **tek bayrak** (kuyruk değil):
uçuşta besteci/düzenleyici alanları kilitli kalır (kilit devri yalnız **dosya
değişiminde**); doğrudan IPC bu kapıları atlar. Yanıt sırası elle çözülen sözlerle
denetlenir, gerçek gecikmeyle değil.

**Açık kayıt:** (a) dosya değişiminde besteci taslağı **korunur** (kullanıcı girdisi) —
p1 için yazılan başlık p2 formunda durur ve "Olay ekle" ile p2'ye yazılabilir (diğer
ekranlarla aynı sözleşme; bilinçli, ölçülmedi); (b) `E_CONFLICT` dışındaki hatada
düzenleyici açık kalır; kayıt gerçekten silinmişse (`E_NOT_FOUND`) kullanıcı aynı hatayı
yeniden görür (kapatma yolu yok, panelden çıkabilir); (c) kaynak paneli nesli dosya
değişiminde artar → aynı anda uçuşta olan `openLink` yanıtı düşer (panel zaten kapalı);
(d) görsel QA **insan bekliyor**.

## İnceleme ekranı — karar geçmişi görünür (ek, 2026-11-08)

**Ölçülen kusur (yazılıyor, okunmuyor):** `review_decisions` her kararda satır yazıyor
(`decide_review_item`, alanlar: karar, `edited_payload`, karar veren, gerekçe, zaman) ve
`review_history` komutu + `api.ts` bağı **F08'den beri** vardı; arayüzde tek çağrı yoktu
(ölçüm: `grep -rn "reviewHistory" apps/desktop/src` → yalnız `lib/api.ts` tanımı). Şartname
E2E-06 açıkça *"karar geçmişi görünür"* der; F08-T04 satırı "kabul/ret/düzelt geçmişi" diye
işaretliydi. Sonuç: "delil iste" gerekçesi, sonraki kararla birlikte ekrandan **siliniyordu**
(kalem `needs_more_evidence` olarak kuyrukta beklerken bile "neden delil istendi" görünmüyordu);
birden çok düzeltmenin içeriği ve karar veren kimliği ulaşılamazdı. Çoklu satır **ölçüldü**:
`decide_review_item` yalnız `pending`/`needs_more_evidence` durumundan karar kabul eder, yani
`delil iste → kabul` iki satır üretir (mevcut test `review_item_requires_source_and_is_decided_once`
final karardan sonra reddi doğrular).

**Yazılan.** `Review.tsx` (+218/−14):

- Her **karar verilmiş** kalemde ve kuyrukta **delil bekleyen** kalemde "Geçmişi göster" düğmesi;
  geçmiş **istek üzerine** okunur (liste açılışında kalem başına N+1 okuma yok — ölçüldü:
  açılışta `reviewHistory` çağrısı **0**).
- Kapılar: yanıt yalnız **aynı dosya** ve **en son istek** için yazılır (`caseRef` + `historySeq`);
  dosya değişiminde panel kimliği, satırlar, hata ve uçuştaki okuma kimliği **birlikte** düşer.
  Panel kapanıp yeniden açılırsa yeni nesil başlar: eski yanıt görünmez. Açılışta önceki satırlar
  gösterilmez (bayat liste "yeni okuma" gibi sunulmaz).
- Karar/kabul/yerine geçme başarılıysa **paneli açık kalemin geçmişi tazelenir**.
- Dürüstlük: düşen okuma **"kayıt yok" demez** (kendi `role=alert` satırı, liste düşer);
  tanınmayan karar kodu **ham** gösterilir; karar veren + gerekçe + zaman yazılır; düzenlenmiş
  içerik JSON olarak gösterilir.

`crates/storage/src/claims.rs`: `ReviewDecisionRow.edited_payload_invalid` eklendi — `edited_payload`
sütunu **çözülemeyen** JSON taşıyorsa `true` (payload yoksa `false`). Arayüz bu durumda
"Düzenlenen içerik okunamadı (kayıt bozuk)." yazar; bozuk kayıt "düzenleme yok" gibi gizlenmez.
Sıra değişmedi (`julianday(decided_at) ASC, rowid ASC`).

**Testler.** `Review.test.tsx` **35 → 42** (+7; yeni `describe` "karar geçmişi") ·
`claims::tests::review_history_marks_unreadable_edited_payload` (+1) → `dim-storage --lib`
**437 → 438**.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-review-history-gates.py`).** Taban 7/7 yeşil;
**eski kodda** (HEAD `Review.tsx`) yeni testlerin **7/7'si kırmızı**. Mutasyonlar **7/7 kırmızı**,
her biri tek başına (`-t` ile yalnız kendi testi): M1 panel açılışında okuma yok (1) ·
M2 istek nesli kapısı yok (1) · M3 hata "kayıt yok" gibi gösterilir (1) · M4 tanınmayan karar kodu
uydurma etikete çevrilir (1) · M5 karardan sonra geçmiş tazelenmez (1) · M6 dosya değişiminde
panel/kimlik düşmez (1) · R1 bozuk payload bayrağı yanmaz (1). Geri yükleme sha256 birebir
(ui `ffad9f13…`, rs `d9c7680…`). **Dürüst (kendi hatam):** kapı betiğinin ilk sürümü filtreli
koşuda vitest'in `N failed | M skipped` biçimini okumadı (regex yalnız `… passed` arıyordu) →
M1 kırmızı olduğu hâlde "YEŞİL" sanıldı ve betik erken durdu; ayrıştırıcı `failed`/`passed`
alanlarını ayrı arayacak biçimde düzeltildi, 7/7 yeniden ölçüldü.

**Ölçüm (yerel, Node 22.23.1 + TZ=UTC).** `tsc --noEmit` temiz · vitest **1297/1297** (63 dosya;
yerel fazlalık ağaçtaki **başka ajanın** commit'siz `format.test.ts` testi) · `cargo test -p
dim-storage --lib claims::tests::review_history` **1/1** · `cargo fmt` iki manifest temiz.
Yerel `vite build` koşulmadı (disk eşiği) → paket ölçümü CI'dan.

**CI (ölçüldü).** Kod `8440e87` → **`42427139500` 10/10 YEŞİL**: `arayüz` **63 dosya / 1296 test**
(yerel 1297 − ağaçtaki başka ajanın testi), paket `index-CvBUy_dC.js` **637,91 kB** / gzip
**171,16 kB** (CSS 7,28 kB / gzip 2,07); `dim-storage` **438** hem `ubuntu-latest` (479,54 s) hem
`macos-15` (200,56 s) — **birebir**; `masaüstü kabuk` `40 passed · 5 ignored` + yoksayılan koşu
`5 passed · 40 filtered` (41,97 s); diğer yedi iş yeşil.

**Dürüst sınırlar.** (a) `review_history` komutu **`item_id`** ile çalışır, `case_id` taşımaz —
doğrudan IPC, kimliği bilinen başka dosyanın kalem geçmişini okuyabilir (diğer kalem okumalarıyla
aynı sözleşme); arayüz yalnız o dosyanın kalemlerini gösterir. (b) Geçmiş **salt-okunur**: silinemez,
düzeltilemez. (c) **Dosya geneli** toplu karar listesi yok — okuma kalem başına. (d) Bozuk
`edited_payload` **işaretlenir**, onarılmaz. (e) Karar sayısı pratikte 1–2 (durum makinesi final
karardan sonra karar kabul etmez); geçmiş "denetim izi" değil, kalemin kendi karar kaydıdır.
(f) jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.

## Dosya tarafları — kişi/kurum, rol ve geri alınabilir birleştirme (ek, 2026-11-08)

**Ölçülen boşluk (şartname F08-T01 / REQ-010).** Şartname "kişi/kurum ve dosya
rolünü ayır; birleştirmeyi inceleme ve **geri alma** işlemine bağla" der; tablo
sözleşmesi (`actors`: `display_name`, `kind`, `normalized_names`, `review_state`,
`merged_into_actor_id`; `case_participants`: `case_id`, `actor_id`, `role`,
`role_note`, `valid_from/to`) şemanın ilk sürümünden beri var. Ama kodda **tek SQL
yoktu** (`git grep`: iki tabloya tek çağrı yok) — üründe taraf/rol kaydı,
birleştirme ve geri alma hiç yoktu. Bu README'nin tablosundaki `F08-T01` satırı
**olay tarihi** işini anlatıyor; şartname numaralandırmasıyla çakışıyordu (aşağıdaki nota bakın).

**Yazılan — çekirdek (`crates/storage/src/actors.rs`, şema v25).**
- `add_case_party(case_id, …)` — aktör + katılımcı **tek işlemde** yazılır; katılımcı
  yazımı düşerse yeni aktör de geri alınır (sahipsiz kişi kaydı kalmaz). Ad/rol
  kırpılır; boş, kontrol karakterli veya sınır üstü (ad 200 · rol 80 · not 500)
  değer reddedilir. Tür **kapalı küme** (`person|organization|unknown`).
  `actorId` verilirse **mevcut** tarafa rol eklenir (ad uydurulmaz); birleştirilmiş
  tarafa yeni rol `E_DB_ACTOR` ile reddedilir.
- Aynı dosyada aynı taraf + aynı rol ikinci kez yazılmaz (tablo UNIQUE'i); ihlal
  **ham SQL hatası olarak sızmaz**, `E_DB_ACTOR` + "zaten kayıtlı" olur.
- `list_case_parties(case_id)` — **kayıt sırası** (`created_at, rowid`), en çok 200
  satır. `remove_case_participant` geri dönüşümlü siler (`deleted_at`), denetim
  kaydı kalır.
- `merge_actors(source, target, reviewer)` — çakışma **ön denetimi**: aynı dosyada
  aynı rol iki tarafta da kayıtlıysa birleştirme reddedilir (sessiz yinelenen rol
  yok). Roller hedefe taşınır, kaynak `merged` işaretlenir; `actor_merges` kaydı
  kaynak, hedef, **taşınan satır kimlikleri**, kaynağın **önceki inceleme durumu**
  ve birleştiren kimliğini saklar. `undo_actor_merge` rolleri kaynağa döndürür ve
  önceki durumu **birebir** yazar; birleştirmeden sonra elle silinmiş roller geri
  gelmez (silme bilinçli kullanıcı eylemidir).
- Yeni komutlar: `case_party_list`, `case_party_add`, `case_party_remove`,
  `actor_merge`, `actor_merge_undo` (+ `api.ts` bağları ve tipleri).

**Arayüz (`components/Parties.tsx`; İnceleme ekranında dosya altında).** Taraf
tablosu (ad · tür · rol · rol notu), yeni taraf formu (mevcut tarafa ikinci rol
seçilebilir), **onay kutusu** işaretliyken çalışan birleştirme ve **çekirdekten
okunan** birleştirme geçmişi + "Geri al". Dosya değişince liste, form, seçim ve
bildirim birlikte düşer (INV-05); düşen okuma "taraf yok" **demez** (`role=alert`),
geç yanıt `seq` kapısıyla düşer; bilinmeyen tür **ham** gösterilir.

**Testler.** storage `actors::tests` **10** (yeni) → `dim-storage --lib` **442 →
452**; `Parties.test.tsx` **11** (yeni) → yerel arayüz **1308** (64 dosya; fazlalık
ağaçtaki **başka ajanın** commit'siz `format.test.ts` testi) · `Review.test.tsx`
harness'ı `casePartyList` mock'uyla güncellendi (42 test değişmedi).

**Kapı kanıtı (gerçek koşular; betikler `scratch/dim-parties-gates*.py`; her
mutasyon tek başına, geri yükleme sha256 birebir).** Çekirdek **7/7 kırmızı** —
R1 roller taşınmaz · R2 çakışma ön denetimi kapalı (birleştirme ham `E_DB_SQL`e
düşer; kullanıcı "çakışma" görmez) · R3 geri alma rolleri döndürmez · R4
birleştirilmiş tarafa rol eklenir · R5 yinelenen rol `E_DB_ACTOR` yerine SQL hatası
· R6 liste sırası ters · R7 silinen satır listede kalır. Arayüz **7/7 kırmızı** —
U1 ekleme çekirdeğe gitmez · U2 onay kapısı kapalı · U3 geri alma çekirdeğe gitmez
· U4 dosya değişiminde liste temizlenmez · U5 okuma hatası yutulur · U6 geç yanıt
kapısı · U7 aynı taraf seçimi kapalı değil.
**Dürüst (kendi hatam, iki tanesi):** (1) U4 ilk ölçümde **yeşil kaldı**:
temizleme iki yerde (effect + okuma) yapılıyordu, biri kaldırılınca diğeri testi
geçiriyordu → yük taşımayan effect satırı **kaldırıldı**, U4 tek kapıyla yeniden
ölçüldü (kırmızı). (2) Arayüz betiğinin ilk ayrıştırıcısı filtreli vitest özetini
(`1 failed | 10 skipped`) okumadı; `failed`/`passed`/`skipped` ayrı aranacak
biçimde düzeltildi ve yedi ölçüm yenilendi (ilk koşunun "kırmızı"ları kanıt
sayılmadı).

**Ölçüm (yerel, Node 22.23.1 + TZ=UTC).** `tsc --noEmit` temiz · vitest
**1308/1308** (64 dosya) · `cargo test -p dim-storage --lib` **452/452** (230,95 s)
· `cargo fmt` iki manifest temiz · `cargo clippy -p dim-storage --all-targets`
ve masaüstü clippy `-D warnings` temiz · `cargo check --all-targets` (masaüstü)
temiz. Yerel `vite build` koşulmadı → paket ölçümü CI'dan.

**Commit/CI (ölçüldü).** Kod `91cf11b` (13 dosya, **+1834/−19**) → **`42486765219`
10/10 YEŞİL**: `arayüz` **64 dosya / 1307 test** (yerel 1308 − ağaçtaki başka
ajanın commit'siz `format.test.ts` testi), paket `index-B8lzUIqY.js` **647,83 kB**
/ gzip **173,29 kB** (CSS 7,28 kB / gzip 2,07); `dim-storage` **452** hem
`ubuntu-latest` (298,48 s) hem `macos-15` (187,10 s) — **birebir**; `masaüstü
kabuk` `40 koştu · 0 düştü · 5 yoksayıldı (kaynak 45)` + yoksayılan koşu
`5 passed`; diğer altı iş yeşil.

**Dürüst sınırlar.** (a) Birleştirme **dosya kapsamı dışında** da yapılabilir
(doğrudan IPC); arayüz seçimini o dosyanın taraflarıyla sınırlar. (b) Geri alma
yalnız **hâlâ hedefte duran** rolleri döndürür — birleştirmeden sonra silinen rol
geri gelmez. (c) `normalized_names` basit küçük harftir; otomatik eşleştirme/öneri
**yoktur**, birleştirmeyi kullanıcı seçer (şartname "otomatik eşleştirme kabul
edilmeden birleşmez" der; bu turda otomatik eşleştirme hiç üretilmiyor). (d)
Birleştirme geçmişi dosya başına en yeni 50 kayıtla sınırlı (küresel tarama 200
satır); daha eski birleştirmeler listede görünmez. (e) Kart **İnceleme** ekranında:
Dosyalar ekranı (`Cases.tsx`) o turda başka bir çalışma hattındaydı (commit'siz
değişiklikler); kartın Dosyalar'a taşınması **ürün kararı**. (f) Rol **serbest
metindir**; kapalı rol listesi (davacı/davalı/vekil…) hukuki ürün kararıdır ve
uydurulmadı. (g) Bozuk bir `moved_participants` JSON'u olan birleştirme kaydı
listede **atlanır** (taraflar listesi kilitlenmez); bozukluk onarılmaz. (h)
`review_state='pending'` durumu bu turda **yazılmıyor** (otomatik aday üretimi yok);
kullanıcı kaydı `accepted` açılır. (i) `case_party_*`/`actor_merge*` **komut olarak
koşulmadı** (`State<AppState>` altyapısı yok) — kablolama cargo check + clippy,
davranış storage düzeyinde **gerçek SQLite** ile; jsdom tarayıcı değil → gerçek
Tauri penceresi ve **görsel QA insan bekliyor**.

**Not (numaralandırma).** Şartname F08-T01 kişi/kurum + rol işidir; bu README'nin
tablosundaki `F08-T01` satırı olay tarihini anlatır. İki numaralandırma arasındaki
fark bu turda görünür kılındı (tabloya `F08-T01 *(şartname numarası)*` satırı
eklendi); eski satırlar **yeniden numaralanmadı** (geçmiş kanıtlar bozulmasın).

## Tarih kesinliği — gün uydurulmadan sıralama (şartname F08-T03)

**Kimlik:** şartnamedeki **F08-T03** (§20.10 satır 1555: *"Kesin, yaklaşık,
aralık ve bilinmeyen tarihleri gün uydurmadan sırala"*) ve §590/§955
("yaklaşık tarih `2026-03` veya aralık olarak gösterilir; **gün uydurulmaz**").
Bu README'nin tablosundaki `F08-T03` satırı onay (`draft → confirmed`) işini
anlatır — numaralandırma farkı yukarıdaki notta.

**ÖLÇÜLEN KUSUR (iki tanesi).**

1. **Gün uydurma.** Ay/yıl kesinliği tam gün zorunlu tutuyor ve tam gün
   gösteriyordu: `datePrecision:"month"` + `dateValue:"2026-05-12"` olduğu gibi
   saklanıyor, ekranda ve CSV'de **2026-05-12** yazıyordu (mevcut test bunu
   pinliyordu). Aralık kesinliği hiç temsil edilemiyordu (tek gün olarak
   saklanıyordu) → "01.03–30.06" aralığı sıralamada tek güne düşüyordu.
2. **Ham tarih kaybı.** Düzenleme, sunucudaki `date_raw`ı ("10 Nisan 2026",
   "01.03.2026") ISO tarihle **eziyordu**: yalnız başlığı düzelten kullanıcı
   kaynak ifadeyi kaybediyordu. Formda ham metin alanı hiç yoktu.

**Yazılan.**

- **Çekirdek:** `StorageError::EventDateInvalid` → yeni kod **`E_EVENT_DATE`**.
  `timeline::normalize_event_date` — gün katı takvim doğrulaması (31 Şubat ve
  "10.04.2026" reddedilir); ay/yıl kesinliğinde gün **saklanmaz** (tam gün
  gelirse kırpılır); aralık `başlangıç/bitiş`, ters sıra `E_EVENT_DATE`;
  yaklaşık girildiği çözünürlükte korunur; bilinmeyen → NULL.
  `sortable_event_date` (aralık **başlangıçla** sıralanır) + `event_date_text`
  (görüntü/çıktı kesinliğe uyar). `create_event`/`update_event` bu
  sözleşmeden geçer.
- **CSV:** `build_case_csv` artık `sortable_date` yerine kesinliğe uygun metni
  yazar; eski kayıtlarda (ay kesinliği + tam gün) da gün **yazılmaz**.
- **Arayüz:** yeni `lib/timeline-date.ts` (`dateInputMode`,
  `isRealDay/isRealMonth/isYear`, `dateDraftIssue`, `composeDateValue`,
  `eventDateText`, `editDateInputs`). `Timeline.tsx` kesinliğe göre giriş
  (gün→`date`, ay/yaklaşık→`month`, yıl→4 hane, aralık→iki `date`,
  bilinmeyen→kapalı), ayrı **"Metinde geçen tarih"** alanı (ekleme +
  düzenleme; `date_raw` korunur), kesinlik değişince tarihin yeni
  çözünürlüğe kırpılması, satırda kesinliğe uygun gösterim. 10 sözlük anahtarı.

**Testler.** storage `timeline::` **12 → 18** (+6: ay/yıl kırpma, gün reddi,
aralık sırası/sıralama, yaklaşık çözünürlük, görüntü metni), `csv_export`
**4 → 5** (+1) → `dim-storage --lib` **503 → 510** (yerel 279,34 s).
`lib/timeline-date.test.ts` **7** (yeni), `Timeline.test.tsx` **36 → 39**
(ay testi yeni sözleşmeye göre yeniden yazıldı; yıl, aralık, boş-gün reddi,
ham-metin koruma testleri eklendi) → yerel tam vitest **65 dosya / 1346 test**.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-timeline-date-gates.py`,
her mutasyon tek başına, `try/finally` + sha256 birebir geri yükleme): 11/11
KIRMIZI** — S1 ay kesinliği günü saklar · S2 gün kesinliği eksik günü kabul
eder · S3 aralık sıra denetimi kapalı · S4 aralık bitişle sıralanır · S5
görüntü metni ay kesinliğinde günü yazar · S6 CSV kesinliği yok sayar ·
U1 ay kesinliğinde gün girişi · U2 düzenlemede ham metin ezilir · U3 aralık
tek değerle gönderilir · U4 gün kesinliğinde boş tarih kapısı kapalı ·
U5 satır görüntüsü kesinliğe uymaz.

**Commit/CI (ölçüldü).** Kod `7979984` (8 dosya, **+960/−60**) → run
`42809915402` **10/10 YEŞİL**: `dim-storage` **510** hem `macos-15`
(242,54 s) hem `ubuntu-latest` (403,01 s) — **birebir**; `arayüz` **65 dosya /
1345 test** (yerel 1346 − ağaçtaki başka ajanın commit'siz `format.test.ts`
testi), paket `index-QaPk0OkB.js` **663,27 kB** / gzip **177,55 kB** (CSS
7,28 kB / gzip 2,07); `masaüstü kabuk` `40 koştu · 0 düştü · 5 yoksayıldı
(kaynak 45)` + yoksayılan koşu `5 passed · 40 filtered`; diğer altı iş yeşil.

**Dürüst sınırlar.** (a) Eski kayıtlar (ay kesinliği + tam gün) görüntüde ve
CSV'de kırpılır ama **veritabanında dönüştürülmedi**; düzenlenip kaydedilince
değer kesinliğe kırpılır (ham metin korunur). (b) Yaklaşık tarih arayüzde
**ay** çözünürlüğünde girilir; gün düzeyinde yaklaşık tarih yalnız çekirdek
sözleşmesinde kabul edilir. (c) Aralık uçları **tam gün** ister (ay uçlu
aralık yok). (d) Ham metin arayüzde 120 karakterle sınırlı (`maxLength`);
çekirdek uzunluğu kırpmaz/reddetmez. (e) Komutlar **komut olarak koşulmadı**
(`State<AppState>` altyapısı yok) — kablolama check+clippy, davranış storage
düzeyinde **gerçek SQLite**. (f) jsdom tarayıcı değil: `type="month"` /
`type="date"` davranışı gerçek Tauri penceresinde (WKWebView) ve **görsel QA
insan bekliyor**. (g) CSV tarih sütunu artık kesinliğe uyar; Excel/LibreOffice
aç–kaydet turu hâlâ **insan/ortam bekliyor** (F14'teki açık madde).

## Yerel bildirim gerçekten teslim edilir — outbox teslim defteri (ek, 2026-11-11)

**ÖLÇÜLEN KUSUR (iki tanesi).** (1) `outbox_events` tablosunun `delivered_at`
sütununu **yazan hiçbir kod yoktu**: `task_reminders_enqueue` ve
`commit_review_decision` satırı `delivered_at NULL` ile açıyor, sonrasında
kimse kapatmıyordu. Yani “bildirim kuyruğa yazıldı” gerçekti ama **hiçbir
bildirim gönderilmiyordu** ve satır sonsuza kadar “bekliyor” kalıyordu
(şartname REQ-028 “görev ve kullanıcı onaylı tarih, **yerel bildirim**”).
(2) `Review` ekranı `review_outbox_pending`in döndürdüğü **tüm** satırları
sayıp “N bildirim bekliyor” yazıyordu; oysa bu satırların bir kısmı
`review.decision_committed` / `review.item_decided` gibi **alan olaylarıdır** —
bildirim değildir, hiçbir zaman kullanıcıya gösterilmez. İki sınıf tek sayıda
toplanınca sayı hem yanlış hem **hiç düşmeyen** bir sayıydı.

**Yazılan — çekirdek (`crates/storage/src/notifications.rs`, yeni).**
- `NOTIFICATION_EVENT_TYPES` **kapalı küme** (bugün `task.reminder_due`):
  bilinmeyen tür “bildirim” sayılmaz; yeni tür eklemek bilinçli karardır.
- `pending_notifications(case_id, limit)` — yalnız bildirim sınıfı, yalnız
  `delivered_at IS NULL`, sıra `created_at ASC, rowid ASC`; `limit` çekirdekte
  `MAX_NOTIFICATIONS_PER_CALL = 50` ile kırpılır.
- `pending_notification_count(case_id)` — aynı kapsamda sayı.
- `mark_notifications_delivered(ids, at)` — **koşullu** `UPDATE … WHERE
  delivered_at IS NULL AND event_type IN (bildirim türleri)`; gerçekten
  değişen satır sayısını döndürür (ikinci çağrı **0**), alan olayı kimliği
  verilirse **hiçbir şey değişmez**.
- `record_notification_failures(ids)` — `attempt_count`ı artırır,
  `delivered_at`ı **açmaz**: gönderilemeyen satır “gönderildi” görünmez.
- `notification_text(policy)` — metin **yalnız politikadan** üretilir:
  “Bugün son günü olan bir göreviniz var.” / “Yarın …” / genel cümle. Görev
  başlığı, dosya adı, taraf adı, kimlik **girmez** (şartname §331: kilit
  ekranı bildirimi genel metindir). Bozuk payload satırı **düşürmez**;
  politika `None` olur ve metin genel cümleye düşer.

**Yazılan — masaüstü.** `tauri-plugin-notification = "2"` (yalnız **Rust**
tarafından kullanılır; JS yüzeyi capability'ye **eklenmedi** — ADR-02 en az
yetki). Yeni komut `notifications_deliver(caseId)`:
izin `permission_state` ile sorulur, `Prompt` ise `request_permission`;
izin yoksa **`E_NOTIFY_PERMISSION`** döner ve **hiçbir satır kapatılmaz**
(“izin yok” ile “gönderildi” karışmaz). Her satır için OS bildirimi
gösterilir; `Ok` → `mark_notifications_delivered`, `Err` →
`record_notification_failures`. Sonuç `{sent, failed, remaining}`.

**Yazılan — arayüz.** `Tasks` ekranındaki “Yerel hatırlatıcılar” kartına
**“Bildirimleri gönder”** düğmesi (`api.notificationsDeliver`): sonuç üç ayrı
cümleyle yazılır — “{sent} bildirim gönderildi · {remaining} bekliyor.”,
“Gönderilen bildirim yok · {remaining} bekliyor.”, “{sent} gönderildi ·
{failed} başarısız · {remaining} bekliyor. Başarısız satır kuyrukta kalır;
yeniden denenebilir.” İzin hatası `role="alert"` satırında **koduyla** görünür.
`Review` kuyruk özeti artık **yalnız hatırlatıcıyı** “bildirim bekliyor” diye
sayar; alan olayları varsa ayrı cümle yazılır: “{count} alan olayı teslim
edilmedi — bunlar bildirim değildir (bildirim gösterilmez, tüketici bekler).”
5 sözlük anahtarı (`tasks.notify*`, `review.outboxDomainNote`).

**Testler.** `notifications::tests` **9** (yeni) → `dim-storage --lib`
**526 → 535** (294,44 s, güncel kodla): alan olayı bildirim sayılmaz ·
teslim idempotent (ikinci çağrı 0) · alan olayı `delivered` kapatılamaz ·
başarısız deneme satırı kapatmaz · metin görev/dosya adı sızdırmaz · bozuk
payload satırı kaybettirmez · limit çekirdekte kırpılır · sıra yazılma
sırasıdır · dosya kapsamı. `Tasks.test.tsx` **+2 → 38**, `Review.test.tsx`
**+1 → 43**; tam vitest **65 dosya / 1370 test** yeşil. `tsc --noEmit` temiz;
i18n bağlama **60/60** (54 + 6); `cargo fmt` iki manifest temiz; storage +
masaüstü `clippy --all-targets -D warnings` temiz.

**Kapı kanıtı (gerçek koşular; betikler `scratch/dim-notify-gates-core.py` ve
`-ui.py`; her mutasyon tek başına, `try/finally` + sha256 birebir —
`notifications.rs 3c2e10d9…`, `Tasks.tsx 5be4bf48…`, `Review.tsx 144ec9ae…`):**
**12/12 KIRMIZI** — S1 alan olayları da bildirim sayılır · S2 teslim kapısı
idempotent değil · S3 alan olayı “teslim edildi” kapatılabilir · S4 başarısız
deneme satırı kapatır · S5 metin görev/dosya adı sızdırır · S6 limit
kırpılmaz · S7 bozuk payload uydurma politikaya düşer · S8 sıra ters;
U1 gönder düğmesi çekirdeği çağırmaz · U2 başarısız satır yine “gönderildi”
yazar · U3 izin hatası yutulur · U4 alan olayları bildirim sayılır.

**CI (ölçüldü).** Kod `03eda2b` (15 dosya, **+1790/−16**) + bildirim tazeleme
`2b4dea7` (`THIRD_PARTY_NOTICES.md`, +2500/−240) → run **`43002028988` 10/10
YEŞİL**: `dim-storage` **535** hem `macos-15` (242,81 s) hem `ubuntu-latest`
(639,46 s) — **birebir**; `arayüz` **65 dosya / 1362 test** (yerel 1370 farkı
ağaçtaki başka ajanın commit'siz testleri), paket `index-Dsl5Bs53.js`
**669,79 kB** / gzip **179,15 kB**; `masaüstü kabuk` 40/0/5; diğer yedi iş
yeşil — **`rust (ubuntu-latest)` dahil**: eklentinin Linux derlemesi (zbus,
saf Rust) doğrulandı. **Dürüst ara koşu:** ilk koşu `43000986797` SBOM işinde
**düştü** (yeni bağımlılıklar `THIRD_PARTY_NOTICES.md`ye girmemişti; kod işleri
yeşildi) → bildirim üretildi, `2b4dea7` ile kapı geçti.

**Dürüst sınırlar.** (a) **Gerçek OS bildirimi bu ortamda koşulmadı**: bildirim
bir Tauri penceresi ve sistem izni ister; jsdom/headless doğrulaması yoktur →
macOS/Win bildirim merkezinde görünme ve izin diyaloğu **insan bekliyor**.
(b) Teslim **kullanıcı eylemiyle** çalışır; arka plan zamanlayıcı yoktur
(uygulama kapalıyken hatırlatıcı gönderilmez) — zamanlama ürün kararı.
(c) Alan olayları (`review.*`) **bilerek teslim edilmez**: tüketicileri yok;
arayüz bunu artık ayrı ve doğru cümleyle söyler, “bildirim” demez.
(d) `attempt_count` artar ama **üst sınır/geri çekilme yoktur**; satır kuyrukta
kaldığı sürece her “gönder” çağrısında yeniden denenir. (e) İzin verilmediğinde
satırlar **kuyrukta kalır** (kaybolmaz); izin sonradan verilirse gönderilir.
(f) Bildirim metni **sabittir** (politika + genel cümle): görev kimliği
gösterilmez, kullanıcı bildirime tıklayınca uygulama içinde ilgili göreve
**gitmez** (tıklama eylemi yok). (g) Komut **komut olarak koşulmadı**
(`State<AppState>` altyapısı yok) — kablolama `cargo check` + `clippy`,
davranış storage düzeyinde **gerçek SQLite**; OS çağrısı derlenir ama
çalıştırılamaz.

## F08-T04e — kanıt ve iddia referansları dosya sınırını aşamaz (şema v32, 2026-11-14)

**Neden.** F05-T03c kapsam kuralını şemaya taşıdı (v31) ama yalnız **zincir**
için: belge → sürüm → koşu → segment → çapa. Dürüst sınırı (b) şuydu: bir
satırın **işaret ettiği** kanıt/iddia hâlâ başka dosyadan olabiliyordu —
`claims.source_anchor_id`, `evidence_links` (`claim_id` + `source_anchor_id`),
`event_sources` (satırda `case_id` yok: olayın dosyası ile çapanın dosyası),
`claim_relations` (iki iddia), `tasks.source_anchor_id`. Çekirdek bu yolların
hepsini kendi sorgusunda doğruluyordu (`anchor_in_case`, `add_event_source`,
`link_claim_relation`), yani **bugünkü ürün yollarında canlı sızıntı yoktu**;
ama doğrudan SQL yazan bir yol (yeni bir komut, bir betik, ileride eklenecek
toplu içe aktarma) kapıyı atlıyordu.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite; geçici sonda, sonra silindi).**
Beş çapraz satır da **kabul edildi** ve `scope_violations()` **0** dedi
(tarama kördü). Sonuç zinciri ölçüldü: **B dosyasının belgesi silinince A
dosyasının kanıt bağı `ON DELETE CASCADE` ile sessizce yok oldu** (1 → 0), olay
kaynağı da (1 → 0), iddianın çapası `SET NULL` ile düştü — denetime **tek
satır** yazılmadı. Yani bir dosyanın kanıtı, sahibi olmadığı bir dosyanın
silinmesiyle kayboluyordu ve hiçbir yerde görünmüyordu. `anchor_locate` gibi
kimlikle okuyan yollar ise A dosyasında B dosyasının belge adını/sayfasını
gösterebilir.

**Yazılan (şema v32, `migrations/0032_reference_scope_guards.sql`).** Kural
şemada: referansın hedefi **kendi dosyasındaki** kayıt olmalı; hedef yoksa satır
yazılmaz (fail-closed). **5 tablo × BEFORE INSERT/UPDATE = 10 tetikleyici**:

- `claims` → `source_anchor_id` (varsa) aynı dosyada
- `evidence_links` → `claim_id` **ve** `source_anchor_id` satırın dosyasında
- `event_sources` → `event.case_id` = `source_anchors.case_id` (satırda `case_id`
  yok; iki üst kayıt karşılaştırılır)
- `claim_relations` → `claim_id` **ve** `related_claim_id` satırın dosyasında
- `tasks` → `source_anchor_id` (varsa) aynı dosyada

Reddetme sabit önekle: `RAISE(ABORT,'DIM_SCOPE:<tablo>')` →
`StorageError::ScopeViolation` → **`E_DB_SCOPE`** (v31 sözleşmesi; hata işlem
içinde oluşur, kısmi satır kalmaz). Kapı **yalnız dosya sınırını** denetler:
çapa durumu (`anchor_status`), `deleted_at`, ilişki türü gibi ürün politikaları
çekirdek sorgusunda kalır — tetikleyici politika öğrenirse eski veri bir anda
"yazılamaz" hâle gelirdi.

**Mevcut veri.** `Database::scope_violations()` artık **13 tabloyu** ayrı ayrı
sayar (8 zincir + 5 referans); kimlik/içerik taşımaz. Güncelleme anında
(`upgrade_with_backup`) toplam sıfırdan büyükse sayı denetime
(`vault_upgrade` / `scope_violations`; yeni `claims`, `evidenceLinks`,
`eventSources`, `claimRelations`, `tasks` alanlarıyla) yazılır.

**Testler.** `scope_guard::tests` **11 → 17** (+6: her tablo için çapraz
INSERT + UPDATE ile taşıma, meşru yolun **yeşil** kaldığı, eski satırı sayan
tarama, **şemadan türetilen** "her kapsam tablosunun kapısı var mı") +
`upgrade_tests::upgrade_reports_cross_case_references_left_by_the_previous_schema`
(v31 kasa → güncelleme → denetimde beş ayrı sayı; yeni çapraz yazma reddedilir)
→ `dim-storage --lib` **619 → 626** (yerel 383,03 s, 0 düştü). `cargo fmt`
temiz; tsc/clippy etkilenmedi (yalnız storage).

**Maliyet (ölçüldü; `crates/storage/tests/scope_guard_cost.rs`, `#[ignore]`,
debug, bellek içi, 20.000 satır, **tek sıra** `--test-threads=1`, iki koşu).**
Referans kapıları (iddia → çapa): tetikleyicili **80,8 / 90,6 µs**, tetikleyicisiz
**70,2 / 71,5 µs** → **+10,6 / +19,1 µs/satır (%15 / %27)**; `scope_violations()`
40.000 iddiada **24,4 / 24,5 ms** (yalnız güncellemede). Aynı koşularda v31
segment kontrolü **+4,2 / +5,6 µs** verdi (önceki turla uyumlu). **Dürüst ölçüm
notu:** bu iki ölçüm testi **paralel** koştuğunda aynı sayılar +51,8 µs ve
+230,6 µs çıktı — çekişme ölçümü kirletiyor; bu yüzden yalnız tek sıra koşular
kanıt sayıldı (ilk paralel koşu raporlanmadı). İddia/kanıt yazımı kullanıcı
eylemi başına bir satırdır; toplu içe aktarma yolunda bu bedel ölçülmelidir.

**Kapı kanıtı (gerçek koşular; betik `~/.hermes/cache/scratch/dim-ref-scope-gates.py`,
her mutasyon tek başına, `try/finally` + `os.utime` + sha256 birebir —
`0032…sql 069eea3e…`, `scope_guard.rs d68f853c…`):** **7/7 KIRMIZI, taban 2/2
yeşilken** — M1 claims kapısı düşer · M2 evidence_links · M3 event_sources ·
M4 claim_relations · M5 tasks · M6 tarama sayımının dosya koşulu bozulur ·
M7 şemadan türetim kuralı referans sütununu görmez (kapı testi kırmızı).
Geri yazım sha256 birebir; taban tekrar 17/17.

**CI (ölçüldü, ilk koşu).** Kod `3be75ba` (5 dosya, **+542/−20**) → run
**`43634459038` 10/10 YEŞİL**: `dim-storage` **626** iki OS'ta birebir
(`ubuntu-latest` **301,82 s** / `macos-15` **347,64 s**; yedi yeni test logda tek
tek `ok`), `dim-cli-adapters` **168** iki OS'ta, `arayüz` 66 dosya / **1428**
test, paket `index-BPk1_PbH.js` 692,77 kB / gzip 184,88 kB (arayüz değişmedi),
`masaüstü kabuk`, `windows güvenli depo`, `belge worker'ı`, `sözleşmeler`,
`SBOM`, `kaynak arşivi` ve `sürüm paketi manifesti` yeşil.

**Dürüst sınırlar.** (a) Tetikleyiciler **yeni** yazmayı engeller; v32 öncesi
çapraz referanslar **sayılır, düzeltilmez** — ve o satırlar için sessiz kaybolma
riski **sürer** (temizlik ayrı veri işi). (b) **JSON dizi** sütunları
(`notes.source_anchor_ids`, `note_revisions.source_anchor_ids`,
`review_items.source_anchor_ids`, `note_drafts.source_anchor_ids`) bu kapının
dışında: çekirdek `validate_note_anchors` doğrular; şema düzeyinde dizi elemanı
denetlenmez (JSON1 sorgusu gerekir — ayrı iş). (c) `tasks.source_anchor_id`
bugün **hiç yazılmıyor** (sütun ileriye dönük); tetikleyicisi o yüzden bugünkü
yollar için ileriye dönük. (d) Kapı yalnız **dosya sınırı**; kanıtın hâlâ
geçerli bir çapaya işaret ettiği (silinmemiş segment, `anchor_status`) çekirdek
kapısıdır. (e) SQLite dosyasını doğrudan düzenleyen (anahtarı olan) bir saldırgan
tetikleyiciyi de düşürebilir; bu kapı uygulama hatasına karşı. (f) Tauri
komutları komut olarak koşulmadı; davranış gerçek SQLite ile storage düzeyinde
ölçüldü; arayüz değişmedi.

## F08-T04f — JSON dizi kanıt referansları dosya sınırını aşamaz (şema v33, 2026-11-15)

**Neden.** F08-T04e tek sütunlu kanıt referanslarını şemada kapıladı (v32) ve
dürüst sınırı (b) olarak şunu bıraktı: kanıt listesi taşıyan dört tablo
(`notes`, `note_revisions`, `review_items`, `note_drafts`) `source_anchor_ids`
**JSON dizisi** kullanıyor ve bu sütunlar kapının dışında — çekirdek
`validate_note_anchors` doğruluyor, ama doğrudan SQL yazan bir yol (yeni komut,
betik, ileride toplu içe aktarma) atlıyordu. `note_revisions`de `case_id` yok
(dosya üst nottan okunur), `note_drafts` ise bilinçli olarak doğrulamıyor
(yarıda kalan otomatik kayıt kullanıcının metnini kaybettirmemeli).

**Ölçülen kusur (düzeltme öncesi davranış; v33 tetikleyicileri düşürülerek
gerçek SQLite'ta ölçüldü, geçici sonda sonra silindi).** A dosyasının notuna B
dosyasının çapası **yazıldı** (`Ok(1)`). Sonra B dosyasının belgesi silindi:
çapa `ON DELETE CASCADE` ile gitti (1 → 0), notta `["a9"]` **kaldı**, denetime
**tek satır** düşmedi. v32 taramasında bu dört tablonun alanı yoktu — yani
kusur ne engelleniyor ne sayılıyordu. Not, kimlikle okuyan yollarda (çapa
çözümleme) A dosyasında B dosyasının belgesini gösterebilir; B silinince
kaynak sessizce boşalır (INV-05 / P0 sınıfı).

**Yazılan (şema v33, `migrations/0033_json_reference_scope_guards.sql`).** Kural
şemada: dizinin **her elemanı** kendi dosyasındaki `source_anchors` kaydını
göstermeli; gösteremiyorsa satır yazılmaz (fail-closed). **4 tablo × BEFORE
INSERT/UPDATE = 8 tetikleyici** (`json_each`), reddetme sabit önekle
`RAISE(ABORT,'DIM_SCOPE:<tablo>')` → `StorageError::ScopeViolation` →
**`E_DB_SCOPE`** (v31/v32 sözleşmesi):

- `notes` / `review_items` → eleman `NEW.case_id` dosyasında olmalı
- `note_revisions` → eleman **üst notun** dosyasında olmalı (satırda `case_id`
  yok; üst not yoksa yazma reddedilir)
- `note_drafts` → **daha gevşek**: var olmayan/doğrulanmamış kimlik kalabilir
  (taslak metni kurtarır), yalnız **başka dosyada var olan** çapa reddedilir
- Bozuk JSON ya da dizi olmayan değer **reddedilir**: doğrulanamayan referans
  yazılmaz. `json_each`e her zaman geçerli metin verilir
  (`CASE WHEN json_valid(...) THEN ... ELSE '[]' END`) ki hata, SQLite'ın
  "malformed JSON" hatası yerine kapının kendi kodu olsun.

Kapı yalnız **dosya sınırını** denetler; `anchor_status`, `deleted_at`, kaynak
sayısı sınırı (`MAX_NOTE_SOURCES`) gibi ürün politikaları çekirdek sorgusunda
kalır — tetikleyici politika öğrenirse eski veri bir anda "yazılamaz" hâle
gelirdi.

**Mevcut veri.** `Database::scope_violations()` artık **17 tabloyu** sayar
(8 zincir + 5 referans + 4 JSON dizi); bozuk JSON "doğrulanamıyor" diye
aykırı **sayılır**. Güncelleme anında toplam sıfırdan büyükse sayı denetime
(`vault_upgrade` / `scope_violations`) dört yeni alanla yazılır: `notes`,
`noteRevisions`, `reviewItems`, `noteDrafts`.

**Testler.** `scope_guard::tests` **17 → 22** (+5: not / revizyon / inceleme
öğesi / taslak için çapraz INSERT + UPDATE, taslakta "eksik kimlik meşru,
yabancı kimlik değil", bozuk JSON reddi, eski satırı sayan tarama) → yeni
`the_scan_counts_json_array_rows_written_before_the_guards_existed`;
`upgrade_tests::upgrade_reports_cross_case_json_anchor_lists_left_by_the_previous_schema`
(v32 kasa → güncelleme → denetimde dört ayrı sayı; yeni çapraz dizi reddedilir)
→ `dim-storage --lib` **626 → 632** (yerel 430,19 s, 0 düştü). Şemadan türetilen
"her kapsam tablosunun kapısı var mı" testi artık `*_anchor_ids` sütunlarını da
kapsar ve JSON tablolarında gövdede `json_each(` arar.

**Maliyet (ölçüldü; `scope_guard_cost.rs`, `#[ignore]`, debug, bellek içi,
20.000 meşru not × 2 çapa, **tek sıra** `--test-threads=1`, iki koşu).**
Tetikleyicili **62,9 / 55,1 µs/satır**, tetikleyicisiz **37,4 / 34,3 µs/satır**
→ fark **+25,5 / +20,8 µs/satır (%68 / %61)**. `scope_violations()` 40.000
notta **79,2 / 71,7 ms**. Not yazımı kullanıcı eylemi başına **tek satırdır**;
mutlak bedel bu yüzden küçük, ama oran yüksek — toplu içe aktarma yolunda bu
bedel ayrıca ölçülmelidir (bugün böyle bir yol not yazmıyor).

**Kapı kanıtı (gerçek koşular; betik `~/.hermes/cache/scratch/dim-json-scope-gates.py`,
her mutasyon tek başına, `try/finally` + `os.utime` + sha256 birebir —
`0033…sql 257e5a97…`, `scope_guard.rs d107cd70…`):** **9/9 KIRMIZI, taban iki
uçta yeşilken** — M1 notes BI kapısı düşer (3 test) · M2 notes BU çapayı
dosyasıyla karşılaştırmaz · M3 revizyon üst notun dosyasını görmez · M4
review_items · M5 taslak INSERT · M6 bozuk JSON kapıdan geçer · M7 tarama not
dizilerini saymaz · M8 şemadan türetim JSON sütununu görmez · M9 taslak UPDATE.
Geri yazım sha256 birebir; taban tekrar 22/22.
**Dürüst (kendi hatam, iki ölçüm hatası — ilk koşu kanıt sayılmadı):** ilk
koşuda **8/9** çıktı. (1) M5 yeşil kaldı: taslak testi yalnız **UPDATE** yolunu
deniyordu (ilk kayıttan sonra hepsi `ON CONFLICT`), INSERT kapısı ölçülmüyordu →
teste yeni taslak (INSERT) yolu eklendi ve M9 (UPDATE) ayrıldı. (2) M6 yeşil
kaldı: `CASE … ELSE '[]'` mutasyonu **davranışı değiştirmiyordu**, çünkü bozuk
JSON'u zaten birinci disjunct (`NOT json_valid`) yakalıyor — bu yüzden M6, o
disjunctı kaldıracak şekilde yeniden yazıldı (o zaman test kırmızı). Yalnız
düzeltilmiş betiğin 9/9 koşusu kanıt sayıldı.

**CI (ölçüldü, ilk koşu).** Kod `3ace064` + ölçüm testi `0bec743` → run
**`43644744375` 10/10 YEŞİL**: `dim-storage` **632** iki OS'ta birebir
(`ubuntu-latest` **311,95 s** / `macos-15` **299,09 s**; altı yeni test logda tek
tek `ok`), `dim-cli-adapters` **168**, `arayüz` 66 dosya / **1428** test, paket
`index-BPk1_PbH.js` 692,77 kB / gzip 184,88 kB (arayüz değişmedi), `masaüstü
kabuk`, `windows güvenli depo`, `belge worker'ı`, `sözleşmeler`, `SBOM`,
`kaynak arşivi`, `sürüm paketi manifesti` yeşil. (İlk push'un koşusu
`43644449302`, ikinci push gelince concurrency ile **cancelled**; geçerli kanıt
HEAD'i kapsayan `43644744375`.)

**Dürüst sınırlar.** (a) Tetikleyiciler **yeni** yazmayı engeller; v33 öncesi
çapraz diziler **sayılır, düzeltilmez** — o satırlarda sessiz kaybolma riski
sürer (temizlik ayrı veri işi). (b) `note_drafts` bilinçli olarak **gevşek**:
var olmayan kimlik taslakta kalabilir; bir çapa taslakta yazıldıktan **sonra**
başka dosyada oluşturulursa taslak o kimliği taşır (okuma yolu çapa
çözümlemez; nota dönüşürken `validate_note_anchors` yine reddeder). (c)
`notes.undo_stack` / `redo_stack` birer **anlık görüntü**dür (revizyon
numaraları), referans değildir; `payload` içindeki serbest JSON yorumlanmaz.
(d) Kapı yalnız dosya sınırı; kaynak sayısı/durum politikası çekirdekte.
(e) SQLite dosyasını doğrudan düzenleyen (anahtarı olan) saldırgan tetikleyiciyi
de düşürebilir; bu kapı uygulama hatasına karşı. (f) Maliyet debug derlemede,
tek makinede (Apple M2), iki koşu; üretim derlemesinde oran ölçülmedi. (g)
Tauri komutları komut olarak koşulmadı; davranış gerçek SQLite ile storage
düzeyinde ölçüldü, arayüz değişmedi — görsel QA + gerçek pencere **insan
bekliyor**.

## F08-T04g — kalan referanslar dosya sınırında: soy ağacı, maskeleme çıktısı,
## gönderim ve giden segment sicili (şema v34, 2026-11-15)

**Neden.** v32 tek sütunlu kanıt referanslarını, v33 JSON dizi kanıt listelerini
kapıladı. Şemadan türetilen kural (test) hâlâ **yedi** referans sütununu
görmüyordu — hiçbiri `case_id` + bilinen kapsam sütunu kalıbına girmiyor ya da
tablo kendi `case_id`'sini taşımıyordu:

| Sütun | Dosya nereden okunur | Bugünkü durum |
|---|---|---|
| `document_versions.parent_version_id` | satırın `case_id`'si | çekirdek doğruluyor, **şema kapısı yok** |
| `redaction_plans.output_version_id` | satırın `case_id`'si | çekirdek doğruluyor, **şema kapısı yok** |
| `ai_context_items.document_version_id` | üst kayıt `ai_runs.case_id` | **çekirdek kapısı da yoktu** |
| `ai_context_items.source_anchor_id` | üst kayıt `ai_runs.case_id` | **çekirdek kapısı da yoktu** |
| `ai_context_items.segment_ids` (JSON) | satırın sürümü | çekirdek doğruluyor, şema kapısı yok |
| `transfer_segments.document_version_id` | üst kayıt `ai_runs.case_id` | **çekirdek kapısı da yoktu** |
| `source_anchors.page_id`, `source_marks.page_id` | sayfanın koşusu | çekirdek doğruluyor, şema kapısı yok |
| `review_items.ai_run_id`, `note_drafts.note_id` | çalıştırma / not | çekirdek doğruluyor, şema kapısı yok |

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite, geçici sonda sonra silindi;
v33 şeması).** Üç sınıf da kapıyı atlıyordu:

1. `record_context_items` **çekirdek kapısı yoktu**: A dosyasının çalıştırmasına
   B dosyasının sürümü + çapası yazıldı (`Ok(1)`) ve `list_context_items`
   A dosyasının ekranında **B'nin belge adını** gösterdi
   (`GIZLI diger dosya belgesi`). Gönderim sicili, cihazdan çıkan içeriğin
   kaydıdır; yanlış dosyayı göstermesi P0 sınıfıdır.
2. `redaction_plans.output_version_id`: doğrudan SQL ile A dosyasının planına
   B dosyasının sürümü çıktı olarak yazıldı; `list_redaction_plans` A dosyasının
   sicilinde **B'nin belge adını** gösterdi.
3. `document_versions.parent_version_id`: A dosyasının sürümü B dosyasının
   sürümünü ebeveyn gösterdi (maskeleme izi yanlış dosyayı gösterir).

**Yazılan (şema v34, `0034_remaining_reference_scope_guards.sql`).** Referansın
hedefi kendi dosyasında olmalı; yoksa satır yazılmaz (fail-closed).
**7 tablo × BEFORE INSERT/UPDATE = 14 tetikleyici**:

* `document_versions` → `parent_version_id` (ebeveyn aynı dosyada; aynı dosyada
  **başka belgenin** sürümü meşru — maskeli kopya yolu korunur).
* `redaction_plans` → `output_version_id` (çıktı sürümü aynı dosyada).
* `ai_context_items` → sürüm + çapa (`ai_runs.case_id` ile) + `segment_ids`
  (`json_each`: her eleman satırın **kendi** sürümünün segmenti; bozuk JSON
  reddedilir). Tablonun `case_id`'si yoktur; dosya üst kayıttan okunur.
* `transfer_segments` → `document_version_id` (giden segment sicili; `NULL`
  eski satır kapıya takılmaz).
* `source_anchors` → `page_id` **kendi koşusunun** sayfası olmalı (0031'in
  `text_segments.page_id` kuralının ikizi).
* `source_marks` → `page_id` kendi **dosyasında** olmalı (sayfa-koşu üyeliği
  geometri politikası olarak çekirdekte kalır).
* `review_items` → `ai_run_id` (aday kökeni), `note_drafts` → `note_id`
  (taslağın bağlandığı not). İkisi de **gevşek**: var olmayan kimlik kalır
  (eski satır/denetim izi), yalnız **başka dosyada var olan** kimlik reddedilir.

**Çekirdek.** `record_context_items` çalıştırmanın dosyasını okur; başka
dosyada **var olan** sürüm/çapa `E_NOT_FOUND` ile reddedilir (gönderimden sonra
silinmiş belge meşru kalır — denetim satırı yaşamalıdır). `record_outbound_segments`
segment aramasını `JOIN document_versions … AND dv.case_id=run.case_id` ile
yapar: yabancı sürümün metni giden sicile yazılamaz. `scope_violations()`
**19 tabloyu** sayar; `documentVersions` (soy ağacı), `redactionPlans` (çıktı),
`sourceAnchors` (sayfa), `sourceMarks` (sayfa), `reviewItems` (köken),
`noteDrafts` (not) sayımlarına yeni koşullar, yeni alan **`aiContextItems`** ve
**`transferSegments`**; güncelleme denetimine iki yeni alan yazılır.

**Testler (güncel kodla).** `scope_guard::tests` **22 → 28** (+6: plan çıktısı
INSERT/UPDATE + meşru yol · soy ağacı + meşru türev + NULL · gönderim sicili
sürüm/çapa/segment/bozuk JSON + var olmayan kimlik · giden segment ·
aday kökeni · taslağın notu · eski satırları sayan tarama) + `ai::tests` **+1**
(`context_items_cannot_point_at_another_cases_version_or_anchor`) +
`upgrade_tests::upgrade_reports_cross_case_output_and_context_links_left_by_the_previous_schema`
(v33 kasa → güncelleme → denetimde üç ayrı sayı + dört yeni çapraz yazma reddi)
→ `dim-storage --lib` **632 → 640** (yerel **445,94 s**, 0 düştü). Şemadan
türetilen test artık **sütun kapsamını** da denetler: kapsamlı bir tabloda
`document_id`/`document_version_id`/`parent_version_id`/`output_version_id`/
`parse_run_id`/`segment_id`/`page_id`/`source_anchor_id`/`anchor_id`/`claim_id`/
`related_claim_id`/`event_id`/`ai_run_id`/`note_id` sütunlarından biri varsa, o
tablonun bir tetikleyicisi sütunu **adıyla** anmalı; yeni bir referans sütunu
kapısız eklenemez. `ai_context_items` ayrıca `json_each(` arar.

**Maliyet (ölçüldü; `tests/scope_guard_cost.rs`, `#[ignore]`, debug, bellek
içi, 20.000 satır, `--test-threads=1`, iki koşu).** Gönderim sicili **+24,91 /
+25,06 µs/satır** (3,51 → 28,42; 710–728%); giden segment **+32,94 / +32,84
µs/satır** (7,78 → 40,62; 422–423%); `scope_violations()` 40.000 bağlam +
40.000 giden segment satırında **77,5 / 78,0 ms**. Bu iki tabloya yazan hacim
düşüktür (çalıştırma başına belge/segment sayısı kadar satır); toplu ithal yolu
yok, olursa ayrıca ölçülmeli.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-v34-gates.py`, her mutasyon
tek başına, `try/finally` + `os.utime` + sha256 birebir —
`0034…sql 1ffcc6d9…`, `scope_guard.rs c8f9206c…`, `ai.rs d2cd8e70…`):**
**13/13 KIRMIZI, taban iki uçta yeşilken** (taban `scope_guard::tests` **28/28**,
`ai::tests::context_items…` 1/1, upgrade testi 1/1; geri yazım birebir, taban
tekrar yeşil) — M1 plan çıktısı INSERT · M2 plan çıktısı UPDATE · M3 soy ağacı ·
M4 gönderim sicili INSERT · M5 gönderim sicili UPDATE · M6 giden segment ·
M7 aday kökeni · M8 taslağın notu · M9 çapanın sayfa üyeliği · M10 kapılı tablo
listesi (şemadan türetim ayrışır) · M11 tarama gönderim sicilini hiç saymaz ·
M12 segment listesi kapıdan çıkar · M13 çekirdek kapsam kapısı düşer.
**Dürüst (kendi hatam, iki ölçüm hatası — ilk koşular kanıt sayılmadı):**
(1) ilk tam koşuda **12/13** çıktı: M11 YEŞİL kaldı, çünkü mutasyon yalnız
sürüm koşulunu çeviriyordu ve aynı satırı **çapa** koşulu yakalıyordu → M11
"tarama bu tabloyu hiç saymaz" olarak yeniden yazıldı, ayrı koşuda KIRMIZI
ölçüldü. (2) iki test fixture'ında sha256 alanı 62 karakterdi (CHECK
`length=64`) → düzeltildi; o koşular kanıt sayılmadı.

**CI (ölçüldü).** Kod `e25f0bc` (6 dosya, **+1063/−21**) → run `43653196298`
**10/10 YEŞİL**: `dim-storage` **640** iki OS'ta birebir (ubuntu **846,13 s** /
macos-15 **314,26 s**), `dim-cli-adapters` **168**, `arayüz` 66 dosya / paket
`index-BPk1_PbH.js` 692,77 kB / gzip 184,88 kB (**arayüz değişmedi**);
`masaüstü kabuk`, `windows güvenli depo`, `belge worker'ı`, `sözleşmeler`,
`SBOM`, `kaynak arşivi`, `sürüm paketi manifesti` yeşil.

**Dürüst sınırlar.** (a) Tetikleyiciler yalnız **yeni** yazmayı engeller; v34
öncesi çapraz bağlar **sayılır, düzeltilmez** (temizlik ayrı veri işi).
(b) `ai_context_items`, `review_items.ai_run_id`, `note_drafts.note_id` bilinçli
**gevşek**: var olmayan kimlik kalır (denetim izi/eski satır); yalnız başka
dosyada **var olan** kimlik reddedilir. (c) Maskeleme çıktısı kapısı yalnız
**dosya sınırı**dır; "çıktı, kaynağın alt sürümü olmalı" politikası çekirdekte
(`create_redaction_plan`) kalır — doğrudan SQL aynı dosyada yanlış sürümü çıktı
gösterebilir. (d) `source_marks.page_id` için kural dosya sınırıdır; sayfanın
**kendi koşusunun** sayfası olması (`source_anchors`ta olduğu gibi) geometri
politikası olarak çekirdekte. (e) `transfer_segments` `document_version_id`'si
`NULL` olan eski satırlar kapıya takılmaz ve "gönderilmiş" sayılmaz.
(f) SQLite dosyasını doğrudan düzenleyen (anahtarı olan) saldırgan tetikleyiciyi
de düşürebilir; bu kapı uygulama hatasına karşı. (g) Maliyet debug derlemede,
tek makinede (Apple M2), iki koşu; üretim derlemesinde oran ölçülmedi.
(h) Tauri komutları komut olarak koşulmadı; davranış gerçek SQLite ile storage
düzeyinde ölçüldü, arayüz değişmedi — görsel QA + gerçek pencere **insan
bekliyor**.

## F08-T04h — belgenin güncel sürümü kendi belgesi olmalı (şema v35, 2026-11-15)

**Neden.** v31 zinciri, v32 tek sütunlu kanıt referanslarını, v33 JSON dizi kanıt
listelerini, v34 kalan referansları kapıladı. `documents.current_version_id` bu
zincirin **çıkış noktasıdır** ve şemada ne FK'si ne tetikleyicisi vardı: her
okuma yolu “güncel sürüm”ü buradan çözer —

* `coverage_report` güncel sürümün ayrıştırma koşusunun sayfa/segmentlerini sayar;
* `check_in_text_quotes` alıntıyı o koşunun sayfalarında arar;
* AI kapsamı gönderilecek gövdeyi o sürümden toplar.

Tek bir doğrudan `UPDATE documents SET current_version_id=…` belgeyi başka bir
belgenin (başka dosyada olabilir) sürümüne çevirebiliyordu.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite, v34; geçici sonda sonra
silindi).** `UPDATE documents SET current_version_id='pv9' WHERE id='pd1'`
**kabul edildi** (`rows=1`); `pd1.current_version_id = Some("pv9")` (B dosyası).
Sonuçlar:

1. `coverage_report('p1c')` → `documents=1 versions=1 parse_runs=1 pages=2
   segments=2`; A dosyasının kendi belgesinde **1** sayfa/segment varken B
   dosyasının iki sayfası A'nın kapsamına girdi.
2. A dosyasının belgesinde «gizli metin» alıntısı: **önce** `exact=0
   normalized=0 not_found=1 pages_scanned=1`; **sonra** `exact=1
   pages_scanned=2` — alıntı yalnız B dosyasının metninde geçtiği hâlde A
   dosyasının belgesinde “bulundu” hükmü aldı (INV-05: delil ekrandaki belgeye
   işaret etmez).

**Yazılan (şema v35, `0035_document_current_version_guard.sql`).**
`current_version_id` NULL olabilir; doluysa **var olan** ve **aynı belgeye** ait
bir sürümü göstermeli. **3 tetikleyici**:

* `documents` → `scope_guard_documents_bi` (INSERT) ve `_bu` (UPDATE): yabancı /
  olmayan sürüm işaretçisi yazılmaz (fail-closed).
* **Ters yön** → `scope_guard_document_versions_current_ref_bu`: bir belgenin
  güncel sürümünü gösteren sürüm satırı başka belgeye taşınamaz (kapı sürüm
  satırı taşınarak atlatılamaz; taşımak isteyen önce işaretçiyi NULL'a çeker).

Bilinçli ayrım — **eski/boşta kalmış işaretçi** (sürümü silinmiş) başka
alanların güncellenmesini **engellemez**: UPDATE kapısı yalnız işaretçiyi
**değiştiren** yazmada koşar (`NEW.current_version_id IS NOT OLD...`). Aksi
hâlde eski bir kasada bozuk tek satır belgeyi yeniden adlandırılamaz hâle
getirirdi. Boşta kalan işaretçi çapraz okuma da üretmez (sürüm yok → koşu yok);
bu yüzden tarama sayımına da girmez. Reddetme `RAISE(ABORT,'DIM_SCOPE:documents')`
→ `StorageError::ScopeViolation` → **`E_DB_SCOPE`**.

**Çekirdek.** Değişiklik gerekmedi: `add_version_with_parent` işaretçiyi zaten
yalnız kendi belgesinin yeni sürümüne yazıyor. Kapı **doğrudan SQL** yazan yol
için; meşru yazma yolu testle doğrulandı (`adding_a_version_still_points_the_document_at_its_own_version`).
`scope_violations()` artık **20 tabloyu** sayar; yeni alan `documents` (güncel
sürümü başka belgeyi gösteren belge sayısı) ve güncelleme denetimine
`"documents"` alanı yazılır.

**Testler (güncel kodla).** `scope_guard::tests` **28 → 33** (+5: yabancı/aynı
dosyada başka belge/olmayan sürüm INSERT + UPDATE, ters yön taşıma, boşta
kalan işaretçinin güncellemeyi engellememesi, tarama sayımı, meşru
`add_version` yolu) + `upgrade_tests::upgrade_reports_a_document_pointing_at_another_documents_version`
(v34 kasa → güncelleme → denetimde **2** sayı + yeni çapraz yazma reddi) →
`dim-storage --lib` **640 → 646** (yerel **407,95 s**, 0 düştü). Şemadan türetilen
test artık `current_version_id` sütununu da görür: `documents` kapılı tablo
kümesine girer (`GUARDED_TABLES` 19 → 20); yeni bir “güncel sürüm” işaretçisi
sessizce kapısız eklenemez.

**Maliyet (ölçüldü; `tests/scope_guard_cost.rs`, `#[ignore]`, debug, bellek içi,
20.000 yazma, iki koşu).** Güncel sürüm işaretçisi yazımı tetikleyicili **11,15 /
11,15 µs/satır**, tetikleyicisiz **1,29 / 1,30** → **+9,86 / +9,85 µs/satır
(%764 / %758)**; `scope_violations()` 20.000 belgede **2,0 / 2,0 ms**. Yol
**seyrek**: işaretçi sürüm eklenirken kullanıcı eylemi başına bir kez yazılır;
tarama yalnız kasa güncellemesinde koşar.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-v35-gates.py`, her mutasyon
tek başına, `try/finally` + `os.utime` + sha256 birebir —
`0035…sql aba1d9f5…`, `scope_guard.rs d7cdbfec…`, `lib.rs 02bfa0e4…`):**
**7/7 KIRMIZI, taban iki uçta yeşilken** (taban `scope_guard::tests` **33/33**,
upgrade testi 1/1; geri yazım birebir, taban tekrar yeşil) — M1 belge INSERT
kapısı · M2 belge UPDATE kapısı · M3 ters yön taşıma kapısı · M4 tarama
işaretçiyi hiç saymaz · M5 güncelleme denetimi sayıyı yazmaz · M6 şemadan
türetim sütunu görmez · M7 boşta kalan işaretçi belgeyi kilitler.

**CI (ölçüldü).** Kod `25c5210` (5 dosya, **+452/−3**) → run `43660270080`
**10/10 YEŞİL**: `dim-storage` **646** iki OS'ta birebir (macos-15 **347,47 s** /
ubuntu **653,43 s**), `dim-cli-adapters` **168**, `arayüz` 66 dosya / **1428**
test, paket `index-BPk1_PbH.js` 692,77 kB / gzip 184,88 kB (**arayüz
değişmedi**); `masaüstü kabuk`, `windows güvenli depo`, `belge worker'ı`,
`sözleşmeler`, `SBOM`, `kaynak arşivi`, `sürüm paketi manifesti` yeşil.

**Dürüst sınırlar.** (a) Tetikleyiciler yalnız **yeni** yazmayı engeller; v35
öncesi çapraz işaretçiler **sayılır, düzeltilmez** — o satırlar için yanlış
okuma riski, veri temizliğine kadar sürer. (b) Okuma yolları işaretçiye
güvenmeye devam eder (sorgulara `dv.document_id=d.id` koşulu eklenmedi); kapı
yeni bozulmayı durdurur, eski bozulmayı gizlemez — tarama sayısı güncellemede
denetime düşer. (c) **Boşta kalan** işaretçi (sürüm silinmiş) bilinçli olarak
güncellemeyi engellemez ve çapraz sayılmaz; bugün üretimde sürüm silme yolu
yok, yalnız testlerde var. (d) Aynı belgenin **eski** sürümünü göstermek
meşrudur (geri alma); kapı yalnız *başka belge* işaretçisini reddeder. (e)
SQLite dosyasını doğrudan düzenleyen (anahtarı olan) saldırgan tetikleyiciyi de
düşürebilir; bu kapı uygulama hatasına karşı. (f) Maliyet debug derlemede, tek
makinede (Apple M2), iki koşu; üretim derlemesinde oran ölçülmedi. (g) Tauri
komutları komut olarak koşulmadı; davranış gerçek SQLite ile storage düzeyinde
ölçüldü, arayüz değişmedi — görsel QA + gerçek pencere **insan bekliyor**.

## F08-T04i — güncel sürüm **okunurken** kendi belgesine bağlı kalır (şema v36, 2026-11-15)

**Neden.** T04h (`0035_document_current_version_guard.sql`) yalnız **yeni**
yazmayı kapıladı ve dürüst sınırını şöyle yazdı: “v35 öncesi çapraz işaretçiler
sayılır, düzeltilmez — veri temizliğine kadar yanlış okuma riski sürer; okuma
sorgularına `dv.document_id=d.id` eklenmedi.” Bu tur o sınırı kapatır: **eski**
çapraz satır artık yanlış veri üretmez, “veri yok” üretir (fail-closed).

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite, v35; tetikleyiciler bilinçli
düşürülüp eski satır taklit edildi, geçici sonda sonra silindi).**
`UPDATE documents SET current_version_id='v9' WHERE id='d1'` (v9 = **başka
dosyanın** sürümü) sonrası okuma yolları:

1. `coverage_report('c1')` → `pages=1 segments=1` (A'nın kendi sürümü hiç
   sayılmadı; B'nin sayfası/segmenti A'nın kapsamına girdi).
2. `check_in_text_quotes` — «gizli tutanak» alıntısı **yalnız B'nin metninde**
   geçtiği hâlde A'nın belgesinde **`exact`** döndü (INV-05).
3. `consent_preview_masked('c1', ['d1'])` → `documents=1 segments=1 sample="gizli
   tutanak metni"` — onaylanacak önizleme **yabancı metni** taşıyordu.
4. `documents_pending_parse` belgeyi **yabancı** sürüm kimliğiyle listeliyor;
   `get_document`/`list_documents` yabancı sürümü “güncel” diye döndürüyor;
   `locate_anchor` / plan okuması yabancı sürüm numarasını yazıyordu.

**Yazılan (şema v36, `0036_current_version_read_view.sql`).** Tek tanım:
`current_document_versions` görünümü (`document_id, case_id, version_id,
version_no, byte_size`) — `documents` ile `document_versions` yalnız
`v.id = d.current_version_id **AND v.document_id = d.id**` iken birleşir.
Bütün okuma yolları bu görünümden geçer (kopya `JOIN`/alt sorgu yok):

* `documents::get_document` / `list_documents` — künyedeki `current_version_id`;
* `documents::documents_pending_parse` — kuyruk (görünümle `INNER JOIN`);
* `ai::coverage_report` (`CURRENT_RUNS`) — “gönderilebilir korpus”;
* `ai::resolve_transfer_scope` + `ai::create_transfer_consent` plan kapsamı —
  gönderim/onay kapsamı;
* `intext_quotes::resolve_quote_scopes` + `resolve_historical_scopes` — alıntı
  taraması ve **güncel sürüm dışlama** kuralı;
* `anchors::locate_anchor` / `rebind_suggestion` — çapa okumaları;
* `redactions::get_redaction_plan` / `list_redaction_plans` (güncel sürüm
  numarası + `version_superseded` nedeni) ve `plan_mask_terms_for_send` —
  plan okuması ve gönderim maskesi kapısı.

**Davranış.** İşaretçi çaprazsa (ya da sürümü silinmişse) belgenin güncel
sürümü **yoktur**: numara uydurulmaz, başka sürüme düşülmez. Bozukluk
`Database::scope_violations()` tarafından **sayılmaya devam eder**; görünüm
onu gizlemez, yalnız okumaya sokmaz. `deleted_at` süzgeci görünümde değil,
çağıranda kalır (mevcut silme politikası değişmedi).

**Testler (güncel kodla).** `scope_guard::tests` **33 → 40** (+7):
`current_version_view_requires_the_same_document` (şemadan türetim: görünüm
kendi belgesini denetler) · `stale_cross_pointer_is_not_read_as_a_current_version`
(künye + kapsam raporu) · `stale_cross_pointer_is_not_queued_for_parse` ·
`stale_cross_pointer_does_not_verify_a_quote_in_foreign_text` (alıntı
`not_found`, konum yok; yalnız belgenin **kendi** eski sürümü taranır) ·
`stale_cross_pointer_is_not_sent_in_the_transfer_scope` (`no_current_version`,
boş örnek) · `stale_cross_pointer_is_not_the_current_version_in_anchor_reads`
· `stale_cross_pointer_is_not_the_current_version_in_redaction_reads`
(`current_version_no = None`, neden `version_superseded` korunur) →
`dim-storage --lib` **646 → 653** (yerel **396,75 s**, 0 düştü). fmt (iki
manifest) / `clippy -p dim-storage --all-targets -D warnings` / masaüstü
`cargo check --all-targets` temiz.

**Maliyet (ölçüldü; `tests/scope_guard_cost.rs`, `#[ignore]`, debug, bellek
içi, 20.000 belge, iki koşu).** Aynı sorgunun görünümlü ve ham işaretçili hâli
yan yana: kapsam sorgusu **99,4 / 98,4 ms** (ham **54,5 / 53,7 ms**) → **+44,9
/ +44,7 ms** (+%82 / +%83, belge başına **+2,2 µs**); kuyruk sorgusu **48,3 /
48,1 ms** (ham **33,5 / 33,2 ms**) → **+14,8 / +14,9 ms** (+%44 / +%45, belge
başına **+0,7 µs**). Sorgu planı (`EXPLAIN QUERY PLAN`, gerçek şema): görünüm
belge başına **iki PK araması** ekler (belge + sürüm); `parse_runs` araması
`idx_parse_runs_version` üzerinden `SEARCH`. Okuma yolu kullanıcı eylemi
başına bir kez koşar; 1.000 belgeli B-01 korpusunda bedel ≈ **+2,2 ms** /
**+0,7 ms**.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-current-version-read-gates.py`,
her mutasyon tek başına, `try/finally` + `os.utime` + sha256 birebir —
`0036…sql fcb789e2…`, `lib.rs 1775727b…`, `documents.rs 345013e4…`,
`intext_quotes.rs 7af00a76…`, `ai.rs 05a7f4e8…`, `anchors.rs 260a12ca…`,
`redactions.rs 61fc5419…`, `scope_guard.rs 6a8fbff7…`,
`scope_guard_cost.rs 0f1e9bb3…`):** **8/8 KIRMIZI, taban iki uçta 7/7
yeşilken** (koşu **son hâl** üzerinde, fmt sonrası, yeniden ölçüldü) — M1
görünüm kendi belgesini denetlemez · M2 belge künyesi ham işaretçiyi okur ·
M3 kuyruk ham işaretçiyle birleşir · M4 alıntı taraması ham işaretçiyi çözer ·
M5 kapsam raporu ham işaretçiyi kullanır · M6 gönderim kapsamı ham işaretçiyi
kullanır · M7 çapa okuması ham işaretçiye döner · M8 plan okuması ham
işaretçiye döner.

**CI (ölçüldü).** Kod `a3a5402` (9 dosya, **+408/−28**) → run `43714629712`
**10/10 YEŞİL**: `dim-storage` **653** iki OS'ta birebir (macos-15 **338,47 s** /
ubuntu-latest **856,48 s**), `dim-cli-adapters` **168**, `arayüz` 66 dosya /
**1428** test, paket `index-BPk1_PbH.js` 692,77 kB / gzip 184,88 kB (**arayüz
değişmedi**); `masaüstü kabuk` 51/0/5 + gerçek worker 5/0/0, `windows güvenli
depo` 28, `belge worker'ı`, `sözleşmeler`, `SBOM`, `kaynak arşivi`, `sürüm
paketi manifesti` yeşil.

**Dürüst sınırlar.** (a) Görünüm **okuma**yı düzeltir; v35 öncesi çapraz
satırlar kasada **durur**, silinmez (veri temizliği ayrı ürün kararı) — ama
artık yanlış okuma üretmezler, yalnız belgeyi “sürümsüz” gösterirler ve
`scope_violations()` sayar. (b) Sürümsüz görünen belge **kuyrukta listelenmez**:
çapraz işaretçili belge otomatik ayrıştırmaya alınmaz; nedeni tarama
sayısındadır, kullanıcı belgeyi yeniden içe alana kadar sessiz kalır. (c)
`ai::create_transfer_consent` plan kapsamı denetimi de görünümden okur; burada
davranış **değişmez** (çapraz işaretçide plan zaten reddediliyordu) — tek
tanım için yapıldı, ayrı mutasyonla ölçülemez. (d) `redaction_plans`
`version_superseded` nedeni korunur (plan gerçekten güncel değildir); yalnız
uydurma sürüm numarası kaldırıldı. (e) Görünüm **her** okuma yolunun onu
kullanmasını zorlamaz: yeni bir sorgu ham sütunu okursa kapı dışında kalır —
yeni `current_version` işaretçileri için `scope_guard` şemadan türetim testi
vardır, okuma yolu için statik bir kapı yok. (f) Maliyet debug derlemede, tek
makinede (Apple M2); üretim derlemesinde ölçülmedi. (g) Tauri komutları komut
olarak koşulmadı, arayüz değişmedi — gerçek pencere + görsel QA **insan
bekliyor**.

## F08-T04j — sürüm izi sicilleri kendi dosyasına bağlanır (şema v37, 2026-11-15)

**Neden.** 0031–0036 zinciri kanıt referanslarını, JSON dizilerini, soy ağacını,
maskeleme çıktısını, gönderim sicilini ve “güncel sürüm” işaretçisini kapıladı.
Şema taraması **iki sicili hiç görmüyordu**: ikisi de `case_id` taşımıyor,
kapsamı üst kaydından miras alıyor ve gösterdikleri sürüm kimliği serbest metin:

* `citation_checks.input_document_version_id` → sürümün **belgesinin** dosyası;
* `export_items.source_version_id` → sahibi üst satırın (`exports`) dosyası.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v36; geçici sonda sonra
silindi).**

1. `record_citation_check` **hiçbir kapı taşımıyordu**: A dosyası adına B
   dosyasının sürümüne atıf kontrolü yazıldı (`Ok(<id>`) ve
   `list_citation_checks('v9')` **B dosyasının ekranında** A'nın belgesinden
   çıkarılmış metni gösterdi («gizli tutanak metni») — INV-05. Var olmayan
   sürüm kimliği de kabul ediliyordu; tabloda `case_id` sütunu yoktu.
2. `record_export` **çekirdek** kapısı doğru çalışıyordu (`E_EXPORT_SOURCE`),
   ama şemada kapı yoktu: doğrudan SQL ile A'nın dışa aktarmasına B'nin sürümü
   yazıldı ve `list_export_items` A'nın kayıt defterinde **B'nin belge adını**
   («Diger dosya belgesi») gösterdi.
3. Her iki durumda `scope_violations()` **0** dedi (kör).

**Yazılan (şema v37, `0037_version_trace_scope_guards.sql`).**

* `citation_checks.case_id` sütunu — satır **hangi dosyada** yazıldı. Göç: eski
  satırlar başvurdukları sürümün dosyasına atfedilir (başka bilgi hiç yoktu);
  sürümü silinmiş satırda `NULL` kalır — **uydurulmaz**, tarama “aidiyeti
  yazılmamış” diye sayar, liste göstermez.
* **4 tetikleyici** (2 tablo × BEFORE INSERT/UPDATE): satırın gösterdiği sürüm
  **başka dosyada var ise** yazma reddedilir; `citation_checks` kendi
  `case_id`'sini taşır (zorunlu), `export_items` dosyayı `exports` satırından
  okur (0034'ün `ai_context_items` kuralıyla aynı: kapsam üst kayıttan miras).
* `Database::scope_violations()` **20 → 22 tabloyu** sayar; kasa güncellemesi
  denetimine `citationChecks` / `exportItems` alanları eklendi (kimlik/içerik
  taşımaz).
* **Bilinçli gevşeklik** (0033 `note_drafts` / 0034 `ai_context_items` ile
  aynı): **var olmayan** sürüm kimliği sicilde kalabilir — bu iki sütun yabancı
  anahtar değildir (denetim izi; belge/sürüm sonradan silinebilir). Kapı yalnız
  **başka dosyada var olan** sürümü reddeder; tarama da yalnız onu sayar.
* Ret `RAISE(ABORT,'DIM_SCOPE:<tablo>')` → `StorageError::ScopeViolation` →
  **`E_DB_SCOPE`**.

**Yazılan (çekirdek).** `record_citation_check` kapsam kapısı (`E_NOT_FOUND`,
satır **hiç** yazılmaz) · `CitationCheckRow`/`NewCitationCheck` `case_id` alanı ·
`list_citation_checks(case_id, version_id)` **iki anahtarlı** (B dosyasının sürüm
kimliği A'nın listesine karışamaz; çapraz yazılmış eski satır **kendi** dosyasının
listesinde kalır) · `exports.rs` kaynak sorgusu tek tanıma indi (`documents`
üzerinden; şema tetikleyicisiyle aynı kaynak) · `get_citation_check` sütun
sırası.

**Yazılan (arayüz).** Tauri `citation_check_list(case_id, version_id)`;
`api.ts` `citationCheckList(caseId, versionId)` + `citationCheckCreate.caseId`;
`Precedents.tsx` yazmada dosya kimliğini gönderir, listeyi iki anahtarla okur.
Ek olarak kontrol listesinin **okuma durumu** (`idle/loading/ready/error`):
düşen okuma **“0 kontrol” demez**, koduyla yazar ve yeniden denenebilir; sürüm
değişiminde önceki sürümün satırları **anında** kalkar; geç yanıt sıra
numarasıyla ezilmez (`checksSeq`); anlam panelinin bağlam hatası ayrı yazılır
(`meaningError`). 8 sözlük anahtarı.

**Testler (güncel kodla).** `scope_guard::tests` **40 → 43** (+3:
`a_citation_check_cannot_reference_another_cases_version`,
`an_export_item_cannot_reference_another_cases_version`,
`scope_violations_reports_cross_case_version_traces`) — ayrıca şemadan türetim
testi iki sicili **kural olarak** kapsar (4. kural: `export_id` +
`source_version_id`; `reference_columns` iki sütun daha) · `precedents::tests`
+1 (`citation_check_cannot_reference_another_cases_version`) · `upgrade_tests`
+1 (`upgrade_reports_version_traces_left_by_the_previous_schema`) →
`dim-storage --lib` **653 → 658** (yerel **501,83 s**, 0 düştü).
`Precedents.test.tsx` **55 → 56** → tam vitest **66 dosya / 1437** test yeşil;
`tsc --noEmit` temiz; fmt (iki manifest) / `clippy -p dim-storage --all-targets
-D warnings` / masaüstü `cargo check --all-targets` temiz.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-v37-gates.py`, her mutasyon
tek başına, `try/finally` + `os.utime` + sha256 birebir — `0037…sql
67e07b37…`, `scope_guard.rs f40dd850…`, `precedents.rs 407ad7de…`,
`Precedents.tsx b7f55871…`; koşu **son hâl** üzerinde, `cargo fmt --check` iki
manifestte temiz olduğu doğrulandıktan sonra):** **11/11 KIRMIZI, taban iki
uçta yeşilken** (rust 4/4, arayüz 4/4, koşu sonunda taban tekrar yeşil) — M1
atıf kontrolü INSERT kapısı düşer · M2 aynı tablonun UPDATE kapısı · M3 dışa
aktarma kalemi INSERT · M4 aynı tablonun UPDATE · M5 çekirdek çapraz sürüm
kapısı · M6 liste iki anahtarı bırakır (yalnız sürüm) · M7 satır kendi dosyasını
taşımak zorunda değil (`case_id IS NULL`) · M8 tarama çapraz satırı görmez ·
M9 şemadan türetim atıf tablosunu görmez · U1 arayüz yazmada dosya kimliğini
göndermez · U2 arayüz listeyi yalnız sürümle okur.

**Dürüst (kendi hatam, iki ölçüm hatası).** Betiğin ilk sürümü **9/11** verdi ve
M9/U1'i “YEŞİL” sandı; ikisi de **kapı boşluğu değil, ölçüm hatasıydı**:

1. M9, `GUARDED_TABLES` dizisinden eleman **siliyordu** → `[&str; 22]` 21
   elemanla **derlenmiyordu**; çıktıda `test result:` olmadığı için betik
   derleme hatasını okuyamadı ve YEŞİL yazdı. Düzeltme: derleme hatası ayrı
   hüküm (**kanıt değil**, kapı başarısız) ve mutasyon **derlenen** anlamsal
   biçime çevrildi (eleman sayısı korunur, tablo listeden düşer).
2. U1'de vitest `-t` süzgeci özeti `Tests  1 failed | 55 skipped` yazar; betik
   yalnız `passed` aradığı için hiçbir sayı okuyamadı → YEŞİL. Düzeltme:
   `failed/passed/skipped` birlikte okunur; hiç test koşmadıysa hüküm
   **ÖLÇÜLEMEDİ** olur ve kapı başarısız sayılır.

Yalnız düzeltilmiş betiğin koşusu (11/11) kanıt sayıldı; dosyalar sha256
birebir geri yazıldı. **Ders:** kapı betiğinin kendi ayrıştırıcısı da ölçülmeli —
“YEŞİL” hükmü testin gerçekten koştuğunun kanıtı değilse kanıt sayılmaz.

**CI (ölçüldü).** Kod `909b54e` (11 dosya, **+948/−70**) → run `43729719259`
**10/10 YEŞİL**: `dim-storage` **658** iki OS'ta birebir (macos-15 **337,52 s** /
ubuntu-latest **932,35 s**), `dim-cli-adapters` **168**, `arayüz` 66 dosya /
**1434** test (yerel 1437 — fark ağaçtaki başka ajanın commit'siz testleri),
paket `index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB (**arayüz bu turda
değişti**); `masaüstü kabuk` 51/0/5 + gerçek worker 5/0/0, `windows güvenli
depo` 28, `sözleşmeler`, `belge worker'ı`, `SBOM`, `kaynak arşivi`, `sürüm
paketi manifesti` yeşil.

**Dürüst sınırlar.** (a) v37 öncesi çapraz `citation_checks` satırı göçte
başvurduğu sürümün dosyasına **atfedilir** — satırda yazılı dosya bilgisi hiç
yoktu, gerçek aidiyet ayırt **edilemez**; bu satırların temizliği ürün kararıdır.
(b) Var olmayan sürüm kimliği sicilde kalabilir (bilinçli denetim izi); kapı
yalnız “başka dosyada var olan” sürümü reddeder, `case_id`i `NULL` kalmış eski
satır hiçbir listede **görünmez** (tarama sayar). (c) `export_items` kendi
`case_id`'sini taşımaz: dosya üst kayıttan okunur, `exports.case_id` sonradan
değişirse eski kalemler yeni dosyaya “taşınmış” görünür — bugün böyle bir
güncelleme yolu yok. (d) `citation_checks.case_id` **FK değil** (sicil satırı
belge/sürüm silinmesinden sonra yaşamalı). (e) Tetikleyiciler yalnız **yeni**
yazmayı engeller; anahtarı olan saldırgan düşürebilir (uygulama hatasına karşı).
(f) Tauri komutları komut olarak koşulmadı; arayüz davranışı jsdom’da ölçüldü —
gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04k — giden segment sicili kendi segmentine bağlıdır (şema v38, 2026-11-15)

**Neden.** 0034, giden segment sicilinin gösterdiği **sürümün** kendi dosyasından
olmasını kapıladı; satırın gösterdiği **segment kimliği** hiç denetlenmedi.
`transfer_segments` giden metnin kendisini (maskelenmiş hâli) ve o metnin hangi
parçadan geldiğini (`outbound_segment_id` = `source_segment_id`) birlikte taşır:
satır “V sürümünün S segmenti gönderildi” der. Şema bu iki alanın birbirini
tutmasını istemiyordu, `scope_violations()` de bu sınıfı **hiç saymıyordu** (kör).
Sicil, cihazdan **ne** çıktığının kaydıdır; yanlış belgeyi göstermesi P0 sınıfıdır.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v37; göç dosyaları doğrudan
uygulandı, sonda sonra silindi — `scratch/schema-probe/measure_v38.py`).**

1. Doğrudan SQL ile A dosyasının çalıştırmasına (`run1`, `c1`) **A'nın sürümü**
   (`v1`) + **B dosyasının segmenti** (`s9`, `v9`) yazıldı; satır **kabul
   edildi** (`DIM_SCOPE` yok).
2. `list_transfer_segments('run1')` — A dosyasının gönderim sicili —
   `('forged','run1','v1','s9','gizli tutanak metni')` döndürdü: kayıt A'nın
   belgesinden çıkmış gibi görünürken gösterdiği segment **B'nin** parçası.
3. Aynı kasada v34 taraması **0** dedi (kör); v38 taraması **1** diyor.
4. Düzeltmeden sonra aynı satır `DIM_SCOPE:transfer_segments` ile **reddedildi**
   (v38 kasası: yazma yok, tarama 0).

**Yazılan (şema v38, `0038_transfer_segment_identity_guard.sql`).**

* 0034'ün iki tetikleyicisi (`scope_guard_transfer_segments_bi` / `_bu`)
  **genişletilerek** yeniden yazıldı (tek tanım kalsın diye `DROP` + `CREATE`):
  v34 kuralı (sürüm başka dosyada **var ise** reddet) korunur.
* Yeni kural: satırın **iki segment kolonundan** biri (`outbound_segment_id` —
  ekranın gösterdiği alan — ve `source_segment_id`) **var olan** bir segment ise
  gösterilen sürümün segmenti olmalı; sürüm `NULL` ise (0026 öncesi biçim)
  segment **çalıştırmanın dosyasında** olmalı. İki kolon birden denetlenir
  (`s.id IN (…, …)`): üretim yolu ikisini aynı değerle yazar
  (`record_outbound_segments`), ama ayrışırlarsa **gösterilen** alan da kapıdan
  geçmek zorundadır (bu, ilk yazdığım `COALESCE` hâlinin açığıydı — kendi
  kapımı ölçerken buldum, testi ve mutasyonu var).
* `Database::scope_violations()` `transfer_segments` alanını **genişletti**
  (yeni tablo yok): sürüm çaprazlığı **ya da** segment uyuşmazlığı.
* Ret `RAISE(ABORT,'DIM_SCOPE:transfer_segments')` → `StorageError::ScopeViolation`
  → **`E_DB_SCOPE`**.
* **Bilinçli gevşeklik** (0033/0034/0037 ile aynı): **var olmayan** segment
  kimliği sicilde kalabilir — `transfer_segments` bir denetim izidir, belge/sürüm
  silinince segmenti de gider, satır yaşamalıdır (bu tabloda segment yabancı
  anahtar değildir). Kapı yalnız **var olan ama başka sürüme/dosyaya ait**
  segmenti reddeder; tarama da yalnız kanıtlanabilir uyuşmazlığı sayar.

**Testler (güncel kodla).** `scope_guard::tests`
`a_transfer_segment_cannot_claim_a_segment_of_another_version` (kendi sürümünün
iki koşusunun segmenti meşru · var olmayan segment serbest · B'nin segmenti sürüm
`v1` altında **ve** sürüm `NULL` iken reddedilir · iki kolon ayrışırsa reddedilir ·
`UPDATE` ile segment taşıma reddedilir) · `the_scan_counts_transfer_segments_claiming_another_versions_segment`
(kapı düşürülüp eski satır yazılır → tarama **1**; segmenti silinmiş satır aykırı
**değil**) · `upgrade_tests::upgrade_reports_transfer_segments_claiming_another_versions_segment`
(v37 kasa → güncelleme → denetimde `transferSegments: 1`, yeni çapraz yazma
reddedilir, kendi segmenti serbest) → `dim-storage --lib` **658 → 661**
(yerel **465,66 s**, 0 düştü). Arayüz dosyası **değişmedi** (bu dilim şema +
çekirdek sayımı), bu yüzden vitest/tsc koşulmadı. fmt (iki manifest) /
`clippy -p dim-storage --all-targets -D warnings` / masaüstü
`cargo check --all-targets` temiz.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-v38-gates.py`, her mutasyon tek
başına, `try/finally` + `os.utime` + sha256 birebir — `0038…sql fde88192…`,
`scope_guard.rs a836068b…`; koşu **son hâl** üzerinde, `cargo fmt --check` iki
manifestte temiz olduğu doğrulandıktan sonra):** **6/6 KIRMIZI, taban iki uçta
yeşilken** (4/4, koşu sonunda taban tekrar yeşil) — M1 INSERT kapısı segment
kimliğini denetlemez · M2 UPDATE kapısı segment kolonlarında koşmaz · M3 sürümü
`NULL` satırın dosya ölçütü düşer · M4 tarama segment uyuşmazlığını saymaz ·
M5 v34 sürüm/dosya kuralı düşer · M6 iki segment kolonu birlikte denetlenmez.

**Dürüst (kendi hatam, iki ölçüm hatası).**

1. İlk kural `COALESCE(source_segment_id, outbound_segment_id)` kullanıyordu:
   `source_segment_id` meşru + `outbound_segment_id` yabancı olan satır kapıdan
   **geçerdi** — ve ekranın gösterdiği alan tam da `outbound_segment_id`. Kural
   iki kolonu birden denetleyecek biçime çevrildi, testi (M6) eklendi.
2. İlk kapı koşusu **4/5** verdi: M5 “YEŞİL” kaldı çünkü betiğin test süzgeci
   v34 testini (`a_transfer_segment_cannot_record_another_cases_version`)
   içermiyordu — mutasyon o dalı düşürüyor, hiçbir test onu görmüyordu. Süzgeç
   düzeltildi, **6/6** yeniden ölçüldü; ilk koşunun 4'ü kanıt sayılmadı.
   **Ders:** mutasyonun hangi testi kıracağı yazılmadan “kırmızı” sayılmaz.

**CI (ölçüldü).** Kod + belge `9c8c8b1` (5 dosya, **+407/−4**) → run `43739891732`
**10/10 YEŞİL**: `dim-storage` **661** iki OS'ta birebir (macos-15 **358,54 s** /
ubuntu-latest **886,09 s**), `dim-cli-adapters` **168** (5,49 / 5,09 s), `arayüz` 66
dosya / **1434** test, paket `index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB —
**değişmedi** (arayüz dosyasına dokunulmadı); `masaüstü kabuk` (clippy + test),
`windows güvenli depo` 28, `workers`, `sözleşmeler`, `SBOM`, `kaynak arşivi`,
`sürüm paketi manifesti` yeşil.

**Dürüst sınırlar.** (a) Tetikleyiciler yalnız **yeni** yazmayı engeller; v38
öncesi yanlış segmentli satırlar **silinmez**, sayılır (temizlik ürün kararı).
(b) Var olmayan segment kimliği bilinçli serbesttir: uydurulmuş kimlikli satır
kapıdan geçer — kapı *kanıtlanabilir* uyuşmazlığı durdurur, uydurmayı değil.
(c) Satırın metni (`outbound_text`) segmentin metniyle **karşılaştırılmaz**
(maskeli kopya tanımı gereği farklıdır); anahtarı olan bir yazıcı geçerli segment
kimliğiyle keyfî metin yazabilir — bu kapı **kimlik tutarlılığını** korur, içerik
kaynağını kanıtlamaz. (d) Sürümü `NULL` eski biçim satırlarda ölçüt **dosya**
üyeliğidir (sürüm kimliğinden daha zayıf). (e) Tetikleyicileri anahtarı olan
saldırgan düşürebilir (uygulama hatasına karşı). (f) Tauri komutları komut olarak
koşulmadı; arayüz bu turda değişmedi, gerçek pencere + görsel QA **insan
bekliyor**.

## F08-T04l — aktarım onayı kendi dosyasının belgelerini gösterir (şema v39, 2026-11-15)

**Neden.** `transfer_consents` satırı “şu belgeler için aktarım izni verildi” der:
cihazdan **ne** çıkabileceğinin tek yetki kaydıdır. Çekirdek `create_transfer_consent`
listeyi doğruluyor (her kimlik `case_scope` dosyasında, silinmemiş bir belge olmalı;
boş seçim hiçbir şeyi yetkilendirmez), ama **şemada hiçbir kapı yoktu** — sütun
serbest metin, `scope_violations()` de bu tabloyu hiç saymıyordu (kör). 0031–0038
serisinin son açık JSON dizi referansı buydu.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v38; geçici sonda
`crates/storage/tests/probe_consent_scope.rs` — kanıt alındıktan sonra silindi).**

1. Doğrudan SQL ile A dosyasının onayına **B dosyasının belgesi** yazıldı:
   `UPDATE transfer_consents SET selected_document_ids='["d9"]'` → **`Ok(1)`**.
2. `list_transfer_consents('c1')` — A dosyasının onay envanteri — `belge_sayisi=1
   kimlikler=["d9"]` döndürdü: ekranın “onaylanan belgeler” sütunu B dosyasının
   belgesini A'nın onaylanmış kapsamı diye sayar (sicil yanlış dosyayı gösterir).
3. Boş liste (`[]`) ve **bozuk JSON** (`bozuk-json`) onay satırı olarak yazılabildi;
   bozuk satır `serde_json::from_str(...).unwrap_or_default()` ile **sessizce**
   “0 belge” okunur — yetki kaydı çöker, kimse görmez.
4. Silinmiş belgenin kimliği de yazılabildi (`deleted_at` denetlenmiyor).
5. `scope_violations()` toplam **0** dedi (sınıf hiç sayılmıyordu).
6. Düzeltmeden sonra aynı yazmalar `DIM_SCOPE:transfer_consents` ile **reddedildi**;
   meşru onay (kendi belgesi) ve **iptal** (`revoked_at`) çalışmaya devam etti.

**Dürüst sınır (kusurun gerçek ağırlığı).** Gönderim yolu çapraz kimliği **zaten**
düşürüyordu: `consent_scope_segments` → `resolve_transfer_scope` dosya süzgeciyle
belgeyi `missing` diye atlar, `ai_run_execute` gönderilecek segment yoksa
`E_AI_UNSUPPORTED` verir — yani çapraz onay satırı **bayt çıkarmıyordu**. Kusur
yetki **kaydındaydı** (envanter yanlış kapsam gösterir, tarama görmez), 0031/0033
ile aynı sınıf: doğrudan SQL yazan bir yol çekirdek kapısını atlar.

**Yazılan (şema v39, `0039_transfer_consent_document_scope.sql`).**

* **2 tetikleyici** (`scope_guard_transfer_consents_bi` / `_bu`):
  `selected_document_ids` **geçerli JSON dizi** olmalı, elemanları metin, liste
  **boş olamaz**; dizinin **her elemanı** `case_scope` dosyasında **silinmemiş**
  bir `documents` satırı göstermeli (yoksa satır yazılmaz — fail-closed). Kural
  çekirdeğin `create_transfer_consent` kapısının **aynısı**.
* `UPDATE OF case_scope, selected_document_ids`: yalnız bu iki sütunu değiştiren
  yazma kapıdan geçer. **İptal** ve süre güncellemesi takılmaz (v35 dersi:
  eskimiş referans başka alanın güncellemesini engellememeli). Eski aykırı satır
  bu yüzden hâlâ **iptal edilebilir**.
* `json_array_length` yalnız `json_valid(...) AND json_type(...)='array'` olan
  değerde `CASE` ile çağrılır: bozuk JSON'da SQLite'ın “malformed JSON” hatası
  değil kapının kendi kodu döner.
* `Database::scope_violations()` **`transfer_consents` alanı** eklendi (yeni tablo
  yok): çapraz dosya belgesi, boş liste, bozuk/array olmayan JSON. `GUARDED_TABLES`
  22 → **23**; şemadan türetim kuralı `(case_scope + selected_document_ids)`
  çiftini kapsar (kapsam sütunu `case_id` adını taşımayan ilk tablo).
* Ret `RAISE(ABORT,'DIM_SCOPE:transfer_consents')` → `StorageError::ScopeViolation`
  → **`E_DB_SCOPE`**.
* **Bilinçli gevşeklik** (0033/0034/0038 ile aynı): **var olmayan** ya da sonradan
  **silinmiş** belge kimliği ne kapıya takılır ne taranır — onay verildikten sonra
  belge silinmiş olabilir; bu denetim izidir, kanıtlanabilir uyuşmazlık değildir
  (gönderim yolu onu yine atlar).

**Testler (güncel kodla).** `scope_guard::tests`
`a_transfer_consent_cannot_name_another_cases_document` (kendi belgeleri meşru —
tek ve iki elemanlı · B'nin belgesi ve **ters yön** reddedilir · boş liste · bozuk
JSON · dizi olmayan değer · var olmayan kimlik · silinmiş belge reddedilir ·
`UPDATE` ile kapsam genişletme/taşıma reddedilir · **iptal takılmaz**) ·
`the_scan_counts_transfer_consents_written_before_the_guard_existed` (kapı
düşürülüp eski satırlar yazılır → tarama **3** = çapraz + boş + bozuk; sonradan
silinen belge aykırı **değil**; satırlar silinmez) ·
`upgrade_tests::upgrade_reports_transfer_consents_naming_another_cases_document`
(v38 kasa → güncelleme → denetimde `transferConsents: 3`, yeni çapraz yazma
reddedilir, kendi belgesi serbest, eski aykırı satır **iptal edilebilir**) →
`dim-storage --lib` **661 → 664** (yerel **464,49 s**, 0 düştü). Arayüz dosyası
**değişmedi** (bu dilim şema + çekirdek sayımı) → vitest/tsc koşulmadı. fmt (iki
manifest) / `clippy -p dim-storage --all-targets -D warnings` temiz.

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-v39-gates.py`, her mutasyon tek
başına, `try/finally` + `os.utime` + sha256 birebir — `0039…sql 0addec9c…`,
`scope_guard.rs f1b254e4…`; koşu **son hâl** üzerinde, `cargo fmt` sonrası):**
**8/8 KIRMIZI, taban iki uçta yeşilken** (K0: kapı+tarama+upgrade **3/3**, türetim
**1/1**; koşu sonunda K1 aynı, geri yazım birebir) — M1 INSERT kapısı belgenin
dosyasını/silinmesini denetlemez · M2 boş liste kapısı düşer · M3 JSON
geçerlilik/tür kapısı düşer · M4 `UPDATE OF` listesinde belge listesi yok (kapsam
genişletme geçer) · M5 UPDATE kapısı **yeni** dosyayı değil **eski** dosyayı
denetler · M6 tarama çapraz belgeyi saymaz · M7 tablo listesinden çıkarma
(şemadan türetim testi yakalar) · M8 tarama boş listeyi saymaz.

**Dürüst (kendi hatam).** Kapı betiğinin ilk koşusu **hiç başlamadı**: M1 deseni
(`WHERE d.id = j.value AND d.case_id = NEW.case_scope …`) BI **ve** BU
gövdelerinde birebir aynı olduğu için “2 kez eşleşti” hatası verdi; BI'ye özgü
`first()` yardımcısı eklendi (M2/M3 de aynı nedenle), 8/8 yeniden ölçüldü. İlk
koşunun hiçbir hükmü kanıt sayılmadı. **Ders:** ortak gövde metni taşıyan iki
tetikleyicide mutasyon deseni tekilliğini doğrulamadan koşmamalı.

**CI (ölçüldü).** Kod `709b6be` (4 dosya, **+358/−5**) → run `43748617813`
**10/10 YEŞİL** (`dim-storage` **664** iki OS'ta birebir: macos-15 **324,80 s** /
ubuntu-latest **730,69 s**; `arayüz` 66 dosya / **1434** test, paket
`index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB — **değişmedi**; `masaüstü kabuk`
clippy+test, `windows güvenli depo` 28, `workers`, `sözleşmeler`, `SBOM`,
`kaynak arşivi`, `sürüm paketi manifesti`). `HEAD == origin/main` = `709b6be`;
ağaçta yalnız başka ajanın 9 dosyası.

**Dürüst sınırlar.** (a) Tetikleyiciler yalnız **yeni** yazmayı engeller; v39
öncesi çapraz/boş/bozuk onay satırları **silinmez**, sayılır (temizlik ürün
kararı; v39 öncesi `[]` satırı 2026-10-29'dan önceki kural boşluğundan da gelebilir).
(b) Kapı **kaydı** korur, **onay eylemini** kanıtlamaz: anahtarı olan bir yazıcı
aynı dosyanın **başka** canlı belgesini listeye ekleyebilir — bu, onaylanan
kapsamı sessizce genişletir; çekirdek yolu bunu yapmaz, şema düzeyinde
“kullanıcı neyi onayladı” bilgisi yoktur (kayıt **kendisi** onaydır).
(c) Var olmayan/silinmiş belge kimliği bilinçli serbesttir; uydurulmuş kimlikli
satır geçer. (d) `case_scope` **FK değil**: var olmayan dosyayı gösteren onay
satırı hiçbir listede görünmez (sayılmaz). (e) Tetikleyicileri anahtarı olan
saldırgan düşürebilir (uygulama hatasına karşı). (f) Tauri komutları komut olarak
koşulmadı; arayüz bu turda değişmedi — gerçek pencere + görsel QA **insan
bekliyor**.

## F08-T04m — karar hedefi kendi dosyasında olmalı (şema v40, 2026-11-16)

**Neden.** `review_items.target_entity_id`, avukatın onayladığı önerinin dönüştüğü
kalıcı kaydı (`claims` / `notes` / `tasks` / `events`) gösterir. Arayüz onu
`review.targetBadge` ile öğenin üstünde **ilk 8 karakter** olarak yazar; öğe
`target_entity_id IS NULL` şartı yüzünden „karara bağlandı“ sayılır ve bir daha
karara bağlanamaz. Şemada hiçbir kapı yoktu; `scope_violations()` bu tabloyu
sayıyor ama **bu sütunu hiç denetlemiyordu**.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v39; `scratch/schema-probe/measure_v40.py`,
log `scratch/dim-v40-measure.log`, kasa `scratch/schema-probe/probe-39-target.db`):**

1. Doğrudan SQL ile A dosyasının öğesine **B dosyasının** `claim9` / `n9` kimliği
   yazıldı → **KABUL** (`rows=1`).
2. `list_review_items('c1')` — okuma yolunun aynısı — onu A'nın öğesi diye
   döndürdü: ekran B dosyasının kaydının kimliğini A'nın kararı sanar.
3. v39 taraması **0** dedi (sınıf hiç sayılmıyordu); önerilen ek koşul **2** buldu.
4. Karar kapısı yanlış hedefle **kilitleniyordu** (`UPDATE … WHERE target_entity_id
   IS NULL` → `changed=0`): öğe bir daha karara bağlanamaz.
5. Var olmayan kimlik **geçiyor** — hedef sonradan silinebilir, bu bilinçli
   serbest olmalı (0033/0038/0039 gevşekliği).

**Yazılan (şema v40, `0040_review_target_scope_guard.sql`).** 0033'ün iki
tetikleyicisi (`scope_guard_review_items_bi/_bu`) `DROP` + `CREATE` ile
**genişletildi** (tek tanım): `target_entity_id` doluysa kimlik `claims` /
`notes` / `tasks` / `events` tablolarından **birinde var** ise satırın `case_id`'si
aynı olmalı; yoksa satır yazılmaz (fail-closed). `UPDATE OF case_id,
source_anchor_ids, target_entity_id`: yalnız bu üç sütunu değiştiren yazma
kapıdan geçer — **durum/`row_version` güncellemesi takılmaz** (v35 dersi), eski
aykırı satır bu yüzden karara bağlanabilir/reddedilebilir kalır. **Bilinçli
gevşeklik** (0033/0034/0038/0039 ile aynı): var olmayan/silinmiş hedef kimliği ne
kapıya takılır ne taranır (denetim izi). `scope_violations()` `review_items` alanı
**genişledi** (yeni tablo yok → `GUARDED_TABLES` **23 kalır**, şemadan türetim
testi değişmedi). Ret `RAISE(ABORT,'DIM_SCOPE:review_items')` →
`StorageError::ScopeViolation` → **`E_DB_SCOPE`**.

**Testler.** `scope_guard::tests` **+2**
(`a_review_item_cannot_point_at_another_cases_entity`: dört tablo için çapraz
hedef reddi · kendi dosyasının kaydı/hedefsiz/var olmayan serbest · UPDATE ile
taşıma reddi · durum güncellemesi geçer · dosya değiştirme hedefi yabancılaştırır ·
`the_scan_counts_review_items_pointing_at_another_cases_entity`: kapı düşürülüp
eski satırlar yazılır → tarama **1**, var olmayan hedef sayılmaz, satırlar
silinmez), `upgrade_tests` **+1** (v39 kasa → güncelleme → denetimde
`reviewItems: 1`, yeni çapraz yazma reddi, eski satırın durumu güncellenebilir) →
`dim-storage --lib` **664 → 667**. Hedefli koşu `cargo test -p dim-storage --lib
review_item` **9/9 yeşil**; **tam `--lib` bu turda yerelde koşulmadı** (tur
bütçesi) — tam paket CI'da `cargo test --workspace --all-targets` ile koşuyor.
`cargo fmt` (iki manifest) temiz; clippy yerelde koşulmadı (CI `-D warnings`).

**Kapı kanıtı (gerçek koşular; betik `scratch/dim-v40-gates.py`, log
`scratch/dim-v40-gates.log`; her mutasyon tek başına, `try/finally` + `os.utime` +
sha256 birebir — `0040…sql df5b9aa1c7ccc116`, `scope_guard.rs d998e54851261b7c`):**
**8/8 KIRMIZI, taban iki uçta yeşilken** (K0 **9/9**, K1 **9/9**) — M1 BI hedef
kuralı düşer · M2 BU hedef kuralı düşer · M3 BI eşitliği ters (meşru hedef
reddedilir) · M4 `UPDATE OF` listesinden target kolonu düşer · M5 BI yalnız
`claims`i denetler (notes/tasks/events kör) · M6 tarama hedef koşulu düşer ·
M7 tarama eşitliği ters · M8 tarama yalnız `notes`a bakar (claims kör).
**Dürüst (kendi hatam):** ilk koşu **M3'te durdu** — `x.case_id <> NEW.case_id`
BI gövdesinde **4 kez** geçiyor (dört tablo), tekillik assert'i yakaladı ve betik
hiçbir hüküm yazmadan çıktı; desen `claims` satırıyla daraltıldı, **8/8 yeniden
ölçüldü**. İlk koşunun iki hükmü (M1/M2) kanıt sayılmadı.

**CI (ölçüldü).** Kod `ad66689` (4 dosya, **+293/−2**) → run `43769333157`
**10/10 YEŞİL** (`dim-storage` **667** iki OS'ta birebir: macos-15 **352,99 s** /
ubuntu-latest **364,78 s**; `arayüz` 66 dosya / **1434** test, paket
`index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB — **değişmedi**; `masaüstü kabuk`
clippy+test, `windows güvenli depo` 28, `workers`, `sözleşmeler`, `SBOM`,
`kaynak arşivi`, `sürüm paketi manifesti`). `HEAD == origin/main` = `ad66689`.

**Dürüst sınırlar.** (a) Tetikleyiciler yalnız **yeni** yazmayı engeller; v40
öncesi çapraz hedefler **silinmez**, sayılır (temizlik ürün kararı). (b) Hedefin
**türü** ile `item_kind` karşılaştırılmaz (şemada tür bilgisi tek sütunda değil):
kapı kimliğin **dosyasını** korur, „bu öğe bu tür kayda dönüştü“ iddiasını
kanıtlamaz. (c) Var olmayan/silinmiş hedef bilinçli serbesttir; uydurulmuş
kimlikli satır geçer. (d) `review_items.superseded_by_item_id` (aynı tabloya
referans) bu dilimin dışında. (e) Tetikleyicileri anahtarı olan saldırgan
düşürebilir (uygulama hatasına karşı). (f) Arayüz bu turda değişmedi; Tauri
komutları komut olarak koşulmadı — gerçek pencere + görsel QA **insan bekliyor**.


## F08-T04n — yerine geçen kayıt kendi dosyasında olmalı (şema v41, 2026-11-16)

**Neden.** `review_items.superseded_by_item_id`, "bu öğenin yerine hangi öğe geçti"
bağını tutar; arayüzde gerçek bir eylem var (`review.supersedeAction` düğmesi →
`review_supersede` → `supersede_review_item`) ve çekirdek aynı dosya, kendini
göstermeme, durum kurallarını denetler — ama **şemada hiçbir kapı yoktu**. 0006
sütunu `REFERENCES review_items(id) ON DELETE SET NULL` ile ekledi: yabancı anahtar
satırın **varlığını** korur, **dosyasını** korumaz. Önceki turun (T04m) dürüst
sınırlar listesinde (d) olarak yazılıydı.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v40, `PRAGMA foreign_keys=ON`;
`scratch/schema-probe/measure_v41.py`, log `scratch/dim-v41-measure.log`, kasa
`scratch/schema-probe/probe-40-supersede.db`):**

1. A dosyasının öğesine **B dosyasının** `j9` öğesi "yerine geçen" yazıldı →
   **KABUL** (`rows=1`; yabancı anahtar aynı tabloya baktığı için susuyor).
2. Öğe **kendini** gösterdi (`i2 → i2`) → **KABUL**; çekirdek bunu reddediyor.
3. **Damgasız** yazım (`superseded_by_item_id` dolu, `superseded_at` NULL) →
   **KABUL**; `list_review_items('c1')` öğeyi **hâlâ kuyrukta, karara açık**
   gösterdi (durum `superseded_at`ten türetilir) — satır "yerine geçildi" derken
   ekran "bekliyor" diyor.
4. Aynı satırda çekirdeğin gerçek `supersede_review_item` UPDATE'i
   (`… WHERE id=? AND superseded_at IS NULL`) **çalıştı** (`changed=1`) ve ilk
   işareti **sessizce ezdi**: denetim izi ile satır çelişir.
5. `list_review_items('c1', 'superseded')` çapraz işaretli kaydı A dosyasının
   öğesi diye döndürdü (`i1 → j9`, `j9` `c2`'de).
6. v40 taraması **0** dedi (sınıf kör); önerilen koşul **3** buldu.
7. Meşru biçim (`ON DELETE SET NULL`: damga var, işaret yok) önerilen koşula
   **takılmadı** (0) — tek yönlü kural.

**Yazılan (şema v41, `0041_review_supersede_scope_guard.sql`).** 0033/0040'ın iki
tetikleyicisi (`scope_guard_review_items_bi/_bu`) `DROP` + `CREATE` ile
**genişletildi** (tek tanım): `superseded_by_item_id` doluysa (a) `superseded_at`
de dolu olmalı, (b) satır kendini gösteremez, (c) kimlik `review_items`ta **başka
dosyada var** ise satır yazılmaz (fail-closed). `UPDATE OF case_id,
source_anchor_ids, target_entity_id, superseded_by_item_id, superseded_at`: yalnız
bu beş sütunu değiştiren yazma kapıdan geçer — durum/`row_version` güncellemesi
takılmaz (v35 dersi). `scope_violations()` `review_items` alanı **genişledi** (yeni
tablo yok → `GUARDED_TABLES` **23 kalır**, şemadan türetim testi değişmedi).
Ret `RAISE(ABORT,'DIM_SCOPE:review_items')` → `StorageError::ScopeViolation` →
**`E_DB_SCOPE`**. **Kasıtlı gevşeklik** (0033/0038/0039/0040 ile aynı): var
olmayan/silinmiş yerine-geçen kimliği ne kapıya takılır ne taranır (0006'nın
yabancı anahtarı zaten yazdırmaz; hedef silinirse `ON DELETE SET NULL` yalnız
işareti çeker, damga kalır — bu yüzden kural tek yönlüdür). Yerine geçen öğenin
**durumu** denetlenmez: bugün "pending" olan yerine-geçen sonradan reddedilebilir
(tarihsel gerçek, kanıtlanabilir uyuşmazlık değil).

**Testler (güncel kodla).** `scope_guard::tests` **+2**
(`a_superseded_review_item_points_at_its_own_case`: damgasız/kendini gösteren/
çapraz INSERT reddi · aynı dosyada damgalı bağ, damga-tek-başına ve bağsız öğe
meşru · UPDATE ile çapraz/kendine/damgasız taşıma reddi · çekirdek yolu geçer ·
durum güncellemesi takılmaz · dosya taşıma bağı yabancılaştırır),
`the_scan_counts_cross_case_and_half_written_supersede_pointers`: kapı düşürülüp
eski satırlar yazılır → tarama **3**, `ON DELETE SET NULL` biçimi sayılmaz,
satırlar silinmez), `upgrade_tests` **+1** (v40 kasa → güncelleme → denetimde
`reviewItems: 2`, yeni çapraz bağ reddi, eski satırın durumu güncellenebilir) →
`dim-storage --lib` **667 → 670** — **tam paket yerelde koşuldu: 670/670 yeşil,
504,39 s**. `cargo fmt` (iki manifest) temiz; `clippy -p dim-storage --all-targets
-D warnings` temiz; masaüstü `cargo check --all-targets` temiz (2 dk 13 s).
**Dürüst (kendi hatam):** yeni testin ilk hâli çapraz INSERT'i (`iz` → `j9`)
yabancı öğe **henüz yokken** deniyordu; yabancı anahtar tetikleyiciden **önce**
düştü ve `scope_error` FK hatasıyla panikledi — ölçüm "kapı çalıştı" sanılacaktı.
`j9` önce yazıldı, test yeniden koşuldu (12/12).

**Kapı kanıtı.** **10/10 KIRMIZI, taban iki uçta yeşilken** (betik
`scratch/dim-v41-gates.py`, log `scratch/dim-v41-gates.log`; her mutasyon tek
başına, `try/finally` + `os.utime` + sha256 birebir — `0041…sql
f46effb3585127f5`, `scope_guard.rs a9c15e59a5f39d43`): M1 BI yerine-geçen kuralı
düşer · M2 BU kuralı düşer · M3 BI damga koşulu düşer · M4 BU damga koşulu düşer ·
M5 BI kendini-gösterme kuralı düşer · M6 BU kendini-gösterme kuralı düşer ·
M7 BU `UPDATE OF` listesinden iki sütun düşer · M8 tarama yerine-geçen koşulu
düşer · M9 tarama damgasızlık alt koşulu düşer · M10 tarama çapraz eşitliği ters.
K0 **12/12**, K1 **12/12**.

**CI (ölçüldü).** Kod `3639754` (4 dosya, **+335/−2**; `+129` yeni göç dosyası) →
run `43777585572` **10/10 YEŞİL** (`dim-storage` **670** iki OS'ta birebir:
macos-15 **424,03 s** / ubuntu-latest **380,63 s**; `dim-cli-adapters` 168;
`arayüz` 66 dosya / **1434** test, paket `index-DHBc7fVy.js` 694,53 kB / gzip
185,32 kB — **değişmedi**; `masaüstü kabuk` 51/0/5 + gerçek worker 5/0/0;
`windows güvenli depo` 28; `workers`, `sözleşmeler`, `SBOM`, `kaynak arşivi`,
`sürüm paketi manifesti`). `HEAD == origin/main` = `3639754`; ağaçta yalnız
başka ajanın 9 dosyası (2026-11-07'den beri) — dokunulmadı.

**Dürüst sınırlar.** (a) Tetikleyiciler yalnız **yeni** yazmayı engeller; v41
öncesi çapraz/damgasız bağlar **silinmez**, sayılır (temizlik ürün kararı).
(b) Bağ bugün **hiçbir ekranda çizilmiyor** — arayüz yalnız `superseded` durum
etiketini yazar; kapı bugünkü görünür davranışı değiştirmez, duruktaki dosya
sınırını korur ve ileride bağı gösteren/navigate eden bir ekranın başka dosyaya
sızmamasını sağlar. (c) Damgasız bağ artık **iki adımda** yazılamaz (önce işaret
sonra damga); çekirdek zaten tek UPDATE'te yazıyor, ama ileride iki adımlı bir
yazıcı eklenirse önce damga yazmalı. (d) Yerine geçenin **türü** ile `item_kind`
karşılaştırılmaz (v40'ın aynı sınırı). (e) Tetikleyicileri anahtarı olan saldırgan
düşürebilir (uygulama hatasına karşı). (f) Arayüz bu turda değişmedi; gerçek
pencere + görsel QA **insan bekliyor**.

## F08-T04o — kontrol listesi kanıt bağları kendi dosyasında olmalı (şema v42, 2026-11-16)

**Neden.** `case_checklist_items.linked_document_ids`, "bu kontrol listesi
maddesi **bulundu**" iddiasının tek kanıtıdır (şartname veri modeli satırı:
`case_id, template_version_id, item_text, state, linked_document_ids, note`;
AI-07 "incelenen kapsamda bulunan/bulunamayan belgeler"). Çekirdek
`set_case_checklist_state` her kimliği **kendi dosyasında, silinmemiş** bir
belge olarak doğruluyor; `present` en az bir bağ istiyor; `open` / `missing` /
`not_applicable` bağ kabul etmiyor. Ama **şemada hiçbir kapı yoktu** — sütun
serbest metindi ve `scope_violations()` bu tabloyu hiç saymıyordu. Doğrudan SQL
yazan bir yol (göç, onarım betiği, ileride eklenecek ikinci yazıcı) bu kuralları
atlıyordu.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v41, `PRAGMA foreign_keys=ON`;
`scratch/schema-probe/measure_v42.py`):**

1. A dosyasının öğesine doğrudan SQL ile **B dosyasının** belgesi yazıldı
   (`UPDATE … state='present', linked_document_ids='["d9"]'` → `rows=1`);
2. A'nın listesi öğeyi `('c1','present','["d9"]')` diye döndürdü; belge adı
   A dosyasında **çözülemiyor** (`NULL`) — ekran "Bulundu" rozetiyle **çıplak
   UUID** gösterir (arayüz `checklists.staleLinks` uyarısı yalnız "belge
   silinmiş veya arşivlenmiş olabilir" der);
3. `present` durumu **boş bağ listesiyle** yazılabildi: kanıtsız iddia;
4. **bozuk JSON** (`oops`) yazılabildi; okuma yolu (`map_case_item`) satırı
   çözemiyor → `list_case_checklist` **tüm listeyi** hata ile düşürür; tek
   bozuk satır kontrol listesi ekranını kapatır;
5. `missing` ("bulunamadı") durumu **bağ listesiyle** yazılabildi (çelişki);
6. v41 taramasında bu tablo için **alan yok** (`GUARDED_TABLES` 23) — sınıf kör;
7. `open` durumunda bağ taşıyan satır da kabul edildi (durum yeniden açılırken
   bağ temizlenmezse çelişki kalır).

**Yazılan (şema v42, `0042_checklist_link_scope_guard.sql`).** İki tetikleyici
(`scope_guard_case_checklist_items_bi/_bu`): `linked_document_ids` **geçerli
JSON dizi** olmalı, elemanları metin, en çok **50** eleman (çekirdeğin
`MAX_LINKS` sınırı); dizinin **her elemanı** `case_id` dosyasında **silinmemiş**
bir `documents` satırı göstermeli (fail-closed); durum ile liste çelişemez:
`present` **en az bir** bağ ister, `open`/`missing`/`not_applicable` bağ kabul
etmez. `UPDATE OF case_id, state, linked_document_ids`: yalnız bu üç sütunu
değiştiren yazma kapıdan geçer — **not** ve `row_version` güncellemesi takılmaz
(v35 dersi). Tarama `case_checklist_items` alanı **eklendi** (`GUARDED_TABLES`
**24**; türetim kuralı `case_id` + `linked_document_ids` çiftini kapsar). Ret
`RAISE(ABORT,'DIM_SCOPE:case_checklist_items')` → `StorageError::ScopeViolation`
→ **`E_DB_SCOPE`**. **Kasıtlı gevşeklik** (0033/0039 ile aynı): **sonradan**
silinmiş belge kimliği kapıya takılmaz (kapı yalnız yeni yazmada koşar) ve
taranmaz — bağ kurulduktan sonra belge silinebilir, bu tarihsel bir gerçektir.

**Testler (güncel kodla).** `scope_guard::tests` **+2**
(`a_checklist_link_must_name_a_document_of_its_own_case`: INSERT ve UPDATE
yollarında çapraz/kanıtsız/bozuk/dizi-olmayan/51 eleman/silinmiş belge/var
olmayan kimlik reddi · aynı dosyada bağ meşru · dosya taşıma bağı
yabancılaştırır · not güncellemesi takılmaz,
`the_scan_counts_checklist_links_written_before_the_guard_existed`: kapı
düşürülüp eski satırlar yazılır → tarama **7**, sonradan silinen belge
sayılmaz, satırlar silinmez), `upgrade_tests` **+1** (v41 kasa → güncelleme →
denetimde `caseChecklistItems: 3`, yeni çapraz bağ reddi, eski satırın notu
güncellenebilir) → `dim-storage --lib` **670 → 673** — **tam paket yerelde
koşuldu: 673/673 yeşil, 521,62 s**. `cargo fmt` (iki manifest) temiz;
`clippy -p dim-storage --all-targets -- -D warnings` temiz; masaüstü
`cargo check --all-targets` temiz (1 dk 55 s).

**Kapı kanıtı.** **11/11 KIRMIZI, taban iki uçta 14/14 yeşilken** (betik
`scratch/dim-v42-gates.py`, loglar `scratch/dim-v42-gates.log` ve
`dim-v42-gates-m9.log`; her mutasyon tek başına, `try/finally` + `os.utime` +
sha256 birebir — `0042…sql fdb948af4a646412`, `scope_guard.rs
d8cb089a2ea080df`): M1 BI çapraz-dosya kuralı düşer · M2 BU çapraz-dosya kuralı
düşer · M3 BI "present" boş liste kuralı düşer · M4 BU "present" boş liste
kuralı düşer · M5 BI 50 sınırı düşer · M6 BU `UPDATE OF` listesinden `case_id`
düşer · M7 tarama çapraz eşitliği kapanır · M8 tarama "present" boş liste
koşulu düşer · M9 tarama "present dışında dolu" koşulu ters · M10 tarama
metin-dışı eleman koşulu düşer · M11 tarama 50 sınırı düşer. K0 **14/14**, K1
**14/14**. **Dürüst (kendi hatam):** M9'un ilk biçimi (`<> 0` → `<> -2`)
pozitif uzunluklarda davranışı **değiştirmiyordu** (`1 <> -2` doğru) — kapı
boşluğu değil, **etkisiz mutasyon**; ilk koşunun M9 hükmü kanıt sayılmadı,
`= 0` biçimiyle tek başına yeniden ölçüldü (K0 14/14 → KIRMIZI 12/14 → K1
14/14).

**CI (ölçüldü).** Kod `275da27` (4 dosya, **+410/−4**; `+107` yeni göç dosyası)
→ run `43831663715` **10/10 YEŞİL** (`dim-storage` **673** iki OS'ta birebir:
macos-15 **391,43 s** / ubuntu-latest **746,13 s**; `arayüz` 66 dosya /
**1434** test, paket `index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB —
**değişmedi**; `masaüstü kabuk`, `windows güvenli depo`, `workers`,
`sözleşmeler`, `SBOM`, `kaynak arşivi`, `sürüm paketi manifesti`).

**Dürüst sınırlar.** (a) Tetikleyiciler yalnız **yeni** yazmayı engeller; v42
öncesi çapraz/bozuk/çelişkili satırlar **silinmez**, sayılır (temizlik ürün
kararı). (b) Arayüz bu turda **değişmedi**: ekran zaten "listede görünmeyen
bağlar" uyarısını gösteriyordu; kapı bugünkü görünür davranışı değiştirmez,
duruktaki dosya sınırını korur (çekirdek zaten reddediyordu). (c) Bağın
**türü/içeriği** denetlenmez — yalnız kimliğin dosyası ve varlığı; "bu belge
gerçekten o maddeyi kanıtlıyor" iddiası hukukî değerlendirmedir. (d) Aynı
belgeye birden çok kez bağlanma (tekrar) şemada engellenmez; çekirdek
`set_case_checklist_state` listeyi tekilleştirir. (e) Tetikleyicileri anahtarı
olan saldırgan düşürebilir (uygulama hatasına karşı). (f) Gerçek Tauri
penceresi + görsel QA **insan bekliyor**; bu turda arayüz dosyası değişmedi.

## F08-T04p — çalıştırmanın yasal dayanağı kendi dosyasında olmalı (şema v43, 2026-11-16)

**Neden.** `ai_runs.consent_id`, "bu gönderim hangi onayla yapıldı" sorusunun
tek kaydıdır (şartname §13.5 aktarım onayı; `transfer_consents` yetki kaydı).
Çekirdek `start_ai_run` onayın `case_scope`unu, belge listesini, sağlayıcı
özetini ve maske bağını denetliyor; ama **şemada hiçbir kapı yoktu** — sütun
serbest metindi ve `scope_violations()` bu tabloyu hiç saymıyordu. Doğrudan SQL
yazan bir yol (göç, onarım betiği, ileride eklenecek ikinci yazıcı) bu kuralları
atlıyordu. Ayrıca `ai_runs.case_id` **değiştirilebiliyordu**: çalıştırma başka
dosyaya taşınınca mevcut gönderim sicili (`transfer_segments`,
`ai_context_items`) sessizce çaprazlaşıyor — v34 taraması bunu sonradan **1
aykırı** olarak görmeye başlıyordu, yani hasar tek UPDATE ile doğuyordu.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v42, `PRAGMA foreign_keys=ON`;
sonda `scratch/dim-v43-probe/measure_v43.py`, log `measure_before.log`):**

1. A dosyasının çalıştırmasına doğrudan SQL ile **B dosyasının onayı** yazıldı
   (`INSERT … consent_id='tc9'` → KABUL, `rows=1`);
2. Var olan çalıştırmanın onayı UPDATE ile B'nin onayına çevrildi
   (`UPDATE ai_runs SET consent_id='tc9'` → KABUL);
3. Çalıştırma başka dosyaya taşındı (`UPDATE ai_runs SET case_id='c2'` →
   KABUL); `list_ai_runs('c2')` satırı B'nin çalıştırması diye döndürdü;
   satırın **A'nın sürümünü** gösteren gönderim sicili yerinde kaldı ve v34
   taraması onu **1** aykırı saydı;
4. v42 taramasında `ai_runs` için **alan yok** (`GUARDED_TABLES` 24) — sınıf kör;
5. Var olmayan onay kimliği yazılabildi (bilinçli gevşeklik; aşağıda).

**Yazılan (şema v43, `0043_ai_run_consent_scope_guard.sql`).** İki tetikleyici:
`scope_guard_ai_runs_bi` — `consent_id` doluysa ve o kimlikte bir onay
**varsa**, onayın `case_scope`u satırın `case_id`si olmalı (fail-closed);
`scope_guard_ai_runs_bu` — `UPDATE OF case_id, consent_id`, iki sütun
**değişmez** (`IS NOT`; mevcut kod yalnız `status`/`usage_report`/
`completed_at` günceller). Durum güncellemesi takılmaz (v35 dersi). Tarama
`ai_runs` alanı **eklendi** (`GUARDED_TABLES` **25**; türetim kuralı `case_id`
+ `consent_id`). Ret `RAISE(ABORT,'DIM_SCOPE:ai_runs')` →
`StorageError::ScopeViolation` → **`E_DB_SCOPE`**; güncelleme taraması denetime
`aiRuns` sayısıyla düşer. **Kasıtlı gevşeklik** (0033/0034/0038/0039 ile aynı):
**var olmayan** onay kimliği ne kapıya takılır ne taranır — kayıt denetim
izidir (onay satırları üründe silinmez; iptal `revoked_at` ile yazılır).

**Testler (güncel kodla).** `scope_guard::tests` **+2**
(`an_ai_run_cannot_cite_another_cases_consent`: kendi onayı/NULL/var olmayan
kimlik meşru · çapraz onay reddi · `consent_id` ve `case_id` UPDATE reddi ·
durum güncellemesi takılmaz; `the_scan_counts_ai_runs_citing_another_cases_consent`:
kapı düşürülüp eski satır yazılır → tarama **1**, NULL/var olmayan kimlik
sayılmaz, satırlar silinmez), `upgrade_tests` **+1**
(`upgrade_reports_ai_runs_citing_another_cases_consent`: v42 kasa → güncelleme
→ denetimde `aiRuns: 1`, yeni çapraz yazma reddi, eski satırın durumu
güncellenebilir) → `dim-storage --lib` **673 → 676** — **tam paket yerelde
koşuldu: 676/676 yeşil, 450,33 s**. `cargo fmt` (iki manifest) temiz;
`clippy -p dim-storage --all-targets -- -D warnings` temiz; masaüstü
`cargo check --all-targets` temiz.

**Kapı kanıtı.** **9/9 KIRMIZI, taban iki uçta 4/4 yeşilken** (betik
`scratch/dim-v43-probe/gates_v43.py`, log `gates_v43.log`; her mutasyon tek
başına, `try/finally` + `os.utime` + sha256 birebir — `0043…sql
758d7b100acefe24`, `scope_guard.rs ff92d7506eb89dc0`): M1 BI çapraz-dosya
kuralı düşer · M2 BU kuralı düşer · M3 BI eşitliği ters · M4 BU `UPDATE OF`
listesinden `consent_id` düşer · M5 tarama EXISTS koşulu düşer · M6 tarama
eşitliği ters · M7 `GUARDED_TABLES`tan tablo düşer (türetim ayrışır) · M8
türetim listesinden `consent_id` düşer (tablo kapsamsız kalır) · M9 BU
`UPDATE OF` listesinden `case_id` düşer (dosya taşınır).

**CI (ölçüldü).** Kod `667dd37` (4 dosya, **+296/−2**; `+60` yeni göç dosyası)
→ run `43847057980` **10/10 YEŞİL** (`dim-storage` **676** iki OS'ta birebir:
macos-15 **417,06 s** / ubuntu-latest **518,33 s**; `arayüz` 66 dosya /
**1434** test, paket `index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB —
**değişmedi**; `masaüstü kabuk`, `windows güvenli depo`, `workers`,
`sözleşmeler`, `SBOM`, `kaynak arşivi`, `sürüm paketi manifesti`).

**Dürüst sınırlar.** (a) `consent_id` **NULL** bırakılırsa kapı susar: sütun
şemanın ilk sürümünden beri nullable ve **var olmayan** onay kimliği denetim
izi sayılır (0033/0034/0038/0039 ile aynı sınıf) — kapı **yanlış dayanak**
yazmayı durdurur, "dayanak yok" satırını durdurmaz; çekirdek her zaman onay
yazar. (b) v42 öncesi çapraz onaylı satırlar **silinmez**, sayılır (temizlik
ürün kararı); immutability kuralı bu satırların UPDATE ile onarılmasını da
engeller — silmek/temizlemek ayrı iştir. (c) Kapı onayın **içeriğini**
(amaç/maske/belge listesi) yeniden denetlemez; o kurallar 0039 ve çekirdekte.
(d) Tetikleyicileri anahtarı olan saldırgan düşürebilir (uygulama hatasına
karşı). (e) Arayüz değişmedi: çalıştırma listesi `consent_id`yi **göstermiyor**
— yasal dayanağın ekranda görünmesi ürün kararı; kapı duruktaki sınırı korur.
(f) Gerçek Tauri penceresi + görsel QA **insan bekliyor**.

## F08-T04q — emsal sürümünün çakışma işareti kanıtlanabilir olmalı (şema v44, 2026-11-16)

**Neden.** `identity_conflict`, Emsaller ekranında görünen tek çakışma iddiasıdır
(`screens/Precedents.tsx` → `versionConflictText` rozeti: "aynı künye, farklı
metin"). `conflict_with_version_id` iddianın karşı tarafını gösterir (F12-T06:
"kullanıcı iki metni karşılaştırabilsin"). Çekirdek ikisini birlikte ve doğru
yazar (`import_precedent_copy`: aynı künye + farklı SHA-256); ama **şemada
hiçbir kapı yoktu** — tablo FK'siz ve tetikleyicisizdi (v43 taraması: 54 tablo,
66 tetikleyici içinde `precedent_versions` yok). Doğrudan SQL yazan bir yol
rozeti **yalana** çevirebiliyordu.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v43; sonda
`scratch/dim-v44-probe/measure_v44.py`, log `measure_before.log`):**

1. A emsalinin sürümü **B emsalinin sürümüyle** çakıştığını iddia etti
   (`conflict_with_version_id='vB1'`, bayrak=1 → **KABUL**);
2. sürüm **kendini** çakıştığı sürüm gösterdi (`v-self → v-self`);
3. "aynı künye, **farklı metin**" rozeti **aynı** `body_hash` ile yazıldı —
   rozet ekranda yalan söylüyordu;
4. `identity_conflict=3` yazılabildi (her sıfır-dışı değer rozeti açar);
5. işaret dolu + bayrak 0 → okuyucu işareti taşır, rozet görünmez;
6. bayrak 1 + işaret NULL → rozetin karşılığı yok;
7. var olan sürümün işareti UPDATE ile B emsaline çevrildi;
8. `body_hash` UPDATE ile değiştirildi (iddia sonradan "doğru/yanlış" oldu);
9. sürüm **başka emsale taşındı** (`precedent_id` UPDATE): ona işaret eden
   satırlar çapraz kaldı.

**Yazılan (şema v44, `0044_precedent_conflict_guard.sql`).** İki tetikleyici:
`precedent_versions_conflict_guard_bi` — `identity_conflict` 0/1; rozet ile
işaret **birlikte** (`= 1` ⇔ `conflict_with_version_id IS NOT NULL`); işaret
varsa hedef kendisi olamaz, **var olan** hedef aynı emsalin sürümü ve **farklı**
`body_hash`te olmalı; **ters yön**: yeni satırın kimliğine işaret eden eski bir
satır varsa yeni satır da o iddiayı tutmalı ("hedef sonradan yazıldı" boşluğu
kapanır). `precedent_versions_conflict_guard_bu` — `UPDATE OF id, precedent_id,
body_hash, identity_conflict, conflict_with_version_id`: beş sütun **değişmez**
(mevcut kod sürümü yalnız INSERT eder; `verification_state` gibi diğer
sütunların güncellemesi takılmaz — v35 dersi). Ret
`RAISE(ABORT,'DIM_INTEGRITY:precedent_versions')` →
`StorageError::Integrity` → **`E_DB_INTEGRITY`**; `DIM_SCOPE`'tan ayrı önek,
ayrı kod (bu kural **dosya kapsamı** değil **emsal bütünlüğü**dür).
**Bilinçli gevşeklik** (0033/0034/0038/0039/0043 ile aynı): **var olmayan** hedef
kimliği ne kapıya takılır ne sayılır — kayıt denetim izidir. Bu tablo `case_id`
taşımadığı için `scope_violations()` alanı **eklenmez**; `GUARDED_TABLES`
**25 kalır** (türetim testi değişmedi).

**Testler (güncel kodla).** `precedent_import::tests` **+2**
(`a_precedent_conflict_marker_must_be_true`: kendi sürümü/çakışmasız/var olmayan
hedef meşru · çapraz emsal, kendini gösterme, aynı hash, bayrak 3, sessiz işaret,
karşılıksız rozet ve ters yön reddi; `a_precedent_version_conflict_marker_is_immutable`:
beş sütun UPDATE reddi, `verification_state` güncellemesi ve değişmeyen yazma
serbest), `upgrade_tests` **+1**
(`upgrade_guards_precedent_conflict_markers_written_before_v44`: v43 kasasındaki
yanlış rozetler **silinmez**, UPDATE ile onarılamaz, yeni yanlış yazma kapıda
durur, meşru çakışma hâlâ yazılır, kapsam taraması 0) → `dim-storage --lib`
**676 → 679** — **tam paket yerelde koşuldu: 679/679 yeşil, 442,88 s**. fmt (iki
manifest) temiz; `clippy -p dim-storage --all-targets -- -D warnings` temiz;
masaüstü `cargo check --all-targets` temiz.

**Kapı kanıtı.** **13/13 KIRMIZI, taban iki uçta 3/3 yeşilken** (betik
`scratch/dim-v44-probe/gates_v44.py`, log `gates_v44.log`; `try/finally` +
`os.utime` + sha256 birebir — `0044…sql
7d1735843239d3a8`, `error.rs 15b41e496cd324ab`): M1 BI bayrak 0/1 kuralı düşer ·
M2 BI rozet⇔işaret eşitliği düşer · M3 BI kendini-gösterme koşulu düşer · M4 BI
aynı-emsal koşulu düşer · M5 BI farklı-hash koşulu düşer · M6 BI ters yön
aynı-emsal düşer · M7 BI ters yön farklı-hash düşer · M8 BU emsal taşıması
serbest kalır · M9 BU metin özeti değişimi serbest kalır · M10 BU `UPDATE OF`
listesinden işaret düşer · M11 listesinden bayrak düşer · M12 listesinden id
düşer · M13 `DIM_INTEGRITY` öneki eşlenmez (E_DB_SQL'e düşer). **Dürüst (kendi
hatam):** ilk koşuda **M1 YEŞİL** kaldı — testte `flag=3 + işaret NULL` durumu
yoktu; eşitlik kuralı 3'ü "1 değil" sayıp NULL ile geçiriyordu (`0 = 0`). Test
`vA9b` ile güçlendirildi, **13/13 yeniden ölçüldü**; ilk koşunun 12 hükmü kanıt
sayılmadı.

**CI (ölçüldü).** Kod `bd126c3` (5 dosya, **+316/−1**) → run `43860857265` **10/10 YEŞİL**
(`dim-storage` **679** iki OS'ta birebir: macos-15 **401,80 s** / ubuntu-latest **759,41 s**);
`arayüz` 66 dosya / **1434** test, paket `index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB —
**değişmedi**; `masaüstü kabuk`, `windows güvenli depo`, `workers`, `sözleşmeler`, `SBOM`,
`kaynak arşivi`, `sürüm paketi manifesti`.

**Dürüst sınırlar.** (a) **Var olmayan** hedef kimliği serbesttir: rozet
karşılıksız kalabilir (denetim izi; 0033/0038/0039/0043 ile aynı sınıf) — kapı
*kanıtlanabilir* yalanı durdurur, "karşı taraf bilinmiyor" satırını durdurmaz.
(b) v44 öncesi yanlış rozetler **silinmez, sayılmaz** (tablo kapsam taramasında
yok) ve immutability onların UPDATE ile onarılmasını engeller — temizlik/onarım
ürün kararı. (c) Kapı metnin **içeriğini** yeniden hesaplamaz; `body_hash`
sütunlarının farkını denetler — yazıcı `full_text` ile tutarsız bir `body_hash`
yazarsa iddia yine "tutarlı" görünür (hash'i yeniden üretmek SQLite'ta mümkün
değil; çekirdek doğru yazar). (d) Emsal sürümü silme yolu yok; olsaydı işaret
sahipsiz kalırdı (bugün yalnız emsal silinince `CASCADE` ile birlikte gider).
(e) Tetikleyicileri anahtarı olan saldırgan düşürebilir (uygulama hatasına
karşı). (f) Arayüz değişmedi; çakışan sürümlerin **yan yana karşılaştırma**
ekranı hâlâ yok (F12-T06 yalnız işaretler) — gerçek pencere + görsel QA
**insan bekliyor**.

## F08-T04r — gönderim sicilinin dönüşüm bağı kanıtlanabilir olmalı (şema v45, 2026-11-16)

**Neden.** `transfer_segments.transform_manifest_id`, satırın «hangi maske tanımıyla»
gönderildiğini söyler. Okuyucu iki yerde gerçek: `list_transfer_segments` kimliği
`outbound_transforms`la sol birleştirip ekranda **türü** (`identity` / `mask_terms`)
ve terim sayısını gösterir (Ai.tsx gönderim sicili); `sent_segments` ise alıntı
doğrulamasında dönüşüm kimliğine bakıp **kimlik mi aralık haritası mı** kullanılacağına
karar verir — yani yanlış bağ, doğrulanan alıntının özgün konumunu değiştirir.
`transfer_consents.outbound_transform_id` ise onayın **maske tanımıdır** (0030:
«maske onayın parçasıdır»): `start_ai_run` maske eşitliğini bu satırın
`settings_hash`i üzerinden kurar. 0027 tabloyu tanımladı, 0034/0038 sicilin
**sürümünü ve segmentini** kapıladı — ama iki dönüşüm bağı **hiçbir tetikleyici
taşımıyordu** (v44 taraması: gönderim/aktarım zincirinde FK'siz ve kapısız kalan
tek okuyucu-yazıcı bağ çifti).

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v44; sonda
`~/.hermes/cache/scratch/dim-v45-probe/measure_v45.py`, log `measure_before.log`):**
(1) `run1` (onayı **A** maskesi) siciline **B** maskesinin dönüşümü yazıldı;
(2) **var olmayan** dönüşüm kimliği yazıldı (ekran «unknown» der);
(3) maskeli satır `transform_manifest_id=NULL` ile yazıldı — okuyucu onu **kimlik**
sayar, alıntı doğrulaması maskeli metni özgün sanar; (4) `kind='identity'` dönüşüm
satırı maskeli sicile bağlandı; (5) yazılmış satırın dönüşümü UPDATE ile B maskesine
çevrildi; (6) bağ UPDATE ile **silindi** (NULL); (7) sicil satırı **başka
çalıştırmaya** taşındı (onay bağlamı değişti); (8) onayın maske bağı UPDATE ile
tA→tB çevrildi — maske kapısının karşılaştırdığı `settings_hash` `h-a`→`h-b` oldu,
yani **onaylanan maske A iken B ile gönderim kapıyı geçer**; (9) onayın bağı
NULL'a çekildi (yasal kayıt silindi); (10) onaya **var olmayan** dönüşüm kimliği
yazıldı; (11) onaya `kind='identity'` dönüşüm bağlandı. v44 taramasında bu sınıfı
sayan alan yok: 4 uydurma/çapraz sicil satırı ve 2 bozuk onay bağı hiçbir yerde
görünmüyordu.

**Yazılan (şema v45, `0045_transfer_transform_binding_guard.sql`, 4 tetikleyici).**
`transfer_segments` BI — dolu `transform_manifest_id` **var olan** ve
`kind='mask_terms'` dönüşümü göstermeli (kimlik `NULL` ile temsil edilir; çekirdek
`identity` satırı yazmaz); çalıştırmanın onayı **çözülebiliyorsa** bağ onayla aynı
olmalı: 1+ terimli onayda sicil satırı **aynı** kimliği göstermeli, 0 terimli onayda
`NULL` olmalı ("onaylandı A, gönderildi B" yazılamaz). `transfer_segments` BU —
`id`, `ai_run_id`, `transform_manifest_id` **değişmez** (satır hangi çalıştırmaya
aitse ve hangi maskeyle gönderildiyse öyle kalır; diğer sütunlar takılmaz — v35
dersi). `transfer_consents` BI — dolu bağ var olan ve `kind='mask_terms'` dönüşümü
göstermeli; `NULL` (0030 öncesi onay) serbesttir, "bağ bilinmiyor" meşru durumdur.
`transfer_consents` BU — `outbound_transform_id` **değişmez**: onaylanan maske
tanımı sonradan ne genişletilebilir ne daraltılabilir ne silinebilir (yeni maske
**yeni onay** ister). Ret `RAISE(ABORT,'DIM_INTEGRITY:<tablo>')` →
`StorageError::Integrity` → **`E_DB_INTEGRITY`** (0044 ile aynı önek). Tablolar
`case_id` taşımadığı için tarama alanı **eklenmedi**; `GUARDED_TABLES` **25 kalır**.

**Testler.** `ai::tests` **+2** (`outbound_record_can_only_claim_the_mask_the_consent_approved`:
onaylanandan farklı var olan maske, uydurma kimlik, NULL "kimlik" iddiası,
`identity` türü, onaysız çalıştırmada uydurma/kimlik türü, UPDATE ile bağ çevirme /
silme / çalıştırma taşıma reddi, aynı değeri yeniden yazma serbest;
`consent_mask_binding_cannot_be_rewritten_or_invented`: bağ UPDATE ile
çevrilemez/silinemez, uydurma/`identity` bağ reddi, iptal damgası serbest),
`upgrade_tests` **+1**
(`upgrade_guards_transfer_transform_bindings_written_before_v45`: v44 kasasındaki
yanlış bağlar **silinmez**, UPDATE ile onarılamaz, yeni yanlış yazma kapıda durur,
meşru yazma çalışır, kapsam taraması 0) → `dim-storage --lib` **679 → 682**.
**Dürüst (fixture düzeltmesi).** v45'ten önce üç test, **maskesiz onayın**
çalıştırmasına maskeli sicil yazıyordu (üretimde `start_ai_run` bunu reddeder);
fixture'lar gerçek sözleşmeye çekildi — `preview_with_mask_matches_what_is_sent`
ve `masked_outbound_records_transform_and_original_range_map` onayı maske
terimleriyle açıyor, `ai_candidate` fixture'ı maskeli gönderim için **ayrı onay +
ayrı çalıştırma** taşıyor; `pre_v30_consent_without_mask_binding_is_refused` NULL
bağı artık **doğrudan INSERT** ile kuruyor (v45 bağı UPDATE'e kapatır).

**Kapı kanıtı.** **13/13 KIRMIZI, taban iki uçta 3/3 yeşilken** (betik
`~/.hermes/cache/scratch/dim-v45-probe/gates_v45.py`, log `gates_v45_final.log`;
`os.utime` + sha256 birebir — `0045…sql
f0e190b7675ab2172da9ef214ffad8e426a59a679ed1e10a43e258b86d5ddfd5`): M1 BI
varlık+kind kapısı düşer · M2 BI `kind='mask_terms'` düşer · M3 BI onay-eşitliği
dalı düşer · M4 BI 0-terim NULL zorunluluğu düşer · M5 BI 1+ terim eşitliği düşer ·
M6 BU değişmezlik tetikleyicisi etkisiz · M7 BU `ai_run_id`yi korumaz · M8 BU
`<>` ile karşılaştırır (NULL kaçar) · M9 onay BI varlık kapısı düşer · M10 onay BI
kind düşer · M11 onay BU etkisiz · M12 onay BU `<>` (NULL'a çekme kaçar) · M13 ret
öneki `DIM_SCOPE` olur (`E_DB_SCOPE`). **Dürüst (kendi hatam):** ilk koşuda
**M1/M2 YEŞİL** kaldı — testte **onayı çözülemeyen** çalıştırma yoktu, varlık/kind
dalını onay-eşitliği dalı maskeliyordu; **M6** mutasyonu BU `WHEN`ının yalnız ilk
birleşimini düşürüyordu (etkisiz), **M13** deseni iki kez geçiyordu. Test
(onaysız çalıştırma `run-serbest`) ve betik desenleri düzeltildi; ikinci koşuda
**13/13**, yorum düzeltmesinden sonra son hâl üzerinde **yeniden 13/13** ölçüldü.
İlk koşunun 9 hükmü kanıt sayılmadı.

**CI (ölçüldü).** Kod `1572ae3` (5 dosya, **+629/−12**) → run `43878690384`
**10/10 YEŞİL** (`dim-storage` **682** iki OS'ta birebir: macos-15 **354,34 s** /
ubuntu-latest **563,94 s**); `arayüz` 66 dosya / **1434** test, paket
`index-DHBc7fVy.js` 694,53 kB / gzip 185,32 kB — **değişmedi**; `masaüstü kabuk`
51/0/5 + gerçek worker 5/0/0, `windows güvenli depo` 28, `workers`, `sözleşmeler`,
`SBOM`, `kaynak arşivi`, `sürüm paketi manifesti`.

**Dürüst sınırlar.** (a) `outbound_transforms` satırları içerik adreslidir ve
hiçbir yol silmez; bu yüzden "var olmayan kimlik" burada denetim izi değil
**kanıtlanabilir yalan** sayıldı ve fail-closed kapıya takıldı (0033/0038/0043'ün
gevşekliğinden bilinçli ayrılık). (b) Onay zinciri **çözülemiyorsa** (0043'ün izin
verdiği uydurma `consent_id`) eşitlik kuralı boşta kalır; varlık/kind kapısı yine
çalışır. (c) Bu tablolar `case_id` taşımaz; v45 öncesi yanlış bağlar **silinmez ve
sayılmaz**, immutability onların UPDATE ile onarılmasını da engeller — temizlik
ürün kararı. (d) Sicil **metni** (`outbound_text`/`outbound_hash`) bu dilimin
dışında: UPDATE ile değiştirilebilir kalır (sıradaki aday). (e) Maskenin
**içeriği** yeniden hesaplanmaz; kapı `body_hash`i değil bağ **kimliğini** denetler
(SQLite'ta SHA-256 yok; çekirdek doğru yazar). (f) Tetikleyicileri anahtarı olan
saldırgan düşürebilir (uygulama hatasına karşı). (g) Arayüz değişmedi; ekran zaten
türü/terim sayısını gösteriyor — gerçek pencere + görsel QA **insan bekliyor**.
