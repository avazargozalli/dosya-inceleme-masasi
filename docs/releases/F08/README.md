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
