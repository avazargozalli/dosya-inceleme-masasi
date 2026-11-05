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
| F09-T09 onay yenileme + iptal kapısı | 🚧 **arayüz kapsamı** | Ölçüldü 2026-11-05 (aşağıdaki bölüm): dosya değişiminde onay düşer, **amaç** değişiminde onay düşer, iptali uçuşta olan onayla **yeni gönderim başlamaz**; görev türü onayın amacından çözülür. **Kalan:** sağlayıcı değişiminde yenileme ekranda ulaşılabilir değil (sağlayıcı seçici yok, rota değişiminde ekran yeniden kuruluyor) ve çekirdek onayın `provider_snapshot`ını çalıştırmayla karşılaştırmıyor |

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
