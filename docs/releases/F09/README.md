# F09 — AI sağlayıcı çerçevesi

- **Sürüm etiketi:** `v0.8.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F09-T01 tek `Provider` sözleşmesi | ✅ | `crates/providers` |
| F09-T02 `OffProvider` | ✅ | her istek `E_AI_OFF` |
| F09-T03 aktarım onayı | ✅ | `transfer_consents`; onay **belge kümesine** bağlı (boş seçim, başka dosyanın belgesi ve silinmiş belge reddedilir) — ölçüldü 2026-10-28, aşağıdaki "Aktarım kapsamı" bölümü |
| F09-T04 çalıştırma sicili | ✅ | `ai_runs`, `ai_context_items`, `transfer_receipts` — **artık gerçek yolda yazılıyor**: gönderim öncesi belge/sürüm/bayt sicili, gönderim sonrası hedef + çıkan baytların sha256'sı (`ai_run_execute`). Önceden bu iki tablo yalnız testte doluyordu |
| F09-T05 onaysız çalıştırma reddi | ✅ | `E_AI_CONSENT`: onay yok / iptal / süresi geçmiş / **belge seçimi boş** / seçili belge silinmiş (7 test) |
| F09-T06 yerel model indirme/doğrulama | ⬜ | — |
| F09-T07 API profili | ✅ | `register_api_profile`: https zorunlu, anahtar **saklanmaz** (yalnız `DIM_API_KEY`), idempotent |
| F09-T08 gönderim önizlemesi (tam) | ✅ | `consent_preview`: belge, segment, bayt, 200 karakter örnek ve **kapsam dışı belgeler + nedeni**; gönderim yolu aynı çözümleyiciyi kullanır. Önceden SQL hatası veriyordu (aşağıdaki bölüm) |
| F09-T09 onay yenileme + iptal kapısı | 🚧 **arayüz kapsamı** | Ölçüldü 2026-11-05: dosya değişiminde onay düşer, **amaç** değişiminde onay düşer, iptali uçuşta olan onayla **yeni gönderim başlamaz**; görev türü onayın amacından çözülür. **Onay envanteri** eklendi: aynı dosya+amaç yeniden onaylanınca önceki yetki kapanır (denetimli), bütün onaylar görünür ve etkin olan iptal edilebilir (aşağıdaki bölüm). Çekirdek `start_ai_run` onayın `provider_snapshot`ını çalıştırılan profille karşılaştırır (F11-T04). **Kalan:** sağlayıcı değişiminde yenileme ekranda ulaşılabilir değil (sağlayıcı seçici yok, rota değişiminde ekran yeniden kuruluyor) |

## Dürüst durum

AI **varsayılan olarak kapalıdır**. Kayıtlı tek profil `off` ise arayüz `E_AI_OFF`
gösterir ve çalıştırma başlatılamaz. Gerçek sağlayıcı bağlanmadı; `ai_run_start`
sağlayıcı `off` değilse kayıt açar, ancak gerçek yürütücü F11 kapsamındadır.

## Aktarım kapsamı ve sicil (2026-10-28 — kusur #69/#70/#71)

Yukarıdaki `✅` işaretleri **kanıtsızdı**; bu tur ölçüldü ve üç kusur bulundu.

- **#69 (kritik, işlevsiz dikey dilim): `consent_preview` ve `context_segments`
  çalışmıyordu.** SQL `text_segments.text` ve `text_segments.segment_index`
  sütunlarını okuyordu; tabloda bu adlar **yok** (`raw_text`, sıralama için
  `rowid`). Ölçüm (ilk kırmızı test): `SqlInputError { msg: "no such column:
  s.text" }`. Yani **"Gönderilecek içeriği göster"** düğmesi her zaman hata
  veriyor, `ai_run_execute` hiçbir zaman gönderim yapamıyordu. Testler bu iki
  işlevi ya hiç çağırmıyordu ya da boş/olmayan belgeyle çağırıp SQL'e
  girmeden dönüyordu — `✅` bu yüzden yanlıştı.
- **#70 (izin sessizce genişliyordu): onay belge kümesine bağlı değildi.**
  `transfer_consents.selected_document_ids` yazılıyor ama **hiçbir yerde
  okunmuyordu**; `start_ai_run` yalnız `case_scope` denetliyordu. Arayüz de
  onayı **boş liste** ile açıp gönderimde **dosyanın bütün belgelerini**
  geçiriyordu. Şartname §"AI aktarımı" "uygulama mevcut izni kendiliğinden
  genişletemez" der → kullanıcının onaylamadığı belge gönderilebiliyordu.
- **#71 (sicil beyan edilmiş, yazılmıyordu):** modül başlığı "gönderilen içerik
  `ai_context_items` ve `transfer_receipts` ile denetlenebilir" diyordu; iki
  yazma işlevinin **üretimde tek çağrı yeri yoktu** (yalnız test). Gerçek
  gönderimden sonra "hangi sürümün kaç baytı çıktı" kaydı tutulmuyordu.
- **Aynı sınıftan önizleme/gönderim ayrışması:** önizleme `documents.current_version_id`
  ile, gönderim ise "belgenin **tüm** sürümleri içinde en yeni koşu" ile
  çalışıyordu → güncel sürüm ayrıştırılmamışsa önizleme "0 bayt" derken
  gönderim **eski sürümün metnini** alabiliyordu.

**Düzeltme**

- Tek çözümleyici `resolve_transfer_scope`: belgenin **güncel** sürümü + o
  sürümün **sayfa üreten en yeni** koşusu; `raw_text`/`rowid` (gerçek sütunlar).
  Önizleme ve gönderim **aynı** işlevi kullanır → ikisi ayrışamaz.
- Kapsam **onaydan** çözülür: `consent_scope_segments(case_id, consent_id)`;
  Tauri komutu artık `document_ids` **almaz** (çağıran kapsamı genişletemez).
- Onay **sağlayıcıya** da bağlı: `start_ai_run` profilün `adapter_id@policy_version`
  değerini onayın `provider_snapshot`'ı ile karşılaştırır; başka uç nokta/model
  `E_AI_CONSENT`, hiç olmayan profil `E_AI_PROVIDER`.
- Onay oluşturma doğrulaması: boş liste, başka dosyanın belgesi ve silinmiş
  belge `E_AI_CONSENT`; yinelenen kimlik tekilleştirilir. `start_ai_run` ayrıca
  onaydan sonra silinmiş belgeyi yakalar (izin daralmışsa çalıştırma açılmaz).
- Gönderim öncesi `ai_context_items` (belge sürümü + bayt), gönderim sonrası
  `transfer_receipts` (hedef **yalnız ana makine + model**, çıkan baytların
  sha256'sı). Makbuz yazılamazsa çalıştırma `succeeded` değil **`partial`**
  kapanır — "gönderildi ama sicili yok" sessizce başarı sayılmaz.
- Sicil belge metnini **saklamaz**: canary testi `audit_log`, `ai_runs`,
  `ai_context_items`, `transfer_receipts`, `transfer_consents` tablolarını
  `PRAGMA table_info` ile sütun sütun tarar (pozitif kontrol: metin
  `text_segments`te bulunur).
- Arayüz: onay **somut belge listesiyle** açılır, kartta "Onay kapsamı: N belge ·
  gönderilecek: M belge, K segment, B bayt" ve kapsam dışı belgelerin **nedeni**
  yazılır (`lib/transferScope.ts`).

**Kanıt (ölçüldü):** `ai` testleri **9 → 17** (8 yeni: önizleme/gönderim
birebirliği, güncel sürüm + sayfa üreten koşu, kapsam yalnız onaydan, sağlayıcı
kapsamı, silinmiş belge kapısı, boş seçim, başka dosyanın belgesi, sicilde metin yok);
`cargo test -p dim-storage --lib` **380/380**; tauri lib **31 geçti / 5 ignored**;
`vitest` **426/426** (33 dosya, +3 `transferScope`); `tsc --noEmit` temiz;
clippy `dim-storage` + tauri `--all-targets -D warnings` temiz.

**Dürüst sınırlar:** gerçek uçtan uca gönderim **denenmedi** — `ApiProvider`
`https://` zorunlu kılıyor, yerel TLS sunucusu yok; gönderim yolu derleme +
çekirdek testleriyle doğrulandı, ağ çağrısıyla değil. Sağlayıcı yanıtının
şeması/aday doğrulaması bu turun dışında. Windows denenmedi; Tauri penceresi
açılmadı → yeni kapsam satırının görsel QA'si yok. Prompt injection ve model
davranışı ölçülmedi (F15-T08 açık kalıyor) — bu turda ölçülen **veri kapsamı**
ve **sicil** tarafıdır.

## Onay amacı ve iptal uçuşu (2026-11-05, F09-T09)

Şartname F09-T09: *"Sağlayıcı/dosya/kapsam değiştiğinde onayı yenile; onay
iptalini yeni gönderimden önce uygula."* Arayüzde onay **dosyaya** bağlıydı ve
dosya değişiminde düşüyordu; **amaç** ve **iptal** tarafı bağlı değildi.

**Ölçülen kusurlar (ham koşu: 5 failed | 51 passed / 56):**

1. **İptali uçuşta olan onayla yeni gönderim başlıyordu (en ağırı).**
   `revokeBusy` yalnız "İptal et" düğmesini kapatıyordu; "Çalıştır" açıktı.
   Kullanıcı iptal ederken çalıştırırsa içerik, iptal çekirdekte uygulanmadan
   dışarı çıkabiliyordu (`ai_consent_revoke` ile `ai_run_execute` ayrı iş
   parçacıklarında yarışır; çekirdek kilidi yalnız **başlamış** komutu serileştirir).
   Kırmızı: iptal askıdayken "Çalıştır" `disabled` beklenen `true`, gelen `false`
   ve tıklama `aiRunExecute` çağırıyor.
2. **Görev türü onayın amacını yok sayıyordu.** `start()` her koşuda
   `"summarize"` gönderiyordu; "İddia çıkarımı" için onay alınmışken model
   "Görev: summarize" görüyor, `ai_runs.task_type` onayın amacıyla çelişiyordu.
   Kırmızı: onay amacı `iddia_cikarimi` iken beklenen `extract_claims`, gelen
   `summarize`.
3. **Amaç seçimi onaydan sonra değiştirilebiliyordu.** Onay eski amaca bağlı
   kalıyor, ekran yeni amacın onaylı olduğunu düşündürüyordu (şartname: kapsam
   değiştiğinde onay yenilenir). Kırmızı: amaç değiştikten sonra "İptal et"
   düğmesi ve açık "Çalıştır" ekranda kalıyor.
4. **Onay satırı amacı yazmıyordu.** Hangi amacın onaylandığı görünmüyordu;
   seçici ile onay arasındaki çelişki ekranda okunamıyordu.

**Düzeltme (`Ai.tsx` +63/−7, `i18n.ts` +1):** `revokeTarget` durumu —
iptal **hedefe** bağlandı; "Çalıştır" yalnız `revokeBusy && revokeTarget ===
consent.id` iken kapalı (bu sırada alınmış **yeni** onay engellenmez, çünkü
kimliği farklıdır); dosya değişimi hedefi devreder. Görev türü `consent.purpose`
üzerinden `PURPOSE_TASKS` ile çözülür (`ozet → summarize`, `olay_cikarimi →
extract_events`, `iddia_cikarimi → extract_claims`; bilinmeyen amaç aynen
geçer). Amaç seçicisi değişince onay düşer (yeni gönderim yeniden onay ister).
Onay satırı `Amaç: …` ile onaylanan amacı yazar.

**Kapı kanıtı (5/5 tek kapı mutasyonu kırmızı; geri yükleme sha256
`223c2e6ed96bd82bbaf567b14eedec9af3f4a7275a6a95e7029c9b20eda74b47` birebir):**
M1 çalıştırma kapısı kaldırıldı (1), M2 kapı hedefsiz `revokeBusy` (2 — yeni
onayın çalıştırmasını da kapatır), M3 görev türü `summarize` sabiti (3), M4 amaç
değişimi onayı düşürmez (1), M5 onay satırı amacı yazmaz (1). **Dürüst:** 6
testin 5'i düzeltme öncesinde kırmızı; "iptal uçuşu yeni onayın çalıştırmasını
kapatmaz" testi düzeltme öncesinde de **yeşildi** (sabitleme) ve hedefsiz kapı
mutasyonunu (M2) yakalayan tek testtir.

**Ölçüm:** `tsc --noEmit` temiz · `vitest` **1105/1105** (60 dosya; 1099 → +6) ·
`pnpm build` temiz (**587,02 kB**, `index-Bc4Z-ZRT.js`, gzip **158,43 kB**; CSS
`index-CPW4QqlF.css` 7,28 kB). Rust değişmedi.

**CI (ölçüldü):** `6f0a067` → koşu **`41951851154` 7/7 YEŞİL** (ilk koşu;
`arayüz` günlüğü **60 dosya / 1105 test** ve **aynı paket** `index-Bc4Z-ZRT.js`
587,02 kB / gzip 158,43 kB → yerel/CI **birebir**).

**Dürüst sınırlar:** jsdom **tarayıcı değil** — gerçek Tauri penceresi, gerçek
işaretçi zamanlaması ve **görsel QA yok**; `api` mock'lu olduğu için
`ai_consent_revoke` / `ai_run_execute` SQL'i ve Rust **koşmaz**. Kapı **arayüz
düzeyinde** ve **durum tabanlıdır**: aynı tikte (render öncesi) iki farklı
düğmeye basılırsa `disabled` yetişmez — ref tabanlı ikinci kapı **eklenmedi**
(ölçülemedi; yedekli kapı bırakılmadı). Görev türü eşlemesi çekirdeğe yeni bir
sözleşme getirmez: `task_type` serbest metindir ve model istemi "Görev: …"
satırıyla beslenir; `output_schema_version` hâlâ `candidate.v1`.

**Açık kayıt:** (a) **sağlayıcı değişiminde yenileme** ekranda ulaşılabilir
değil (sağlayıcı seçici yok; ekran rota değişiminde yeniden kurulur) ve çekirdek
`consent_scope_segments` onayın `provider_snapshot`ını çalıştırılan sağlayıcıyla
**karşılaştırmaz** — doğrudan IPC ile eski onay yeni sağlayıcıya götürülebilir
(ölçülmedi, kapatılmadı); (b) yeni onay alınırken eski onay **iptal edilmez**,
çekirdekte etkin kalır (arayüz yalnız sonuncuyu gösterir; yeni gönderim onunla
koşmaz) — onay envanteri ekranı yok; (c) iptal başarısı önizleme kartını
temizlemez (kart belge kapsamı okumasıdır, yetki iddiası taşımaz).

> **Güncelleme (2026-11-05, sonraki tur):** (b) aşağıdaki **"Onay envanteri ve
> süpersede"** bölümüyle kapandı. (a)'nın çekirdek karşılaştırması F11-T04'te
> `start_ai_run` içine eklendi; ekran tarafı (sağlayıcı seçici) açık kalıyor.
> `consent_scope_segments` yalnız **kapsam** çözümleyicisidir, yetki kapısı
> değildir — yetki kapısı `start_ai_run`dır.

## Onay envanteri ve süpersede (2026-11-05, F09-T09)

Önceki turun açık kaydı (b): *"yeni onay alınırken eski onay iptal edilmez,
çekirdekte etkin kalır (arayüz yalnız sonuncuyu gösterir) — onay envanteri
ekranı yok."* Koda karşı okundu: çekirdekte `transfer_consents` için **liste
okuması yoktu**; arayüz onayı yalnız kendi `useState`inde tutuyordu. Sonuç:
rota değişiminde ya da uygulama yeniden açıldığında **etkin bir aktarım yetkisi
ne görünür ne iptal edilebilirdi**. Aynı amaç için ikinci kez onay verilince de
eski kayıt etkin kalıyordu; hangisinin güncel kapsam olduğu ekrandan okunamıyordu.

**Ölçülen kusurlar (ham koşu: 10 failed | 56 passed / 66):**

1. **Görünmez etkin yetki (en ağırı).** `list_transfer_consents` ve
   `ai_consent_list` yoktu. Onay ekranı kapandığında/yeniden açıldığında
   kullanıcı, parola verdiği belge kümesi için verdiği yetkiyi göremiyor ve
   iptal edemiyordu. Kırmızı: "Aktarım onayları" kartı ve satırları yok.
2. **Aynı amaç için iki etkin yetki.** `create_transfer_consent` yalnız INSERT
   ediyordu; aynı dosya+amaç yeniden onaylanınca önceki kayıt etkin kalıyordu.
   Kırmızı: ikinci onaydan sonra `consent_is_active(eski)` hâlâ `true`.
3. **Durum arayüz saatine kalacaktı.** Envanter satırı "süresi geçti" kararını
   tarayıcı saatinden vermemeli; çekirdek `consent_active_at` tek kuralı
   `active` alanıyla taşır (tasarım kararı — kusur değil, ayrışmayı önler).

**Düzeltme:**

- `create_transfer_consent` tek `unchecked_transaction` içinde: INSERT + **aynı
  dosya ve amaç** için etkin önceki onayları `revoked_at` ile kapatır ve her
  biri için `transfer_consent/supersede` denetim kaydı yazar
  (`superseded_by`). Farklı amaç veya farklı dosya ayrı yetkidir, dokunulmaz;
  zaten iptal edilmiş ya da süresi geçmiş kayıt yeniden işaretlenmez (envanterde
  "iptal edildi" ile "süresi geçti" ayrımı korunur).
- `list_transfer_consents(case_id)` → `TransferConsentListRow` (kayıt + `active`),
  dosyaya kapsanmış, en yeni önce. `active`, `consent_is_active` ile **aynı**
  `consent_active_at` fonksiyonundan gelir.
- Tauri komutu `ai_consent_list` + `api.aiConsentList`.
- Ai ekranında **"Aktarım onayları"** kartı: amaç, sağlayıcı özeti, belge sayısı,
  onay zamanı, bitiş ve durum; iptal **yalnız etkin** satırda. Liste dosya
  değişiminde, onaydan ve iptalden sonra çekirdekten tazelenir; iyimser
  ekleme/silme yok. `consentListSeq` ile **son başlayan okuma kazanır**; iptal
  uçuşunda satır düğmeleri kapalıdır; iptal düşerse satır etkin kalır ve hata
  görünür. Sağlayıcı profili değişmişse onay çalıştırmada `E_AI_CONSENT` verir —
  kart bunu sınır notu olarak yazar.

**Testler:** `crates/storage/src/ai.rs` **23 → 28** (+5: süpersede, farklı
amaç/dosya dokunulmaz, varış yeri değişse de süpersede, yalnız canlı kayıtlar,
envanter kapsam/sıra/durum). `Ai.test.tsx` **56 → 66** (+10). **Ham koşu
(düzeltme öncesi bileşen, nihai 66 test): 10 failed | 56 passed** — onu da yeni
envanter testleri; eski 56 test yeşil kaldı (regresyon yok).

**Kapı kanıtı 17/17** (her koşu tek kapı, dosya yedekten geri yüklendi;
`ai.rs` sha256 `f85e41c37580d4771ed73eef6b1ad6849e543103596b6ae35dbcfe6c84c8349c`,
`Ai.tsx` `673f84281d70c3e5756f49aa4be96a23bd96016c7b8ba2a783076dbf93c312fa`
birebir): UI M1 başarı sıra kapısı (2), M2 yalnız etkin satırda iptal (2), M3
satır uçuş kilidi (2), M4 Yenile kilidi (1), M5 onay sonrası tazeleme (2), M6
iptal sonrası dosya kapısı (1), M7 iptal sonrası tazeleme (1), M8 okuma hatası
(1), M9 iptal etiketi (2), M10 boş liste satırı (2); çekirdek C1 süpersede
kapısı (2), C2 amaç filtresi (2), C3 dosya filtresi (2), C4 iptal edilmişi
kapatmama (1), C5 süresi geçmişi kapatmama (1), C6 yeni onayın kendisini koruma
(17 — kendi onayını da kapatınca zincir), C7 süre kuralı (2). **Dürüst:** C2/C3
beklenen 1 yerine 2 test düşürdü (envanter testi de etkileniyor); C6 geniş
kırılım verdi.

**Ölçüm:** `cargo test --workspace --all-targets` **tümü yeşil** (dim-storage lib
**397**, +5; dim-vault 17+13+3) · `cargo clippy --workspace --all-targets
-D warnings` temiz · masaüstü clippy temiz (artımlı 3,7 sn; **dürüst:** ilk koşu
`E0432` ile düştü — `TransferConsentListRow` `dim_storage` kökünden dışa
aktarılmamıştı, `lib.rs` dışa aktarımı eklendi) · `cargo fmt` iki manifest temiz ·
`tsc --noEmit` temiz · `vitest` **1130/1130** (60 dosya; 1120 → +10) · `pnpm
build` temiz (**591,83 kB**, `index-B5cLgRXx.js`, gzip **159,57 kB**; CSS
`index-CPW4QqlF.css` 7,28 kB).

**Dürüst sınırlar:** jsdom tarayıcı değil — gerçek Tauri penceresi, gerçek
işaretçi ve görsel QA yok. `ai_consent_list` **komut olarak koşulmadı**
(`State<AppState>` kuran test altyapısı yok; `commands_tests.rs` CI'da koşmuyor);
kablolama derleme + clippy ile, davranış `list_transfer_consents` düzeyinde
gerçek SQLite ile doğrulandı. Envanter `active` alanı yalnız iptal/süre durumunu
taşır; sağlayıcı profili değişince kayıt "etkin" görünür ama çalıştırma
`E_AI_CONSENT` verir (kart bunu yazar; ayrı "sağlayıcı değişti" durumu yok).
`expires_at` arayüzden hiç yazılmıyor (komut `None` geçer) — "süresi geçti"
satırı yalnız doğrudan IPC/test ile üretilebilir. Süpersede **varış yerine göre
değil, dosya+amaç** ile çalışır: aynı amaç için son onay güncel kapsamdır. Liste
sınırsızdır (sayfalama yok); çok sayıda onayı olan dosyada kart büyür. Görsel QA
**insan bekliyor**.

**Açık kayıt:** (a) sağlayıcı değişiminde yenileme ekranda ulaşılabilir değil
(sağlayıcı seçici yok; ekran rota değişiminde yeniden kurulur) — **2026-11-05
turunda kısmen kapandı:** envanter satırı artık «sağlayıcı değişti» yazar ve
bayat onayla "Çalıştır" kapalıdır (aşağıdaki bölüm); profil **seçici** hâlâ yok;
(b) envanter
satırından **iptal** var, tek tek "yenile" düğmesi yok — yenileme yeni onay
almaktan geçer; (c) onay envanteri denetim kaydı arayüzde gösterilmiyor.

## Sağlayıcı değişimi onayı geçersiz kılar (2026-11-05, F09-T09)

Önceki turun açık kaydı (a) + "envanter `active` alanı yalnız iptal/süre
durumunu taşır" sınırı. Koda karşı okundu; dört ölçülmüş kusur çıktı.

**Ölçülen kusurlar:**

1. **Envanter "etkin" diyordu ama onay yürütülemezdi (en ağırı).**
   `list_transfer_consents` yalnız `consent_active_at`e bakıyordu. Profilin uç
   noktası/modeli değişince kayıt "etkin" kalıyor, `start_ai_run` ise
   `E_AI_CONSENT` veriyordu. Kartın notu "çalıştırmada reddedilir" diyordu —
   yani **hangi satırın** ölü olduğunu söylemeden kullanıcıya tahmin ettiriyordu.
2. **Hata kodu olguyu yanlış adlandırıyordu.** Sağlayıcı değişiminde dönen kod
   `E_AI_CONSENT` ("geçerli aktarım onayı yok"); oysa onay **vardı**, varış
   yeri değişmişti. Ekran "onay yok" diyor, yenileme gerektiğini söylemiyordu.
3. **Çalıştırma ilk off-dışı profille koşuyordu.** `providers.find(p =>
   p.providerType !== "off")`; onay başka bir profile bağlıysa (birden çok
   profil) çekirdek haklı olarak reddediyor, kullanıcı nedenini göremiyordu.
4. **Profilin varış yeri görünmüyordu.** `ProviderProfileRow` uç nokta/model
   özetini taşımıyordu; "sağlayıcı değişti" iddiası ekranda somutlaşamıyordu.

**Düzeltme:**

- `TransferConsentListRow.provider_current`: onayın `provider_snapshot`ı
  yürürlükteki (`off` dışı) **herhangi** bir profilden türetilebiliyor mu?
  Kural `start_ai_run` ile aynı (özet eşitliği); liste başına **bir kez**
  okunur (satır başına sorgu aynı listede farklı anları karıştırırdı).
  `active` (iptal/süre) ile karışmaz: iptal edilmemiş ama yürütülemez onay
  "kayıt yürürlükte, varış yeri değişti" olarak yazılır.
- `ProviderProfileRow.provider_snapshot`: aynı `provider_snapshot()`
  fonksiyonundan türetilir (uç nokta + model parmak izi dahil).
- `start_ai_run` sağlayıcı değişiminde **`E_AI_PROVIDER_CHANGED`** döner.
- `Ai` ekranı: dördüncü durum rozeti «sağlayıcı değişti» (warn tonu), "Varış
  yeri" satırı, bayat onayla "Çalıştır" **kapalı** + `ai.consentStale` uyarısı,
  çalıştırma onayın **kendi** profiliyle koşar, `E_AI_PROVIDER_CHANGED`
  sonrası künye ve envanter tazelenir (ekran kendi kendini düzeltir).
- Testler: `ai.rs` **28 → 31** (+3), `Ai.test.tsx` **66 → 70** (+4).
- **Var olan üç test değişti (dürüst):** `api_endpoint_change_invalidates_…`,
  `api_model_change_invalidates_…`, `consent_is_provider_scoped`
  `E_AI_CONSENT` bekliyordu; kod artık özel kodu döndürüyor. **Davranış aynı**
  (ret), yalnız kod adı değişti — bunlar kusuru sabitleyen değil, genel kodu
  pinleyen testlerdi.
- **Test fikstürü düzeltmesi (dürüst):** `Ai.test.tsx` varsayılan
  `aiConsentCreate` mock'u **sabit** bir özet döndürüyordu; p2'nin yerel
  sağlayıcısıyla alınan onay p1'in API varış yerine bağlanıyor ve ekran yanlış
  yere "sağlayıcı değişti" diyordu (iki mevcut test kırmızı). Mock artık
  **istenen profile** bağlıyor — çekirdek sözleşmesinin aynısı.

**Kapı kanıtı 10/10** (her koşu orijinal metinden, geri yükleme sha256
`0e3ad3b08814766a7ec26d26e4961127503cd08afcf9f1a41ee2d5cb271adbcf` /
`f607b2bd629a65992407507ea7fe44441819192fe9f37768afb96e03fd79f23b` birebir):
çekirdek C1 `provider_current` daima true (1), C2 özet `capabilities`siz
türetilir (1), C3 değişim genele düşer (**4**), C4 profil satırı özet yerine
adapter taşır (1); arayüz U1 satır durumu değişimi yok sayar (1), U2 yeşil ton
(1 — aynı testin **farklı** assertion'ı: metin vs sınıf), U3 düğme kapısı (**2**),
U4 ilk off-dışı profille koşar (1), U5 tazeleme yok (1), U6 uyarı çizilmez (2).

**Ölçüm:** `cargo test -p dim-storage --lib` **400/400** (397 → +3) ·
`cargo clippy -p dim-storage --all-targets -D warnings` temiz · masaüstü
`cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets`
temiz (1 dk 09 s) · `cargo fmt` iki manifest temiz · `tsc --noEmit` temiz ·
`vitest` **1140/1140** (60 dosya; 1136 → +4) · `npm run build` temiz
(**593,29 kB**, `index-BswuJPWp.js`, gzip **160,07 kB**; CSS 7,28 kB).

**Dürüst sınırlar:** `ai_consent_list` / `ai_run_execute` **komut olarak
koşulmadı** (`State<AppState>` kuran test altyapısı yok; `commands_tests.rs`
CI'da koşmuyor) — kablolama derleme + clippy, davranış `list_transfer_consents`
ve `start_ai_run` düzeyinde **gerçek SQLite** ile doğrulandı. jsdom tarayıcı
değil: gerçek Tauri penceresi, gerçek işaretçi ve **görsel QA yok**; `api`
mock'lu olduğu için `provider_profiles`/`transfer_consents` SQL'i koşmaz.
"Yürütülebilir" kararı **özet eşitliğine** dayanır: aynı `adapter@politika` ve
aynı uç nokta+modeli taşıyan iki profil aynı varış yeri sayılır (parmak izi
`provider_type` taşımaz). Onay hâlâ **profil kimliği** saklamaz (yalnız özet).

**Açık kayıt:** (a) sağlayıcı **seçici** yok — onay ilk off-dışı profille
alınır; çalıştırma artık onayın kendi profiliyle koşar ama yeni onayın hangi
profille alınacağını kullanıcı seçemez; (b) envanterden tek tek "yenile" yok
(yenileme yeni onay almaktan geçer); (c) `E_AI_PROVIDER_CHANGED` sonrası
tazeleme **düşerse** yalnız birincil hata görünür (ikincil hata yutulur —
bilinçli, ama yazılı); (d) görsel QA **insan bekliyor**.
