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
| F08-T06 | ✅ | not ↔ kaynak alıntı bağı (`source_anchor_ids`) |

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
