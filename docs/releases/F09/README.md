# F09 — AI sağlayıcı çerçevesi

- **Sürüm etiketi:** `v0.8.0`
- **Durum:** Kısmi
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F09-T01 tek `Provider` sözleşmesi | ✅ | `crates/providers` |
| F09-T02 `OffProvider` | ✅ | her istek `E_AI_OFF` |
| F09-T03 aktarım onayı | ✅ | `transfer_consents`; onay **belge kümesine** bağlı (boş seçim, başka dosyanın belgesi ve silinmiş belge reddedilir) — ölçüldü 2026-10-28, aşağıdaki "Aktarım kapsamı" bölümü. **Maskeye de bağlanır** (2026-11-14, F09-T09b): onaylanan maske tanımı kayda yazılır, çalıştırma aynı maskeyi getirmezse reddedilir (`E_AI_MASK_CHANGED`); maske bağını taşımayan eski onay çalıştırılamaz (`E_AI_CONSENT_UNBOUND`) |
| F09-T04 çalıştırma sicili | ✅ | `ai_runs`, `ai_context_items`, `transfer_receipts` — **artık gerçek yolda yazılıyor**: gönderim öncesi belge/sürüm/bayt sicili, gönderim sonrası hedef + çıkan baytların sha256'sı (`ai_run_execute`). Makbuz **deneme anında** `attempted` açılır, `dispatched`/`failed` ile kapanır ve **okunabilir** (`list_transfer_receipts` + `ai_receipt_list` + ekran kartı; 2026-11-05 F09-T10). **Gönderim sicili de okunur** (`list_context_items` + `ai_context_items` + "Gönderilen içerik" kartı: belge adı, sürüm, gerekçe, bayt; 2026-11-06). **Çalıştırma satırı uzlaştırması (2026-11-11):** açılışta `running` kalan satır kesin duruma bağlanır (`failed` + `usage_report.error=interrupted`); ekran «oturum kesildi · sonuç yazılmadı» yazar (aşağıdaki bölüm) |
| F09-T05 onaysız çalıştırma reddi | ✅ | `E_AI_CONSENT`: onay yok / iptal / süresi geçmiş / **belge seçimi boş** / seçili belge silinmiş (7 test) |
| F09-T06 yerel model indirme/doğrulama | ⬜ | — |
| F09-T07 API anahtarı (şartname: OS güvenli deposu) | 🚧 **macOS + Windows** | `crates/providers/src/secret.rs`: anahtar **OS güvenli deposunda** tutulur — macOS **Anahtar Zinciri** (`security-framework`), Windows **Kimlik Yöneticisi** (`keyring` 3.6.3, 2026-11-06); sıra **depo → `DIM_API_KEY`**, seçilen hedef ekranda ve profil künyesinde (`auth_mode_verified`) **yazılır** — sabit iddia yok. Windows tatbikatı gerçek Credential Manager üzerinde CI `windows-latest` işinde **koştu** (25/25). Linux Secret Service **uygulanmadı** (`E_AI_SECRET_UNSUPPORTED` + ortam değişkeni). Aşağıdaki bölümler |
| F09-T08 gönderim önizlemesi (tam) | ✅ | `consent_preview`: belge, segment, bayt, 200 karakter örnek ve **kapsam dışı belgeler + nedeni**; gönderim yolu aynı çözümleyiciyi kullanır. Önceden SQL hatası veriyordu (aşağıdaki bölüm) |
| F09-T09 onay yenileme + iptal kapısı | 🚧 **arayüz kapsamı** | Ölçüldü 2026-11-05: dosya değişiminde onay düşer, **amaç** değişiminde onay düşer, iptali uçuşta olan onayla **yeni gönderim başlamaz**; görev türü onayın amacından çözülür. **Onay envanteri** eklendi: aynı dosya+amaç yeniden onaylanınca önceki yetki kapanır (denetimli), bütün onaylar görünür ve etkin olan iptal edilebilir (aşağıdaki bölüm). Çekirdek `start_ai_run` onayın `provider_snapshot`ını çalıştırılan profille karşılaştırır (F11-T04). **Kalan:** sağlayıcı değişiminde yenileme ekranda ulaşılabilir değil (sağlayıcı seçici yok, rota değişiminde ekran yeniden kuruluyor) |
| Şartname §06.3 eki — `ai_messages` görünen yanıt sicili | ✅ | Sağlayıcının **görünen** yanıt metni her çalıştırmada kaydedilir; JSON şemaya uymayan yanıt da saklanır (`E_AI_SCHEMA` yolunda kaybolmaz), özel akıl yürütme alanı yazılmaz; rol/durum kapalı küme, 262.144 karakter sınırında kırpma (`truncated`), kaynak başvurusu kapısı — kusurlu satır yazılmaz; yeni komut `ai_messages_list` + Ai ekranında "Model yanıtı" kartı (2026-11-10) |

## Dürüst durum

AI **varsayılan olarak kapalıdır**. Kayıtlı tek profil `off` ise arayüz `E_AI_OFF`
gösterir ve çalıştırma başlatılamaz. Gönderim yolu `ai_run_execute`dir (tek yol);
onaysız/iptal/süresi geçmiş onayla, maskesi onaylananla eşleşmeyen gönderimle ve
kapsam dışı belgeyle **çalıştırma başlamaz**.

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
(`State<AppState>` kuran test altyapısı yok; `commands_tests.rs` CI'da koşmuyordu (2026-11-06: `masaüstü kabuk` işi bu testleri artık koşuyor);
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
CI'da koşmuyordu (2026-11-06: `masaüstü kabuk` işi bu testleri artık koşuyor) — kablolama derleme + clippy, davranış `list_transfer_consents`
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

## Aktarım makbuzu envanteri ve deneme kaydı (2026-11-05, F09-T10)

**Neden bu dilim.** `transfer_receipts` tablosu F11 turundan beri **yazılıyordu**
(`ai_run_execute`), ama tek bir okuma yolu yoktu: `list_transfer_receipts`, IPC
komutu ve ekran satırı hiç yazılmamıştı. Avukatın "ne çıktı, nereye, kaç öğe,
kaç bayt, hangi içerik özetiyle" sorusunun cevabı yalnız SQLite dosyasındaydı.

**ÖLÇÜLEN KUSUR 1 (görünmez denetim kanıtı).** Makbuz yazılıyor, okunamıyordu:
`grep -rn "list_transfer_receipts"` boş döndü, `ai_receipt_list` komutu yoktu,
`Ai` ekranında makbuz kartı yoktu. Denetim izi "var" sayılıyordu ama kimse
göremiyordu.

**ÖLÇÜLEN KUSUR 2 (düşen deneme sicilden siliniyordu).** `record_transfer_receipt`
yalnız `provider.propose()` **başarılı olduktan sonra** çağrılıyordu. Zaman
aşımına düşen, bağlantısı kopan ya da yanıtı gelmeyen bir gönderim içerik
**çıkmış olabilir**; buna karşın sicilde ne hedef, ne bayt, ne özet kalıyordu —
yalnız `ai_runs.usage_report` içinde `{"error":"E_AI_…"}`. "Denendi" ile
"gönderildi" aynı kutuya giriyordu.

**Düzeltme.**

- `begin_transfer_receipt(...)` — makbuz **gönderim denenmeden önce**
  `attempted` olarak açılır; yazılamazsa aktarım **hiç başlamaz**
  (denetlenemeyen aktarım yapılmaz).
- `finish_transfer_receipt(id, status)` — `attempted` → `dispatched` | `failed`.
  Geçiş **tek koşullu `UPDATE`**dir (`WHERE id=? AND status='attempted'`):
  önce okuyup sonra yazmak, arada başka bir sonuçlandırmanın yazdığını ezme
  penceresi bırakırdı. Satır değişmediyse makbuz yoktur (`E_NOT_FOUND`) ya da
  zaten sonuçlanmıştır (`E_AI_RECEIPT`); sicil geriye dönük değiştirilemez.
- `list_transfer_receipts(case_id)` — `ai_runs` ile birleşir, **dosyaya
  kapsanır**, en yeni önce. `TransferReceiptRow` alanları: hedef etiketi, tam
  sha256 özeti, öğe/bayt sayısı, durum, zaman.
- `ai_receipt_list` komutu + `api.aiReceiptList` + `Ai` ekranında **"Aktarım
  makbuzları"** kartı. Kart dosya değişiminde ve **her gönderim denemesinden
  sonra** (başarı **ve** hata) çekirdekten tazelenir; okuma sıra kapısı
  (`receiptListSeq`) ile geç yanıt yeni dosyanın listesine yazılmaz.
- `ai_run_execute`: deneme makbuzu `propose`dan **önce** açılır; düşen
  gönderimde `failed` ile kapanır, başarıda `dispatched`. Çalıştırma raporu
  artık `"receipt": "dispatched" | "failed" | "not_finalized"` yazar.

**Testler.** `crates/storage/src/ai.rs` **31 → 36** (+5: deneme anı, düşen
deneme satırı, sonuç değişmezliği + bilinmeyen durum + olmayan kimlik,
dosya kapsamı, en yeni önce). `Ai.test.tsx` **70 → 78** (+8; yeni `describe`
"Aktarım makbuzu envanteri"). **Ham koşu (düzeltme öncesi bileşen, nihai 78
test): 8 failed | 70 passed** — sekizi de gerçek kusur; mevcut 70 test
düzeltme öncesinde de yeşil.

**Kapı kanıtı 14/14** (her koşu tek kapı; geri yükleme sha256
`18eacfd323d33f59f11f05a9098179bd32c0214a60e19b62ef5e9d85a357beaa` ve
`013126a35e23cab95adb2322c2f2104af7474c9647cf24a2d4da16aba5696084` birebir):
çekirdek C1 deneme kaydı `attempted` yerine `dispatched` (1), C2 koşullu UPDATE
kapısı kaldırıldı (1), C3 bilinmeyen durum kabul (1), C4 dosya kapsamı
kaldırıldı (1), C5 sıra sözleşmesi kaldırıldı (1), C6 olmayan makbuz
`E_NOT_FOUND` yerine makbuz hatası (1); arayüz U1 dosya değişiminde okuma yok
(**6**), U2 okuma sıra kapısı yok (1), U3 gönderim sonrası tazeleme yok (**2**),
U4 «denendi» «gönderildi» yazar (1), U5 «denendi» rozeti yeşil (1), U6 boş
listede tablo (**4**), U7 uçuşta «Yenile» açık (1), U8 okuma hatası sessiz (1).
**Dürüst:** U1'de iki test yeşil kaldı — biri «Yenile» ile elle okumayı, diğeri
p2'nin kendi okumasını ölçer (ilk okumaya bağlı değiller).

**Ölçüm.** `cargo test -p dim-storage --lib` **405/405** (400 → +5; `ai::tests`
**36/36**) · `cargo clippy -p dim-storage --all-targets -D warnings` temiz ·
masaüstü `cargo clippy --all-targets -D warnings` temiz · `cargo fmt` iki
manifest temiz · `tsc --noEmit` temiz · `vitest` **1159/1159** (60 dosya;
1151 → +8) · `npm run build` temiz (**597,70 kB**, `index-DH6BjBZj.js`, gzip
**161,13 kB**; CSS 7,28 kB).

**Commit/CI (ölçüldü).** Kod `b4f9671` (9 dosya, +701/−24) → **`42094357736`
7/7 YEŞİL** (ilk koşu). `arayüz` günlüğü **60 dosya / 1159 test** ve **aynı
paket** `index-DH6BjBZj.js` 597,70 kB / gzip 161,13 kB → yerel/CI **birebir**;
`rust (macos-15)` + `rust (ubuntu-latest)` `ai::tests` yeşil, `masaüstü kabuk
(cargo check)` yeşil. **Dürüst:** düzeltme sırasında masaüstü clippy ilk koşuda
`E0432` ile düştü — `TransferReceiptRow` kökten dışa aktarılmamıştı
(`crates/storage/src/lib.rs` düzeltildi); ikinci koşu temiz.

**Dürüst sınırlar.** `ai_receipt_list` ve `ai_run_execute` **komut olarak
koşulmadı** (`State<AppState>` kuran test altyapısı yok; `commands_tests.rs`
CI'da koşmuyordu (2026-11-06: `masaüstü kabuk` işi bu testleri artık koşuyor) — kablolama derleme + clippy, davranış `begin/finish/list`
düzeyinde **gerçek SQLite** ile doğrulandı. jsdom tarayıcı değil: gerçek Tauri
penceresi, gerçek işaretçi ve **görsel QA yok**; `api` mock'lu olduğu için
`transfer_receipts` SQL'i arayüz testinde koşmaz. `dispatched_at` sütunu
tarihsel adını korur; değeri **deneme anı**dır (gönderim anı değil) — alan
dokümanı bunu yazar.

**Açık kayıt.** (a) `attempted` kalan makbuz (süreç çöktü) **kendiliğinden
kapanmaz**; ekran «denendi · sonuç yazılmadı» yazar, otomatik uzlaştırma yok.
(b) Makbuz yalnız **içerik özetini** taşır; özetin kasadaki segmentlerle
yeniden karşılaştırılması arayüzde sunulmuyor (elle sha256). (c) Makbuz listesi
sayfalanmıyor. (d) Görsel QA **insan bekliyor**.

## API anahtarı OS güvenli deposunda (2026-11-06 — F09-T07)

Üç ölçülmüş kusur kapatıldı:

- **K1 (şartname boşluğu):** anahtar **yalnız** `DIM_API_KEY` ortam
  değişkeninden okunuyordu; uygulamanın kendi anahtar deposu yoktu ve ekran
  kullanıcıya "anahtar yalnız DIM_API_KEY'den okunur" diyordu. Şartname
  F09-T07 "doğrudan API anahtarını OS güvenli deposunda tut; seçilen hedefi
  açık göster" diyor.
- **K2 (yanlış beyan):** profil künyesi **sabit** `env:DIM_API_KEY` yazıyordu
  (`auth_mode_verified` ve `capabilities.auth`). Anahtar başka yerden
  okunduğunda Denetim kaydı yanlış kaynak söylerdi.
- **K3 (boşa harcanan onay):** anahtar yokken `ai_run_execute` önce onayı
  tüketiyor, `transfer_receipts`'e "denendi" yazıyor, sonra
  `E_AI_UNSUPPORTED` ("sağlayıcı desteklenmiyor") dönüyordu — hiçbir bayt
  çıkmadığı hâlde aktarım denenmiş gibi görünüyordu.

**Düzeltme.**

- `crates/providers/src/secret.rs` (yeni): `SecretStore` sözleşmesi +
  `OsStore` (macOS Anahtar Zinciri, `security-framework`) ve sıra
  **zincir → `DIM_API_KEY`**. `SecretTarget` etiketi (`os-keychain` /
  `env:DIM_API_KEY` / `none`) hem ekrana hem profil künyesine yazılır.
  Okuma **gerçek** bir hatayla düşerse yedeğe sessizce geçilmez
  (`E_AI_SECRET`); depo bu platformda yoksa `note` nedenini yazar.
- `ai_secret_status` / `ai_secret_store` / `ai_secret_clear` komutları: anahtar
  değeri **geri dönmez**, tabloya/Denetim kaydına/günlüğe yazılmaz (INV-11);
  boş anahtar `E_AI_SECRET_EMPTY`.
- `register_api_profile` artık `capabilities` içinde `auth` **iddia etmez**
  (`auth_mode_requested` = `auto`); kaynak çalışma anında çözülür.
- `ai_run_execute`: anahtar yoksa **hiçbir şey** denenmez — çalıştırma kaydı ve
  makbuz açılmaz, `E_AI_NO_KEY` döner.
- Sağlayıcılar ekranı: "API anahtarı" kartı — gerçek kaynak + kayıtlı/kayıtlı
  değil + (varsa) depo neden kullanılamıyor; kaydet/sil; yazılan anahtar
  ekranda geri gösterilmez, uçuşta yazılan taslak yanıtla silinmez.

**Testler.** `secret.rs` 10 test (9 koşar + 1 gerçek zincir tatbikatı
`#[ignore]`), `api.rs` +1, `ai.rs` +1 (**36 → 37**), `Providers.test.tsx`
**45 → 54**. Toplam: providers lib **14 → 24** (1 ignored), dim-storage lib
**405 → 406**, vitest **1159 → 1168** (60 dosya). Gerçek Anahtar Zinciri
tatbikatı yerelde koşuldu (yaz → oku → sil, ayrı hizmet adı; kullanıcı
kaydına dokunmaz) — **geçti**.

**Kapı kanıtı 16/16** (her koşu tek kapı, geri yükleme sha256 birebir
`6963cbb4…346af` arayüz / `1bdc86cc…`, `d63e837d…`, `aa4beb5b…` çekirdek):
arayüz U1 kaynak etiketi sabit (3), U2 açılışta durum okunmaz (9), U3 kaydetme
çekirdeğe gitmez (3), U4 taslak koşulsuz silinir (1), U5 uçuş kilidi yok (2),
U6 hata yutulur (1), U7 silme çekirdeğe gitmez (1), U8 "depo yok" uyarısı
çizilmez (1), U9 anahtar kırpılmadan gönderilir (1); çekirdek C1 ortam
değişkeni öncelik kazanır, C2 okuma hatası yutulur, C3 neden bildirilmez,
C4 boş kayıt anahtar sayılır, C5 `missing` etiketi yanlış, C6 künye sabit auth
yazar, C7 capabilities `auth` iddiası ekler.

**Ölçüm.** `cargo test -p dim-providers` **24/24** (+1 ignored) ·
`cargo test -p dim-storage --lib` **406/406** · `cargo clippy --workspace
--all-targets -- -D warnings` temiz · masaüstü `clippy --all-targets -- -D
warnings` temiz · `cargo fmt` iki manifest temiz · `tsc --noEmit` temiz ·
`vitest` **1168/1168** (60 dosya) · `npm run build` temiz (**601,02 kB**,
`index-DrHSau1V.js`, gzip **161,93 kB**; CSS 7,28 kB).

**Commit/CI.** `e1439e2` (14 dosya, +1135/−24) + `02523fb` (düzeltme +
bildirim) → **`42111280081` 7/7 YEŞİL**; `arayüz` günlüğü **60 dosya / 1168
test** ve **aynı paket** `index-DrHSau1V.js` 601,02 kB / gzip 161,93 kB →
yerel/CI **birebir**. **Dürüst:** ilk koşu **`42109884612` KIRMIZI** — (a)
ubuntu işi `error: fields service and account are never read` (`-D warnings`;
macOS dışı `OsStore` alan tutuyordu, alanlar hiç okunmuyordu), (b) SBOM kapısı
`security-framework` + `security-framework-sys` bildirimde yok diye kırmızı.
İkisi de `02523fb` ile kapandı.

**Dürüst sınırlar.** (a) OS deposu **yalnız macOS** (Anahtar Zinciri); Windows
kimlik yöneticisi ve Linux Secret Service **uygulanmadı** — o platformlarda
`OsStore` `E_AI_SECRET_UNSUPPORTED` döner, ekran ortam değişkenine düştüğünü
yazar (sessiz yok sayma yok). (b) `ai_secret_*` ve `ai_run_execute` **komut
olarak koşulmadı** (`State<AppState>` kuran test altyapısı yok;
`commands_tests.rs` CI'da koşmuyordu (2026-11-06: `masaüstü kabuk` işi bu testleri artık koşuyor) — kablolama derleme + clippy, davranış
`secret` düzeyinde **gerçek Anahtar Zinciri** ve gerçek SQLite ile. (c) Yerelde
`--target x86_64-unknown-linux-gnu` ile **derleme doğrulanamadı**: `cc-rs`
`x86_64-linux-gnu-gcc` bulamadı (ortam eksiği, kod değil) — ubuntu kanıtı
CI'dır. (d) jsdom tarayıcı değil: gerçek Tauri penceresi, gerçek işaretçi ve
**görsel QA yok**; `api` mock'lu olduğu için anahtar zinciri arayüz testinde
koşmaz.

**Açık kayıt.** (a) Windows/Linux güvenli deposu yok (yukarıdaki sınır). (b)
Aynı anda hem zincirde hem ortam değişkeninde anahtar varsa **zincir kazanır**;
ekran hangisinin kullanıldığını yazar ama gölgede kalan kaynağı **göstermez**.
(c) Anahtar rotasyonu/son kullanım zamanı izlenmiyor; anahtar yalnız
`Authorization` başlığında kullanılır, istek gövdesine/loga girmez. (d) Görsel
QA **insan bekliyor**.

## F09-T07 ek turu — Windows Kimlik Yöneticisi (2026-11-06)

**Neden.** Önceki tur OS deposunu **yalnız macOS** için uygulamıştı; Windows'ta
anahtar yalnız `DIM_API_KEY` ortam değişkeninden okunuyordu ve ekran bunu
yazıyordu. F09-T07'nin "OS güvenli deposu" şartı Windows'ta karşılanmıyordu.

**Düzeltme.**

- `crates/providers/src/secret.rs` → `#[cfg(target_os = "windows")] mod
  platform`: `keyring` 3.6.3 (`windows-native`) üzerinden Windows **Kimlik
  Yöneticisi** (Credential Manager, genel kimlik). `SecretStore` sözleşmesi
  **aynı**: kayıt yoksa `Ok(None)` (macOS `errSecItemNotFound` karşılığı
  `NoEntry`), gerçek erişim/bozulma hatası `E_AI_SECRET` (sessizce yedeğe
  düşme yok), boş anahtar `E_AI_SECRET_EMPTY`, silme idempotent. Anahtar
  değeri ekrana/IPC'ye dönmez (INV-11).
- `keyring` Win32 çağrılarını **güvenli** API ile sarar; çalışma alanının
  `unsafe_code = "forbid"` kapısı delinmedi (windows-sys'i doğrudan çağırmak
  `unsafe` gerektirirdi — güvenlik kapısı gevşetilmedi).
- Platform ayrımı: macOS `security-framework` (değişmedi), Windows `keyring`,
  Linux/öteki `Unavailable` + ortam değişkeni (not: "yalnız macOS Anahtar
  Zinciri ve Windows Kimlik Yöneticisi uygulandı").
- Arayüz kaynak etiketi platforma göre değişmediği için **"OS güvenli deposu
  (sistem kasası)"** oldu (`providers.keySourceKeychain`).
- **CI:** yeni `windows-secret` işi (`windows-latest`) — `cargo clippy -p
  dim-providers --all-targets -- -D warnings` + `cargo test -p dim-providers
  --lib`. macOS/ubuntu işleri bu modülü **hiç görmez** (`cfg`), bu yüzden ayrı
  iş gerekli.

**Testler.** Windows'ta `windows_credential_manager_round_trip` (yeni): ayrı
hizmet adı + süreç kimliğiyle **gerçek** Credential Manager üzerinde
yaz → oku → üzerine yaz (rotasyon) → sil → yok sırası; ürün kaydına dokunmaz.
macOS tatbikatı (`os_keychain_round_trip`) `#[ignore]` olarak kaldı (elle).
Yerel macOS: **24 passed, 1 ignored** (değişmedi). Windows CI:
**25 passed, 0 failed, 0 ignored** — tatbikat `ok`.

**Kanıt.**

- Yerel: `cargo test -p dim-providers --lib` **24/24** (+1 ignored) ·
  `cargo clippy -p dim-providers --all-targets -- -D warnings` temiz ·
  `cargo fmt` iki manifest temiz · `tsc` temiz · vitest **1168/1168** (60 dosya) ·
  build **601,02 kB** (`index-Cg-CCS7t.js`, gzip **161,93 kB**).
- Windows modülü yerelde **cross-check** edildi: ayrı iskelet crate'te aynı
  modül + tatbikat testi `--target x86_64-pc-windows-msvc` ile derlendi,
  clippy `-D warnings` temiz. **Dürüst:** repo içinde bu hedef derlenemedi —
  `ring` (ureq TLS) `assert.h` bulamadı (`cc-rs`, Windows SDK yok); gerçek
  kanıt CI'dır.
- **CI `42122673787` 8/8 YEŞİL** (ilk koşu, yeni iş dahil): `windows güvenli
  depo` işi **25/25** + clippy; `arayüz` **60 dosya / 1168 test** ve **aynı
  paket** `index-Cg-CCS7t.js` 601,02 kB / gzip 161,93 kB → yerel/CI **birebir**;
  `SBOM` yeşil (bildirim yeniden üretildi).
- **SBOM/bildirim:** `sbom.json` rust **485 → 488**; `THIRD_PARTY_NOTICES.md`
  **667 → 668** bileşen (kapsam 755, ölçülen 668; `keyring` MIT OR Apache-2.0).

**Dürüst sınırlar.** (a) Windows yolu **yerelde koşulmadı** (makine macOS);
kanıt `windows-latest` CI işidir — gerçek kullanıcı oturumunda **Tauri
penceresinden** anahtar kaydetme denenmedi. (b) Linux Secret Service hâlâ
**yok** (`E_AI_SECRET_UNSUPPORTED` + ortam değişkeni). (c) `keyring` Windows'ta
blob'u **UTF-16** yazar; macOS UTF-8 — iki platform aynı kasayı paylaşmaz
(anahtar zaten kasada taşınmaz). (d) Görsel QA **insan bekliyor**.

**Açık kayıt.** (a) Linux deposu yok. (b) `windows-secret` işi yalnız
`dim-providers` derler; **masaüstü kabuğun Windows derlemesi** hâlâ CI'da yok
(F02 MSI doğrulaması Windows makinesi/insan bekliyor). (c) Anahtar
rotasyonu/son kullanım izlenmiyor (değişmedi).

## Gönderim sicili **okunabilir**: hangi belge sürümü çıktı (2026-11-06, F09-T04)

**Ölçülen kusur.** `ai_context_items` F09-T04'ten beri gönderimden **önce**
yazılıyordu (hangi belge sürümü, hangi gerekçe, kaç bayt) — ama
`list_context_items` **yalnız `crates/storage` içinde** tanımlıydı: Tauri komutu,
`api.ts` bağı ve ekran kartı **yoktu** (`git grep`: tek tüketici yok). Makbuz
satırı yalnız **toplam** öğe/bayt ve içerik özetini taşıyordu; avukat "bu
çalıştırmada **hangi belge** gitti" sorusunu yanıtlayamıyordu. (Aynı sınıf kusur
dışa aktarma defterinde 2026-11-06'da kapatıldı — F14-T05.)

**İkinci ölçülen boşluk.** Satır yalnız `document_version_id` (UUID) taşıyordu:
ekranda okunacak bir ad ya da sürüm numarası yoktu.

**Yazılan.**

- `AiContextItemRow` artık `document_title` + `version_no` taşır; sorgu
  `document_versions`/`documents` ile **sol birleşim** yapar. Belge ya da sürüm
  silinmişse **satır düşmez**, ad `null` kalır (kimlik dururken ad uydurulmaz;
  `ai_context_items` sürüme yabancı anahtarla bağlı değil, denetim satırı yaşar).
  Sıra **kayıt sırasıdır** (`rowid ASC`): aynı damgaya düşen iki satır bile
  gönderim sırasını korur.
- Yeni **`ai_context_items`** komutu + `api.aiContextItems`.
- Ai ekranında **"Gönderilen içerik"** kartı: belge · sürüm · gerekçe · bayt ve
  altında **hesaplanan** toplam. Seçim yokken **okuma yapılmaz** ve "kayıt yok"
  denmez; düşen okuma **kendi satırında** yazılır (sessiz yutma yok); liste
  istekten **önce** temizlenir; geç yanıt yeni seçimin altına yazılmaz
  (`contextSeq`); dosya değişiminde sicil ve seçim birlikte düşer.
- Gönderimden sonra **yeni çalıştırma odaklanır** (`focusRun(candidate.aiRunId)`):
  sicil, gönderim anında ekranda — avukat sormak zorunda kalmaz.

**Testler.** `dim-storage` `ai::` **+2** (okunabilirlik + kayıt sırası; belge
yoksa satır durur, ad `null`) → lib **409 → 411**. Arayüz `Ai.test.tsx`
**78 → 86** (+8; yeni `describe` "gönderilen içerik sicili").

**Kapı kanıtı (gerçek koşular).**

- Düzeltme öncesi HEAD bileşeni + yeni testler: **`8 failed | 78 skipped`** —
  sekizi de kırmızı, yani testler gerçekten yeni davranışı ölçüyor.
- Arayüz mutasyonları **7/7**: M1 kart/okuma yok (**7** test — "seçim yokken
  okuma yapılmaz" testi doğası gereği yeşil kalır), M2 liste temizliği (1),
  M3 `contextSeq` kapısı (1), M4 düşen okuma "kayıt yok" yazar (1), M5 adsız
  belgeye uydurma ad (1), M6 gönderim sonrası odak yok (1), M7 görünüm kapısı
  (**2**). M2–M6 **cerrahi**; M1/M7 birden çok testi düşürür (dürüst).
- Çekirdek **3/3**: R1 sol→iç birleşim (**2** — *cerrahi değil*: mevcut
  `context_items_and_receipt_are_recorded` testi de düşer, çünkü var olmayan bir
  sürüm kimliği kullanıyor), R2 sıra ters (1), R3 ad birleşimi kaldırıldı (1).
- Geri yükleme sha256'ları **birebir** (arayüz `ff1b95d7…`, çekirdek `1d44bfc1…`).
- **Dürüst:** ilk "geç yanıt" testi M3 mutasyonunda **yeşil kaldı** — ikinci
  seçim birincinin okumasını hiç başlatmıyordu (`focusRun` içindeki `runSeq`
  kapısı onu kesiyordu). Test, ilk okumanın gerçekten uçuşta olduğunu
  bekleyecek şekilde düzeltildi; M3 yeniden ölçüldü (`1 failed`).
- **Yerel:** `dim-storage --lib` **411/411** (286,06 s) · `ai::` **39/39** ·
  clippy `-p dim-storage --all-targets -D warnings` temiz · masaüstü kabuk
  `cargo check --all-targets` temiz (3 dk 23 s) + clippy temiz · `cargo fmt` iki
  manifest temiz · `tsc --noEmit` temiz · vitest **1190/1190** (60 dosya) ·
  `vite build` **607,94 kB** (`index-Bkw0nht6.js`, gzip **163,44 kB**).
  **Dürüst:** ilk tam suite (cargo test ile **paralel**) 4 kırmızı verdi
  (`Redaction`/bir başka dosya, yük altında zamanlama); yalnız koşuda ve CI'da
  **1190/1190 yeşil** — bu turda eklenen testlerle ilgisi yok.
- **CI:** kod `5161594` → **`42308455810` 10/10 YEŞİL**; `rust (macos-15)`
  `dim-storage` **411 passed** (196,77 s) ve `rust (ubuntu-latest)` **411 passed**
  (266,10 s) — **birebir**; `arayüz` **60 dosya / 1190 test**, paket
  `index-Bkw0nht6.js` **607,94 kB** / gzip **163,44 kB** (yerel/CI birebir);
  `masaüstü kabuk` **34 koştu · 0 düştü · 5 yoksayıldı** + yoksayılan koşu
  **5 passed** (35,06 s).

**Dürüst sınırlar.** (a) `ai_context_items` **komut olarak koşulmadı**
(`State<AppState>` kurup komutu çağıran test altyapısı yok) — kablolama
`cargo check --all-targets` + clippy, davranış `list_context_items` düzeyinde
**gerçek SQLite** ile. (b) Sicil **bayt sayısını** ve sürümü yazar, metnin
kendisini saklamaz; gönderilen metnin içerik özeti makbuz satırındadır.
(c) Kayıt silinemez/düzenlenemez (denetim sicili), kartta filtre/sayfalama yok.
(d) jsdom tarayıcı değil → gerçek Tauri penceresi ve **görsel QA insan bekliyor**.
(e) `document_title` **anlık** okunur (kayıt anındaki ad saklanmaz): belge sonradan
yeniden adlandırılırsa eski çalıştırma yeni adı gösterir — sürüm kimliği ve bayt
değişmez.

## Görünen model yanıtı sicile yazılır (2026-11-10 — şartname §06.3 `ai_messages`)

- **Ölçülen boşluk:** tablo şemanın ilk sürümünden beri vardı, **tek satır
  yazılmıyordu**. `ApiProvider`/`LocalProvider` yanıtı `serde_json` ile çözüp
  yalnız yapısal yükü döndürüyor, ham içeriği atıyordu. Sonuç: (a) model JSON
  yerine düz metin döndürdüğünde çalıştırma `E_AI_SCHEMA` ile düşüyor ve
  **avukat modelin ne dediğini hiç göremiyordu**; (b) başarılı çalıştırmada bile
  yanıt metni (aday yükünün kaynağı) sicilde yoktu.
- **Yazılan:** `Candidate.visible_content` + `ProviderError::SchemaAnswer(String)`
  — yanıt çözülemese de metin hatayla birlikte taşınır (`code()` yine
  `E_AI_SCHEMA`). `record_ai_message(run, role, content, refs, state)` /
  `list_ai_messages(run)`: rol ve durum kapalı küme (`assistant|user|system|tool`;
  `visible|unparsed`), boş/kontrol karakterli içerik reddi, 262.144 karakter
  sınırında kırpma ve `state=truncated`, kaynak başvurusu biçim/yinelenme/sınır
  kapısı; kusurlu satır **hiç yazılmaz**. Denetim kaydı yalnız rol/durum/uzunluk
  taşır — **yanıt metni denetime yazılmaz**. `ai_run_execute` yanıtı **adaydan
  önce** yazar (aday yazımı düşse de metin kalır); içerik çıktıysa makbuz
  `attempted` bırakılmaz. Yeni komut `ai_messages_list`; Ai ekranında
  **"Model yanıtı"** kartı (seçili çalıştırma, kendi `messagesSeq` geç yanıt
  kapısı, düşen okuma "yanıt yok" demez, üç durum kendi etiketiyle, bilinmeyen
  durum ham yazılır).
- **Testler:** storage `ai::tests` **51 → 58** (+7) → `dim-storage --lib`
  **517 → 524** (312,39 s); sağlayıcı **+3** (api 2, local 1);
  `Ai.test.tsx` **105 → 110** (+5) → vitest **65 dosya / 1356 test**;
  `tsc --noEmit` temiz; `cargo fmt` iki manifest temiz; storage + masaüstü
  `clippy --all-targets -D warnings` temiz.
- **Kapı kanıtı (gerçek koşular; betikler `scratch/dim-ai-message-gates-core.py`
  ve `-ui.py`, her mutasyon tek başına, `try/finally` + sha256 birebir):**
  **19/19 KIRMIZI** — S1 rol kapalı kümesi · S2 durum kapalı kümesi · S3
  çalıştırma yokluğu kapısı · S4 boş içerik · S5 kontrol karakteri · S6 kaynak
  başvurusu biçimi · S7 kırpma · S8 `truncated` durumu · S9 kaynak başvurusu
  yazımı · S10 liste kapsamı · S11 liste sırası · S12 denetim kaydına metin
  sızması; P1 şema hatası yanıtı taşımaz (eski davranış) · P2 görünen içerik ham
  gövdeden (akıl yürütme sızar); U1 yanıt okunmaz · U2 düşen okuma yutulur · U3
  geç yanıt kapısı yok · U5 durum etiketi yazılmaz · U6 hata dalı çizilmez.
- **Dürüst (kendi hatam):** U4 ("dosya değişiminde yanıt temizlenmez") ilk
  ölçümde **yeşil kaldı** — `runId` sıfırlandığı için kart eski yanıtı zaten
  çizmiyordu; **yük taşımayan dört temizlik satırı** koddan çıkarıldı ve senaryo
  düşürüldü (test edilmeyen kapı bırakılmadı). S3'ün ilk betiği **EŞLEŞMEDİ**
  verdi (aynı `get_ai_run` kalıbı dosyada 4 kez) → benzersiz bağlamla yeniden
  ölçüldü (kırmızı).
- **CI (ölçüldü):** kod `9f4e29e` (12 dosya, **+743/−8**) → run `42836490597`
  **10/10 YEŞİL** (`dim-storage` **524** hem `macos-15` 267,32 s hem
  `ubuntu-latest` 430,36 s — **birebir**; `arayüz` **65 dosya / 1355 test** —
  yerel 1356 farkı ağaçtaki başka ajanın commit'siz `format.test.ts` testi;
  paket `index-L2m15Y5V.js` **667,71 kB** / gzip **178,52 kB**; `masaüstü kabuk`
  40/0/5 + yoksayılan koşu 5 passed · 0 atlandı).
- **Dürüst sınırlar:** (a) yalnız `ai_run_execute` yolunda yazılır; `ai_run_start`
  ile açılan elle çalıştırmanın yanıtı yoktur; (b) başarılı yanıt **JSON metni**
  olarak saklanır (sağlayıcının görünen içeriği budur), ayrı güzelleştirme yok;
  (c) 262.144 karakteri aşan yanıt kırpılır, sınır ötesi **saklanmaz**;
  (d) `source_refs` biçim doğrulamasından geçer, varlığı yeniden sorgulanmaz —
  değerler çekirdeğin kasadan okuduğu gönderim kaydından gelir, arayüzden
  yazılamaz; (e) şema `system|user|tool` rollerine izin verir, bugün yalnız
  `assistant` yazılır; (f) komut **komut olarak koşulmadı** (`State<AppState>`
  altyapısı yok) — kablolama check+clippy, davranış storage düzeyinde gerçek
  SQLite; (g) jsdom tarayıcı değil → gerçek Tauri penceresi + **görsel QA insan
  bekliyor**.

## Kesilen çalıştırma açılışta kesin duruma bağlanır (2026-11-11 — F09-T04 eki)

- **Ölçülen boşluk:** `start_ai_run` satırı `running` açar; durumu yalnız
  `finish_ai_run` yazar. Süreç gönderim sırasında ölürse satır **sonsuza kadar
  `running`** kalıyordu — Ai ekranının "Çalıştırmalar" tablosu aylar sonra da
  ham `running` yazıyordu, oysa sonucu yazacak çağrı yoktu. Makbuzlar için aynı
  boşluk 2026-11-11 turunda kapatılmıştı (`reconcile_attempted_receipts`);
  `ai_runs` satırının eşdeğeri yoktu.
- **Yazılan:** `reconcile_running_ai_runs()` — kasa açılışında tek koşullu
  `UPDATE … WHERE status='running'` → `failed` + `usage_report`
  `{"billing":"unknown","error":"interrupted","reason":"process_exit"}`.
  Gerekçe kodda yazılı: `ai_run_execute` kasa kilidini çalıştırmanın tamamı
  boyunca tutar → açılışta hiçbir çalıştırma uçuşta olamaz; `running` satırın
  sonucunu yazacak **canlı çağrı yoktur**. `ai_runs.status` CHECK kümesi
  `interrupted` taşımadığı için ayrım `usage_report`ta taşınır (şema değişikliği
  tabloyu yeniden kurmayı gerektirirdi). `completed_at` **yazılmaz** — bitiş
  zamanı bilinmiyor; açılış saatini yazmak uydurma olurdu. Sonuçlanmış satıra
  dokunulmaz, ikinci açılış 0 döner (idempotent), sayı denetime girer
  (**kimlik/içerik yok**). `remember()` iş kurtarması ve makbuz uzlaştırmasının
  yanında çağırır.
- **Arayüz:** "Çalıştırmalar" tablosu durumu artık **Türkçe** yazar
  (başarılı/başarısız/sürüyor/kısmi/iptal edildi);
  `usageReport.error=interrupted` taşıyan `failed` satır **«oturum kesildi ·
  sonuç yazılmadı»** (warn) olur — "başarısız" ya da "sürüyor" demez. Bilinmeyen
  durum ham kalır (ekran tanımadığı değeri uydurmaz).
- **Testler:** `ai::tests` **+1** (`reconcile_closes_orphaned_runs_as_interrupted`)
  → `dim-storage --lib` **525 → 526** (279,39 s, güncel kodla); `Ai.test.tsx`
  **+3** → **114 test** (dosya); `tsc --noEmit` temiz; `cargo fmt` iki manifest
  temiz; storage + masaüstü `clippy --all-targets -D warnings` temiz; i18n
  bağlama testleri **60/60**.
- **Kapı kanıtı (gerçek koşular; betikler `dim-run-reconcile-gates-core.py` ve
  `-ui.py`, her mutasyon tek başına, `try/finally` + sha256 birebir —
  `ai.rs ba6fb019…`, `Ai.tsx 81c4907…`):** **10/10 KIRMIZI** — S1 uzlaştırma hiç
  kapatmaz · S2 sonuçlanmış satır da ezilir · S3 kesinti işareti yazılmaz ·
  S4 bitiş zamanı uydurulur · S5 denetim yazılmaz · S6 denetim sayıyı taşımaz;
  U1 kesilen çalıştırma etiketi yazılmaz · U2 kesilen çalıştırma yeşil rozet
  alır · U3 bilinmeyen durum uydurma etikete düşer · U4 başarılı çalıştırma ham
  yazılır.
- **Dürüst (kendi hatam):** S5/S6'nın ilk betiği **EŞLEŞMEDİ (2)** verdi —
  aynı `if changed > 0 {` ve `"interrupted": changed` kalıpları makbuz
  uzlaştırmasında da geçiyor; iki mutasyon yalnız `ai_run` bloğuna özgü
  bağlamla yeniden yazıldı ve 6/6 yeniden ölçüldü (ilk koşunun 4/6'sı kanıt
  sayılmadı).
- **CI (ölçüldü):** kod `129882b` (6 dosya, **+278/−2**) → run `42986282535`
  **10/10 YEŞİL** (`dim-storage` **526** hem `macos-15` 260,30 s hem
  `ubuntu-latest` 648,53 s — **birebir**; `arayüz` **65 dosya / 1359 test**;
  paket `index-D53oJrbQ.js` **668,64 kB** / gzip **178,77 kB**; `masaüstü kabuk`
  40/0/5 + yoksayılan koşu 5 passed · 40 filtered; diğer altı iş yeşil).
- **Dürüst sınırlar:** (a) uzlaştırma yalnız **kasa açılışında** koşar — açık
  oturumda süreç ölürse satır bir sonraki açılışa kadar `running` görünür
  (doğru: o oturumda sonuç gelmemişti); (b) durum `failed` + işaret: DB'yi
  okuyan denetçi `interrupted` sözcüğünü `usage_report`ta bulur, `status`ta
  bulamaz (CHECK kümesi genişletilmedi — tablo yeniden kurulumu bu turun
  kapsamı dışında); (c) kesilen çalıştırmanın adayı/yanıtı yoktur — içerik
  çıkmış olabilir, sicilden kanıtlanamaz (makbuz ayrı satırdır); (d) komut yolu
  (`remember`) **komut olarak koşulmadı** (`State<AppState>` altyapısı yok) —
  kablolama check+clippy, davranış storage düzeyinde gerçek SQLite; (e) jsdom
  tarayıcı değil → gerçek Tauri penceresi + **görsel QA insan bekliyor**.

## Onay **maskeye bağlanır** — maskenin kaldırılması izni genişletir (2026-11-14 — F09-T09b)

- **Ölçülen boşluk (üç yüzü):** şartname §763 «maskelemenin kaldırılması … izni
  genişletir; uygulama mevcut izni kendiliğinden genişletemez» diyordu ama onay
  maskeyi **hiç bağlamıyordu**: (1) `ai_consent_create` maskeyi **almıyordu** —
  onayın maske hakkında hiçbir bilgisi yoktu; (2) `ai_run_execute` maskeyi **çağrı
  anında** alıyor, çekirdek onayla **karşılaştırmıyordu** — aynı onayla istenen her
  maske (maskesiz dâhil) gönderilebiliyordu; (3) "maske değişince Çalıştır kapanır"
  kapısı **yalnız arayüzdeydi** (`maskChangedSincePreview`). Sonuç: onaylanan
  **maskeli** önizleme yerine doğrudan IPC ile **maskesiz** içerik gönderilebiliyor,
  yani onaylanan içerik ≠ gönderilen içerik oluyordu (INV-05 sınıfı) — kullanıcı
  maskeyi onayladığını sanırken ham metin cihazdan çıkıyordu.
- **Yazılan (şema v30):** `transfer_consents.outbound_transform_id` — onay artık
  **maske tanımına** bağlanır (`outbound_transforms.settings_hash`). Terimler
  saklanmaz; yalnız tanım özeti + terim sayısı (0027'nin sözleşmesi). `NULL` = bu
  sütundan **önce** alınmış onay → maske bağı **bilinmiyor**, çalıştırma
  reddedilir (temkinli taraf; kullanıcı yeniden onay alır). Yeni onaylar
  **maskesizken de** bir tanıma (0 terim) bağlanır, böylece "maskesiz onay" ile
  "bağ bilinmiyor" ayırt edilebilir.
- **Yazılan (çekirdek):** `outbound_settings_hash` **tek kaynak** — gönderim sicili,
  onay bağı ve çalıştırma kapısı aynı işlevi kullanır; `canonical_mask_terms`
  (kırpma/tekrar tek tanım). `NewTransferConsent{mask_terms, mask_plan_id}`: plan
  seçildiyse bölgelerden **türetilen** terimler de onayın parçasıdır ve kapsam
  kapısı gönderim yoluyla **aynı** (`E_MASK_PLAN_SCOPE`, türetmeden **önce**).
  `NewAiRun.mask_terms` alanı **tip düzeyinde zorunlu** — çağıran "unutarak"
  maskeyi düşüremez; `start_ai_run` onaylanan maske özetiyle karşılaştırır, fark
  varsa `E_AI_MASK_CHANGED`, bağ yoksa `E_AI_CONSENT_UNBOUND` (süreç **başlamadan**).
  Eşitlik **tek yön değil**: maskenin kaldırılması da **artırılması** da yeni onay
  ister (onaylanmayan maske onaylanmayan metin gönderir).
- **Ölü yüzey silindi:** `ai_run_start` komutu — hiçbir arayüz tüketicisi yoktu,
  `ai_runs` satırını uydurma `input_manifest_hash:"pending"` ile açıyordu ve maske
  kapısını **atlıyordu**; tek üretim yolu `ai_run_execute`. `api.ts` bağı ve
  `generate_handler` satırı da kaldırıldı.
- **Arayüz:** `approve()` maskeyi ve planı **onaya** gönderir (eskiden yalnız
  önizlemeye gönderiyordu); envantere **"Maske"** sütunu eklendi — «N terim
  maskeli» / «maskesiz» / «maske bağı yok (eski onay) — çalıştırılamaz, yeniden
  onay alın». Bağ yokluğu **"maskesiz" diye yazılmaz** (bilinmiyor ≠ yok).
- **Testler (güncel kodla, gerçek koşular):** `ai::tests` **+6**
  (`consent_binds_the_mask_that_was_approved`, `consent_without_mask_refuses_a_mask_at_execution`,
  `pre_v30_consent_without_mask_binding_is_refused`, `consent_mask_plan_terms_are_part_of_the_binding`,
  `consent_refuses_a_plan_outside_the_selected_documents`, `consent_list_reports_the_mask_definition`)
  → `dim-storage --lib` **593 → 599** (382,44 s); `Ai.test.tsx` **114 → 115**
  (+1: maske sütunu üç hâli ayırır; 3 mevcut assertion maske argümanlarını kapsayacak
  şekilde düzeltildi) → tam vitest **66 dosya / 1431 test** (yerel); `tsc --noEmit`
  temiz; `cargo fmt` (kök + masaüstü manifesti) temiz; `clippy -D warnings` storage
  **ve** masaüstü temiz; masaüstü `cargo check --all-targets` temiz.
- **Kapı kanıtı (gerçek koşular; betik `scratch/dim-mask-consent-gates.py`, her
  mutasyon tek başına, `try/finally` + `os.utime` damgası + sha256 birebir —
  `ai.rs c2fb4e0c…`, `Ai.tsx bbec39c5…`):** **10/10 KIRMIZI, taban yeşilken**
  (taban: `consent` süzgeci **24/24**, arayüz **2/2**; geri yazım sha256 birebir,
  taban tekrar 24/24 + 2/2) — M1 onay maskeyi bağlamaz · M2 çalıştırma onaylanan
  maskeyle karşılaştırmaz · M3 maskesiz eski onay sessizce maskesiz sayılır ·
  M4 kanonik eşitlik kaybolur · M5 plan kapsam kapısı düşer · M6 plan terimleri
  onayın parçası olmaz · M7 envanter terim sayısını sıfırlar · U1 onay çağrısı
  maskeyi taşımaz · U2 maske sütunu çizilmez · U3 bağ yokluğu «maskesiz» yazılır.
- **CI (ölçüldü):** kod `18389bf` (11 dosya, **+640/−58**) → run `43528475117`
  **10/10 YEŞİL** (`dim-storage` **599** iki OS'ta birebir: macos-15 274,47 s /
  ubuntu-latest 565,07 s; `arayüz` 66 dosya / **1423** test — yerel 1431 farkı
  ağaçtaki başka ajanın commit'siz testleri; paket `index-Byj9hYJj.js` **692,43 kB**
  / gzip **184,77 kB**; `masaüstü kabuk` 51/0/5 (kaynak 56) + gerçek worker 5/0/0).
- **Dürüst sınırlar:** (a) **serbest terimlerin içeriği onayda saklanmaz** — yalnız
  tanım özeti (sha256) ve terim sayısı; sözlük saldırısına açık bir özet bilinçli
  kabul edildi (0027 sözleşmesi: kasada düz metin maske terimi tutulmaz). (b)
  Eşitlik **iki yönlü**: onaylanandan **fazla** maske de yeni onay ister (izni
  daraltır ama onaylanan metni değiştirir — bilinçli). (c) `E_AI_MASK_CHANGED`
  sonrası arayüzde "onayı yenile" akışı **yok**: kullanıcı onayı yeniden verir
  (ekran hatayı yazar, kendiliğinden tazelemez). (d) `outbound_transform_id`
  **yabancı anahtar değil** (0029 deseni): dönüşüm satırı silinirse bağ okunamaz →
  çalıştırma reddedilir (fail-closed), ama DB bağı zorlamaz. (e) Plan terimleri
  onay anında türetilir; plan sonradan değişirse (bölge eklenirse) **yeni onay**
  gerekir — eski onay eski terim kümesini bağlar. (f) Tauri komutu
  (`ai_consent_create`) ve gerçek pencere **insan bekliyor**; davranış storage
  düzeyinde gerçek SQLite ile, arayüz jsdom'da ölçüldü.
