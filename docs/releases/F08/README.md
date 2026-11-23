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
| F08-T04s | ✅ | **atıf kontrolü sicili kendi hükmünü kanıtlamalı** (şema **v46**): `citation_checks` üzerinde **3 tetikleyici** + 0037'nin kapsam tetikleyicisi **genişletildi** — alıntı boşluk olamaz, dolu emsal bağı **var olan** emsali gösterir, `reason_codes` metin dizisi, satır **incelenmemiş doğar**; `id`/alıntı/emsal/`identity_match`/`quote_match`/`reason_codes`/`created_at` **değişmez**; anlam hükmü kapalı küme + gerekçe + damga tutarlı (gerekçesiz `misleading`, damgasız `accurate` yazılamaz); `case_id` ve sürüm de dondu (aynı dosyada sürüm taşıması durur); ölçülen kusur: 22 kusur yazımının **21'i** kabul ediliyor, `quote_match` UPDATE ile çevriliyor, bağlam paneli başka emsalin metnini okuyordu; `scope_violations()` beş sınıfı **şimdi sayar** (`GUARDED_TABLES` 25 kalır); **22/22 mutasyon** (2026-11-16) |
| F08-T04ai | ✅ | **kronoloji olayı «bu olay şu tarihte oldu» iddiasıdır** (şema **v61**): `events` üzerinde **2 tetikleyici** — tarih/kesinlik/sıralama üçlüsü tutarlı (`date()` eşitliği: olmayan gün, dolgusuz gün, ay `13`/`00` düşer; aralıkta ayırıcı ve başlangıç ≤ bitiş), `sortable_date` tarihten türetilir (tarih yoksa NULL), doğuşta yalnız `draft`, damgalar ISO ve sıralı; `id`/`case_id`/`created_at`/`assertion_kind` **donar**, durum yalnız `draft`→`confirmed`, `deleted_at` tek yönlü, `row_version` azalamaz ve içerik değişiyorsa artmalı; ölçülen kusur: 66 kusur yazımının **66'sı** kabul ediliyordu (sıralama değeri çevrilerek olay yanlış sırada görünüyor, köken «belge»ye çevriliyor, satır başka dosyaya taşınıyordu); **44/44 mutasyon**; `GUARDED_TABLES` 25 kalır (2026-11-20) |
| F08-T04aj | ✅ | **inceleme hükmü (karar + çelişki değerlendirmesi) doğuşta kanıtlı, sonra değişmez** (şema **v62**): `review_decisions` + `contradiction_assessments` üzerinde **6 tetikleyici** — karar yalnız **açık** öğe için (yerine geçmemiş, hedefi yazılmamış, dosyası silinmemiş), `edit` yükü zorunlu, damga ISO + gerçek gün/saat, yük geçerli JSON; değerlendirme **kendi dosyasının çelişki adayı** için (INV-05); iki sicil de **hiçbir sütunuyla** değişmez ve silinemez (CASCADE dahil); ölçülen kusur: 61 kusur yazımının **57'si** kabul ediliyordu (karar `accept`→`reject` çevriliyor, kapanmış öğeye yeni karar yazılıyor, başka dosyanın değerlendirmesi kabul ediliyor, `dün` damga ve bozuk yük geçiyordu); **40/40 mutasyon**; `GUARDED_TABLES` 25 kalır (2026-11-20) |
| F08-T04ak | ✅ | **sayfa kanıtı (geometri, OCR kökeni, etiket) doğuşta iyi biçimli, sonra değişmez** (şema **v63**): `pages` üzerinde **2 tetikleyici** — kimlik kırpılmış-dolu ≤64, `page_index ≥ 0`, döndürme 0..359, etiket/koordinat uzayı kırpılmış-dolu, `width`/`height` NULL ya da **pozitif**, `quality_flags` **metin dizisi**; on bir kanıt sütunu **donar** (no-op takılmaz); ölçülen kusur: 47 yazımın **42'si** kabul ediliyordu (NULL/boş/boşluk kimlik, sıfır/negatif/metin geometri, `ocr`→`native` çevirisi, sayfanın **başka koşuya** taşınması, bozuk uyarı listesi) ve v54'ün sayfa sınırı hükmü **geriye dönük** bozulabiliyordu (maske sayfa dışına taşar); `GUARDED_TABLES` 25 kalır (2026-11-22) |
| F08-T04al | ✅ | **denetim defteri eklemelidir** (şema **v64**): `audit_events` üzerinde **4 tetikleyici** — doğuşta kimlik/kategori/işlem/aktör/`entity_id` kırpılmış-dolu ≤64, `metadata_allowlist` **JSON nesnesi**, damga `YYYY-MM-DDTHH:MM:SS.uuuuuuZ` **gerçek** gün/saat, `event_hash` 64 küçük harf hex, `prior_hash` **defterin son satırının özeti** (boş defterde NULL), satır defterin **sonuna** eklenir; dokuz sütun **donar** (no-op takılmaz); satır **silinemez**. Ölçülen: 39 kusur yazımının **39'u** kabul ediliyordu (2026-11-23) |
| F08-T04am | ✅ | **dosya tarafı ve rolü kanıtlı doğar, sonra değişmez; birleştirme makbuzu silinemez** (şema **v65**): `actors` + `case_participants` + `actor_merges` üzerinde **9 tetikleyici** — ad/kimlik/`normalized_names` iyi biçimli doğar ve kimlik sütunları **donar**; `review_state` yalnız **makbuzlu** birleştirme geçişleriyle değişir; katılımcı `case_id`/rol/not/aralık donar, `actor_id` yalnız makbuzun taşıdığı satırda ve yalnız hedef/kaynak yönünde değişir; silinmiş satır geri getirilemez; üç tabloda da satır **silinemez**. Ölçülen: 92 yazımın **92'si** kabul ediliyordu (2026-11-23) |
| F08-T04an | ✅ | **şifreli bayt kimliği kanıtlı doğar, sonra değişmez; canlı blob silinemez** (şema **v66**): `blobs` üzerinde **3 tetikleyici** + `integrity_checked_at` sütunu — `storage_id` **dosya adı belirteci** (1..=64, yalnız `[0-9a-z_-]`: yol kaçışı/uzantı/büyük harf/CR-LF-TAB imkânsız), `encrypted_size ≥ 1`, `wrapped_key_ref` iyi biçimli, doğuşta sayaç 0 ve **kanıtsız `verified` yok** (`integrity_status='unverified'`, damga NULL), damga ISO + gerçek gün/saat; **altı sütun donar**, sayaç yalnız ±1, hüküm yalnız **yeni damgayla**; **canlı blob silinemez** (dört gösterim sütunu). Ölçülen: 45 yazımın 40'ı kabul (34 kusur) → düzeltmeden sonra **47 RED · sapma 0**; **37/37 mutasyon** (2026-11-23) |
| F08-T04ao | ✅ | **maske tanımı içerik adreslidir: kanıtlı doğar, değişmez, canlıyken silinemez** (şema **v67**): `outbound_transforms` üzerinde **3 tetikleyici** — `settings_hash` **64 küçük harf hex**, `term_count ≥ 0` ve **`identity` ⇒ `term_count = 0`**, `placeholder` iyi biçimli (≤64, CR/LF/TAB yok), damga ISO + gerçek gün/saat; **altı sütun donar**; **canlı tanım silinemez** (onay *ve* sicil bağı). 0045'in «bu satırlar içerik adreslidir ve hiçbir yol silmez» **varsayımı** ilk kez **zorlanıyor**. Ölçülen: 47 yazımın 44'ü kabul (36 kusur) → düzeltmeden sonra **39 RED · sapma 0**; **25/25 mutasyon** (2026-11-23) |

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

## F08-T04s — atıf kontrolü sicili kendi hükmünü kanıtlamalı (şema v46, 2026-11-16)

**Neden.** `citation_checks` bir avukatın «bu alıntı şu emsalle eşleşiyor» iddiasıdır ve
üç olguyu birlikte taşır: **konu** (hangi belge sürümü, hangi alıntı metni, hangi aday
emsal), **hüküm** (`identity_match`/`quote_match`) ve **anlam incelemesi**
(`meaning_review`/`meaning_note`/`meaning_reviewed_at`; 0015: «boş bırakmak incelenmedi
demektir»). Arayüz rozetleri (`Precedents.tsx`: `badgeIdentity`, `badgeQuote`,
`badgeMeaning`) doğrudan bu sütunları basar; bağlam paneli (`citation_context`) alıntıyı
**bağlı emsalin** tam metninde arar. 0037 yalnız **kapsamı** kapıladı (satır kendi
dosyasının sürümünü gösterir); konu, hüküm ve anlam incelemesi hiçbir tetikleyici
taşımıyordu. v45'in kapanış notunda bu sütun «emsal eşleştirme sicili» diye sıradaki
aday olarak yazılmıştı.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v45; sonda
`~/.hermes/cache/scratch/dim-v46-probe/measure_v46.py`, log `measure_before.log`):**
22 kusur yazımının **21'i KABUL** edildi (tek RED: **var olan** çapraz sürüme taşıma —
0037'nin kapsam kapısı):
(1) **var olmayan** emsal kimliği yazıldı (bağlam paneli çözülemez hâle gelir);
(2) yalnız boşluktan oluşan alıntı; (3) bozuk `reason_codes` JSON'u (okuyucu sessizce
boş liste yapar); (4) `reason_codes` dizi değil (nesne); (5) yeni kontrol «incelendi»
damgasıyla doğdu — `meaning_reviewed_at` yok; (6) anlam hükmü kapalı kümede değil
(`'yanlis'`; rozet ham belirteci basar); (7) `identity_match` UPDATE ile
`match`→`mismatch` çevrildi; (8) `quote_match` UPDATE ile çevrildi; (9) alıntı metni
değiştirildi, hüküm sabit kaldı; (10) emsal bağı p1→p2 çevrildi — **hüküm p1'in ölçümü,
bağlam p2'nin metni**; (11) emsal bağı `NULL`'a çekildi; (12) var olmayan emsal bağı
yazıldı; (13) satır **aynı dosyada başka sürüme** taşındı — `list_citation_checks('c1','v2')`
kontrolü **başka belgenin** listesinde gösterdi; (14) `reason_codes` değiştirildi;
(15) `id` değiştirildi; (16) `created_at` değiştirildi; (17) `'misleading'` damgası
**gerekçesiz** yazıldı (çekirdek istiyor, SQL yolu istemiyordu); (18) `'accurate'`
damgası `meaning_reviewed_at` NULL iken yazıldı; (19) anlam hükmü kapalı küme dışına
taşındı; (20) `'not_reviewed'` + gerekçe dolu (hükmü olmayan gerekçe); (21) sürümü
**var olmayan** satırda `case_id` başka dosyaya taşındı (0037 kapısı yalnız «var olan
çapraz sürüm»ü reddediyordu). v45 taraması bu 8 satırın **hiçbirini** saymıyordu
(`citation_checks` = 0).

**Yazılan (şema v46, `0046_citation_check_verdict_guard.sql`, 3 tetikleyici +
0037'nin kapsam tetikleyicisinin genişletilmesi).**
`citation_check_subject_guard_bi` — alıntı boşluk değil; dolu `candidate_precedent_id`
**var olan** bir emsali gösterir (emsaller hiçbir yoldan silinmez → «var olmayan emsal»
denetim izi değil **kanıtlanabilir yalan**, v45 ile aynı fail-closed karar);
`reason_codes` **metin dizisi**; `meaning_review='not_reviewed'` + boş gerekçe + damgasız
(yeni kontrol **doğuştan incelenmiş olamaz**; inceleme ayrı avukat eylemidir).
`citation_check_subject_guard_bu` — `id`, `extracted_citation`, `candidate_precedent_id`,
`identity_match`, `quote_match`, `reason_codes`, `created_at` **değişmez** (satır hangi
alıntıyı hangi emsalle ölçtüyse öyle kalır; yeniden ölçüm **yeni satır** ister).
`citation_check_meaning_guard_bu` — anlam incelemesi kendi içinde tutarlı: kapalı küme
(`not_reviewed|accurate|misleading|off_context`), `not_reviewed` ⇒ boş gerekçe + damgasız,
incelenmiş ⇒ **damgalı**, `misleading`/`off_context` ⇒ **gerekçeli** (çekirdeğin kuralı
şemada). `scope_guard_citation_checks_bu` (0037) **DROP+CREATE** ile genişletildi:
`case_id` ve `input_document_version_id` **değişmez** — aynı dosyada sürüm taşıması ve
sürümü silinmiş satırın dosya taşıması da durur. Ret öneki **değişmedi**
(`DIM_SCOPE:citation_checks`): aynı sütun kümesini iki tetikleyici paylaşırsa hangisinin
koştuğu belirsiz kalırdı ve v37'nin mevcut kapı testleri kırılırdı. Yeni üç tetikleyici
`DIM_INTEGRITY:citation_checks` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.

**Çekirdek değişikliği (türetilmiş alan kaldırıldı).** `NewCitationCheck.meaning_review`
alandan **çıkarıldı** (F09-T09c deseni): yeni kontrol `not_reviewed` doğar, çağıran
«incelendi» damgası oluşturamaz — tip sistemi kararı atlamayı imkânsız kılar. Arayüzdeki
kopya da silindi (`api.ts` tipi + `Precedents.tsx` çağrısı): ikinci kaynak drift üretirdi.
INSERT `meaning_review`i sabit yazar. Tauri komutu `NewCitationCheck`i serde ile
çözümlediği için eski istemcinin fazladan alanı **yok sayılır** (geriye uyumlu).

**Tarama.** `scope_violations().citation_checks` alanı genişletildi: kapsam kuralının
yanında **beş sınıf** sayılır — bağı çözülemeyen emsal kimliği, kapalı küme dışı anlam
hükmü, gerekçesiz `misleading`/`off_context`, damgasız inceleme, incelenmemiş satırda
gerekçe/damga. Yeni tablo yok → `GUARDED_TABLES` **25 kalır** (türetim testi değişmedi).
**Dürüst sınır:** aynı dosya içinde **sürüm taşıması** taramadan görünmez (satırda
«hangi sürüme yazıldığı» bilgisi tek sütundur).

**Testler.** `precedents::tests` **+4**:

- `a_citation_check_cannot_be_written_without_a_provable_subject` — meşru satır yazılır;
  var olmayan emsal, boşluk alıntı, bozuk/nesne/sayı `reason_codes`, «doğuştan
  incelenmiş» (damgalı **ve** damgasız), kapalı küme dışı hüküm, incelenmemiş satırda
  gerekçe ve damga reddedilir; tabloda **yalnız** meşru satır kalır.
- `a_citation_checks_subject_and_verdict_are_immutable` — sekiz sütunun her biri tek tek
  UPDATE edilir (id, alıntı, emsal bağı + `NULL`, `identity_match`, `quote_match`,
  `reason_codes`, `created_at`) → hepsi `E_DB_INTEGRITY`; **aynı değeri** yeniden yazmak
  serbest; avukatın anlam incelemesi çalışır.
- `a_citation_meaning_cannot_claim_what_the_row_does_not_support` — damgasız inceleme,
  gerekçesiz/boşluk gerekçeli `misleading`/`off_context`, kapalı küme dışı değer,
  incelenmemiş satırda gerekçe/damga reddedilir; gerekçeli `misleading` ve gerekçesiz
  `accurate` meşru.
- `the_scan_counts_citation_checks_whose_verdict_cannot_be_trusted` — beş sınıfın her
  biri için bir satır (kapılar düşürülüp v46 öncesi biçimde yazılır, sonra geri kurulur)
  → tarama **5** der, satırlar **silinmez** (6 kalır).

`scope_guard::tests::a_citation_check_cannot_reference_another_cases_version` genişletildi:
aynı dosyada ikinci sürüme taşıma ve sürümü silinmiş satırın dosya taşıması artık
`E_DB_SCOPE`. **Dürüst (test düzeltmesi):** bu test eskiden
`SET meaning_review='accurate'` ile **damgasız** «incelendi» yazıyordu — yeni kural onu
reddettiği için damga eklendi; testin kusuru sabitlediği raporda yazılı. `upgrade_tests`
**+1** (`upgrade_keeps_citation_checks_written_before_the_verdict_gate`): v45 kasasındaki
iki tutarsız satır **silinmez**, tarama **2** der, yeni uydurma yazma kapıda durur, eski
hüküm UPDATE ile **onarılamaz**, gerekçeli anlam incelemesi çalışır. Toplam:
`dim-storage --lib` **682 → 687** (yerel **687/687 yeşil, 439,71 s**). Arayüz:
`tsc --noEmit` temiz, `Precedents.test.tsx` **56/56** (payload beklentisinden
`meaningReview` düştü).

**Kapı kanıtı.** **22/22 KIRMIZI**, taban iki uçta yeşilken (`K0` 12/12, `K1` 12/12;
`~/.hermes/cache/scratch/dim-v46-probe/gates_v46.py`, log `gates_v46.log`;
`try/finally` + `os.utime` + sha256 geri yazım birebir —
`0046…sql e7e93e15ccff89ec…`, `scope_guard.rs cff49d375a2e3628…`): M1 BI boşluk alıntı ·
M2 BI var olmayan emsal · M3 BI `reason_codes` geçerlilik · M4 BI `reason_codes` dizi ·
M5 BI «doğuştan incelenmiş» · M6 BI doğuşta gerekçe/damga · M7 BU alıntı metni ·
M8 BU emsal bağı · M9 BU kimlik/hüküm sütunları · M10 BU satır kimliği ·
M11 BU `reason_codes`/`created_at` · M12 BU kapalı küme · M13 BU gerekçesiz yanıltıcı ·
M14 BU damgasız inceleme · M15 BU incelenmemiş satırda gerekçe/damga · M16 BU `case_id` ·
M17 BU sürüm · M18–M22 tarama koşulları (emsal bağı, kapalı küme, incelenmemiş
gerekçe/damga, damgasız inceleme, gerekçesiz yanıltıcı). **Dürüst (kendi hatam):**
ilk koşuda **21/22** kaldı — **M5 YEŞİL** çünkü testteki `'accurate'` satırı **damgalı**
yazılıyordu ve damga koşulu (M6) aynı satırı yakalıyordu; damgasız ikinci satır
(`cc-early-bare`) eklendi, **22/22** yeniden ölçüldü, ilk koşunun 21 hükmü kanıt
sayılmadı.

**CI (ölçüldü).** Kod `658a2f6` (8 dosya, **+712/−26**) → run `43925089905` **10/10 YEŞİL**
(`dim-storage` **687** iki OS'ta birebir: ubuntu-latest **378,63 s** / macos-15 **447,13 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` **694,50 kB** / gzip 185,32 kB —
`meaningReview` alanının payload'dan kalkmasıyla paket **değişti**; `masaüstü kabuk` 51/0/5 +
gerçek worker 5/0/0, `windows güvenli depo` 28, `workers`, `sözleşmeler`, `SBOM`,
`kaynak arşivi`, `sürüm paketi manifesti`).

**Dürüst sınırlar.** (a) `precedents` satırları hiçbir yoldan silinmez (silme komutu yok;
`ON DELETE CASCADE` yalnız sürüm/not için) → «var olmayan emsal» fail-closed kapıya
takıldı; **var olmayan sürüm** kimliği 0037'de olduğu gibi serbesttir (belge sonradan
silinebilir; kapı onu ölçemez). (b) Hükmün **doğruluğu** yeniden hesaplanmaz: kapı «hüküm
sonradan değişmedi» der, «hüküm doğru» demez (alıntı yeniden taranmaz). (c) SQLite
`trim()` yalnız verilen ASCII karakterleri kırpar; yalnız U+00A0 gibi boşluklardan oluşan
alıntı bu kapıdan geçer (çekirdek `str::trim` ile reddeder — asıl yol kapalı, doğrudan
SQL artığı kalır). (d) v46 öncesi tutarsız satırlar **silinmez** ve immutability onarımı
engeller; tarama onları **sayar** — temizlik ürün kararı. (e) Tetikleyicileri anahtarı
olan saldırgan düşürebilir (uygulama hatasına karşı). (f) Arayüz **davranışı** değişmedi
(rozetler aynı sütunları basar); gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04t — gönderim sicilinin metni ve özeti kanıtlanabilir olmalı (şema v47, 2026-11-16)

**Neden.** `transfer_segments` satırı «modelin **gördüğü** metin» iddiasıdır ve üç yerde
okunur: `list_transfer_segments` metnin ilk 120 karakterini ve `outbound_hash`in ilk
12'sini ekrana basar (Ai.tsx gönderim sicili tablosu), `sent_segments` aday
doğrulamasında alıntıyı **bu metinde** arar, `source_range_map` alıntının özgün karakter
aralıklarına geri eşlenmesini sağlar (maske sonrası bağlama). v45 bu tabloda yalnız
**dönüşüm bağını** kapıladı (`transform_manifest_id` onaylanan maskeyi gösterir,
`id`/`ai_run_id` değişmez) ve kapanış notunda sınırını açıkça yazdı: «Sicil metni
(`outbound_text`/`outbound_hash`) bu dilimin dışında: UPDATE ile değiştirilebilir kalır
(sıradaki aday)».

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v46; sonda
`~/.hermes/cache/scratch/dim-v47-probe/measure_v47.py`, log `measure_before.log`):**
14 kusur yazımının **12'si KABUL** (2 RED yalnız 0038'in sürüm/segment taşıması kapısı):
(1) yazılmış satırın `outbound_text`i UPDATE ile değiştirildi — okuyucu yeni metni
gösterdi, `sent_segments` alıntıyı sahte metinde arayacaktı; (2) `outbound_hash` metinden
bağımsız çevrildi (çift tutarsız, ekranda yeni özet); (3) metin + hash **birlikte**
çevrildi (tutarlı görünen sahtecilik); (4) `source_range_map` başka aralıklara çevrildi
(alıntı yanlış özgün aralığa bağlanır); (5) `source_char_count` değiştirildi (harita
genişliği kayar); (6) `outbound_segment_id` **aynı sürümde** başka segmente çevrildi —
satır «s1» der, metin s2'nindir (alıntı yanlış çapaya bağlanır); (7) `source_segment_id`
çevrildi; (8) boş metinle satır yazıldı; (9) yalnız boşluktan oluşan metinle satır
yazıldı; (10) `outbound_hash='hash'` (64 haneli değil) yazıldı — ekran onu özet diye
basar; (11) `outbound_hash='Z'*64` (hex değil) yazıldı; (12) `source_char_count=0`
yazıldı. `document_version_id` taşıması 0034/0038 kapsam kapısında zaten duruyordu
(ölçümde 2 RED); bu göç onu **tekrar** kapılamaz (aynı sütunda iki değil, aynı **satırda**
iki tetikleyici olurdu — v46 dersi).

**Yazılan (şema v47, `0047_transfer_segment_content_guard.sql`; v45 adları korunur).**
`transfer_segments_transform_guard_bi` **DROP+CREATE** ile genişletildi: v45'in dönüşüm
koşulları **aynen** durur; ek olarak satır **doğarken** iyi-biçimli olmalı — metin
`NULL`/boş/yalnız-boşluk değil (SQLite `trim()` ASCII boşlukları kırpar), `outbound_hash`
**64 küçük-harf hex** hanesi, `source_char_count` doluysa **pozitif**. `NULL` metin/hash
NOT NULL kısıtından **önce** tetikleyiciye düşer (SQLite BEFORE INSERT sırası) — kapı
onları da kendi sınıfında reddeder. `transfer_segments_transform_guard_bu` **DROP+CREATE**:
v45'in dondurduğu üç sütun (`id`, `ai_run_id`, `transform_manifest_id`) **aynen** donmaya
devam eder; ek olarak satırın **içeriği** de donar — `outbound_text`, `outbound_hash`,
`source_range_map`, `source_char_count` değişmez (yeniden ölçüm **yeni satır** ister).
`scope_guard_transfer_segments_bu` (0038) **DROP+CREATE** ile genişletildi: özne sütunları
(`outbound_segment_id`, `source_segment_id`) da **donar** — v46'da aynı sürüm içinde başka
segmente takas kabul ediliyordu. Sütun zaten 0038'in kapsam tetikleyicisinde olduğu için
**aynı** tetikleyici genişletildi (v46 dersi: aynı sütunda iki BEFORE tetikleyici hata
kodunu belirsizleştirir); önek/numara korunur → `DIM_SCOPE:transfer_segments`.
`document_version_id` 0038'de kalır (NULL → sürüm **tamamlamak** meşru onarım yoludur).
Ret `DIM_INTEGRITY:transfer_segments` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.

**Test düzeltmesi (dürüst).** Bu göçten önce yazılmış **13** test fikstürü gerçek olmayan
`'hash'` değerini `outbound_hash`e yazıyordu (scope_guard 6, `ai` v45 testleri 3,
`upgrade_tests` 4, `scope_guard_cost` 1); yeni BI biçim kapısı bunları reddeder. Fikstürler
**64 hex**'e çevrildi — kapı gevşetilmedi, test verisi gerçek çekirdeğin yazdığı biçime
uydu. **Dürüst (kendi hatam):** ilk tam paket koşusu **687/689** düştü çünkü yeni BU
tetikleyicisini yazarken v45'in dondurduğu `id`/`ai_run_id`/`transform_manifest_id`
sütunlarını listeden **düşürmüştüm** (aynı adlı tetikleyici DROP+CREATE edilince eski
kapsam kayboluyor); iki test (`outbound_record_can_only_claim_the_mask_the_consent_approved`,
`upgrade_guards_transfer_transform_bindings_written_before_v45`) bunu yakaladı, üç sütun
geri eklendi.

**Testler.** `ai::tests` **+1** (`transfer_segment_content_is_frozen_and_well_formed`):
yedi sütunun her biri tek tek UPDATE edilir (id, `ai_run_id`, `transform_manifest_id`,
metin, hash, harita, karakter sayısı) → `E_DB_INTEGRITY`; özne sütunları →
`E_DB_SCOPE`; `NULL`/boş/boşluk metin, `NULL`/kısa (63)/hex olmayan hash,
`source_char_count=0`, uydurma dönüşüm kimliği (onaylı **ve** onaysız çalıştırmada) ve
onaylanan maskeden farklı **var olan** maske reddedilir; meşru yazma ve **aynı değerli**
UPDATE serbest. `upgrade_tests` **+1**
(`upgrade_keeps_transfer_segments_written_before_the_content_gate`): v46 kasasındaki
`'hash'`li satır **silinmez**, UPDATE ile **onarılamaz**, yeni bozuk hash kapıda durur,
çekirdeğin biçimi yazılabilir. Toplam: `dim-storage --lib` **687 → 689**
(yerel **689/689 yeşil, 460,90 s**). Biçim (iki manifest) / clippy `-D warnings` temiz.

**Kapı kanıtı.** **17/17 KIRMIZI**, taban iki uçta **8/8 yeşilken**
(`~/.hermes/cache/scratch/dim-v47-probe/gates_v47.py`, log `gates_v47_final.log`;
`try/finally` + `os.utime` + sha256 geri yazım birebir —
`0047…sql adac3f567f920f9b…`): M1a BI NULL metin · M1b BI NULL hash · M1 BI boşluk metin ·
M2 BI 64 hane · M3 BI hex karakter · M4 BI pozitif `source_char_count` · M5 BI dönüşüm
varlığı/türü · M6 BI onaylanan maske eşitliği · M7–M10 BU metin/hash/harita/karakter
sayısı · M11–M12 kapsam BU özne sütunları · M13 BU satır kimliği · M14 BU çalıştırma
kimliği · M15 BU dönüşüm bağı. **Dürüst (ilk koşu 11/17):** (a) `'hash'` testi M2'yi
değil M3'ü (hex olmayan karakter) ölçüyordu → 63 haneli hex satırı eklendi; (b) uydurma
dönüşüm kimliği onaylı çalıştırmada **eşitlik** kuralına da takıldığı için M5'i
maskeleyordu → onaysız çalıştırma (`run-bare`) satırı eklendi; (c) M7/M1 desenleri
`cargo fmt` sonrası metne uymuyordu → desenler son hâlden alındı; (d) `ai_run_id` ve
`transform_manifest_id` testleri filtreye (`transfer_segment`) girmiyordu → dondurma
listesine eklendi; **17/17** yeniden ölçüldü, ilk koşunun 11 hükmü kanıt sayılmadı.

**CI (ölçüldü).** kod `6bcec1f` (6 dosya, **+473/−16**) → run `43947104984` **10/10 YEŞİL**
(`dim-storage` **689** iki OS'ta birebir: macos-15 **400,22 s** / ubuntu-latest **599,68 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**,
arayüz kodu değişmedi; `masaüstü kabuk` + worker + `windows güvenli depo` yeşil).
Taramaya yeni sınıf eklenmedi: `transfer_segments` zaten kapsam taramasında; biçim/immutability
denetimi taramada **yok** (aşağıdaki sınır (a)/(b)).

**Dürüst sınırlar.** (a) Kapı metnin kendi `outbound_hash`iyle **tutarlı** olduğunu doğrulamaz
(SQLite'ta SHA-256 yok): metni ve hash'i **birlikte** yazan bir yol tutarlı görünen sahtecilik
üretebilir — çekirdek yazıcı doğru hesaplar, doğrudan SQL artığı kalır. (b) `source_range_map`
**içeriği** denetlenmez, yalnız donar: harita ile metin arasındaki uyum yeniden hesaplanmaz.
(c) v47 öncesi bozuk satırlar **silinmez** ve immutability onarımı engeller — temizlik ürün
kararı. (d) `document_version_id` bilinçli olarak 0038'in kapsam kapısında bırakıldı (NULL →
sürüm **tamamlama** meşru onarım yolu); kapsam taşıması orada durur. (e) Tetikleyicileri
anahtarı olan saldırgan düşürebilir (uygulama hatasına karşı). (f) Arayüz **davranışı**
değişmedi (ekran aynı sütunları basar); gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04u — emsal sürümünün metni ve özeti kanıtlanabilir olmalı (şema v48, 2026-11-17)

**Neden.** `precedent_versions` satırı «metin elimizde ve **doğrulandı**» iddiasıdır
(`verification_state='text_verified'`). Üç okuyucusu var: (1) `screens/Precedents.tsx`
`body_hash`in ilk 12 hanesini ve `text_char_count`u ekrana basar; (2) **metin içi alıntı
doğrulaması** (F12) alıntıyı `full_text` içinde arar — metin yoksa sonuç
`reason: 'no_full_text'` yani «kontrol edilmedi» olur; (3) v44 çakışma rozeti «aynı künye,
**farklı metin**» iddiasını `body_hash` farkıyla kanıtlar. v44 yalnız çakışma işaretini ve
`body_hash`in UPDATE ile değişmezliğini kapıladı; `full_text`, `text_char_count`,
`source_kind`, `acquired_at` ve `document_version_id` serbest kaldı (v44 kapanış notu bunu
sınır olarak yazmıştı).

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v47; sonda
`~/.hermes/cache/scratch/dim-v48-probe/measure_v48.py`, log `measure_before.log`):**
14 kusur yazımının **14'ü KABUL** edildi: (1) `body_hash='hash'` (64 hane değil) — ekran
onu özet diye bastı; (2) `body_hash` **büyük** harf hex — aynı metin iki farklı kimlik olur;
(3) 63 haneli özet; (4) `full_text=''` + `text_verified`; (5) yalnız boşluktan oluşan metin
+ `text_verified`; (6) `full_text=NULL` + `text_verified` («doğrulandı» ama metin yok);
(7) `text_char_count` metinle uyuşmuyor (15 karakterlik metne 99); (8) `source_kind='uydurma'`
(kapalı küme dışı); (9) `acquired_at=''` — sürüm sıralaması bozulur; (10) yazılmış satırın
`full_text`i UPDATE ile çevrildi — alıntı doğrulaması **başka metne** bakar, «doğrulandı»
iddiası yerinde kalır; (11) `text_char_count` değiştirildi; (12) `acquired_at` geriye
çekildi; (13) `source_kind` çevrildi (köken iddiası); (14) `document_version_id` başka
sürüme taşındı (köken çaprazlandı).

**Yazılan (şema v48, `0048_precedent_version_content_guard.sql`; v44'ün çakışma kapıları
aynen durur — ayrı tetikleyici, farklı sütunlar, aynı önek).**
`precedent_versions_content_guard_bi`: `body_hash` **64 küçük harf hex**; `full_text`
`NULL` değil, boş/yalnız-boşluk değil ve **kırpılmış** (çekirdek `trim()` sonrası yazar);
`text_char_count` = `length(full_text)` (SQLite metin için **karakter** sayar; çekirdek
`chars().count()` yazar — aynı ölçü); `acquired_at` boş değil; `source_kind` kapalı küme
(`paste`/`document`/`file` — `import_precedent_copy`in kabul ettiği üç değer).
`precedent_versions_content_guard_bu`: `full_text`, `text_char_count`, `acquired_at`,
`source_kind` **değişmez** (yeniden içe alma **yeni satır** ister; nüsha tekrarı `body_hash`
ile yakalanır); `document_version_id` yalnız **NULL'a çekilebilir** — belge sürümü
silinebildiği için bağın koparılması meşru onarımdır; başka sürüme **çevrilemez** (köken
iddiası sonradan taşınamaz). `verification_state` serbest kalır (v35 dersi: durum ilerler).
Ret `DIM_INTEGRITY:precedent_versions` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
Tablo `case_id` taşımadığı için kapsam taramasına sınıf **eklenmedi**; `GUARDED_TABLES`
**25 kalır**.

**Test düzeltmesi (dürüst).** v44'ün iki testi ve ilgili upgrade testi satırları `'hash-a'`
gibi 64 hane olmayan özetlerle ve `full_text`/`text_char_count` olmadan yazıyordu; yeni BI
bunları reddeder. Fikstürler **64 küçük harf hex** + dolu metin + uyumlu sayıya çevrildi —
**kapı gevşetilmedi**; ayrıca reddedilmesi beklenen yazılar da biçimce geçerli kılındı ki
v48 kapısı v44 kurallarını **maskelemesin** (v47'nin M5 dersi).

**Testler.** `precedent_import` **+1** (`precedent_version_content_is_frozen_and_well_formed`):
on biçim/künye kusuru `E_DB_INTEGRITY`; metin/karakter sayısı/köken/edinme zamanı UPDATE
edilemez; `document_version_id` taşınamaz ama **koparılabilir**; `verification_state`
güncellenebilir; değişmeyen yazma takılmaz. `upgrade_tests` **+1**
(`upgrade_keeps_precedent_versions_written_before_the_content_gate`): v47 kasasındaki
bozuk özetli/metinsiz satırlar **silinmez**, UPDATE ile **onarılamaz**, yeni bozuk yazım
kapıda durur, çekirdeğin biçimi geçer. Toplam `dim-storage --lib` **689 → 691**
(yerel **691/691 yeşil, 471,62 s**). Biçim (iki manifest) / clippy `-D warnings` /
masaüstü `cargo check --all-targets` temiz.

**Kapı kanıtı.** **15/15 KIRMIZI**, taban iki uçta **3/3 yeşilken**
(`~/.hermes/cache/scratch/dim-v48-probe/gates_v48.py`, log `gates_v48_final.log`;
`try/finally` + `os.utime` + sha256 geri yazım birebir —
`0048…sql d392ac83468912cc…`): M1–M8 BI (64 hane · hex · NULL metin · boş/boşluk ·
kırpılmamış · karakter sayısı · `acquired_at` · `source_kind` kümesi) · M9–M14 BU
(metin · karakter sayısı · `acquired_at` · `source_kind` · `document_version_id`
çevrilemez · OF listesi) · M15 OF listesinde `full_text` yok. **Dürüst (ilk koşu
14/15):** M9 deseni `WHEN` satırının başına iki boşluk koyuyordu, dosyada girinti yok →
**EŞLEŞMEDİ** (hüküm değil); desen düzeltildi, **15/15** yeniden ölçüldü, ilk koşunun
14 hükmü kanıt sayılmadı.

**Dürüst sınırlar.** (a) Kapı `body_hash`in **metnin** SHA-256'sı olduğunu doğrulamaz
(SQLite'ta SHA-256 yok): metni ve özeti **birlikte** yazan yol tutarlı görünen sahtecilik
üretebilir — çekirdek doğru hesaplar, doğrudan SQL artığı kalır. (b) SQLite `trim()` yalnız
ASCII boşlukları kırpar; U+00A0'dan oluşan metin bu kapıdan geçer (çekirdek `str::trim`
reddeder). (c) v48 öncesi bozuk satırlar **silinmez**, içerik donduğu için **onarılamaz** —
onarım yolu aynı künyeye **yeni sürüm** yazmaktır (nüsha tekrarı `body_hash` ile yakalanır);
temizlik ürün kararı. (d) `verification_state` geçişleri denetlenmez (`text_verified` →
`metadata_verified` serbest); durum bir **iddia** olarak kalır, metin doğrulaması yeniden
çalıştırılmaz. (e) `document_version_id`nin **varlığı** denetlenmez (yalnız taşınamaz);
silinmiş sürüme işaret eden satır kalabilir. (f) Tetikleyicileri anahtarı olan saldırgan
düşürebilir (uygulama hatasına karşı). (g) Arayüz **davranışı** değişmedi; gerçek pencere +
görsel QA **insan bekliyor**.

**CI (ölçüldü).** kod `99b4510` (4 dosya, **+405/−40**) → run `43962162199` **10/10 YEŞİL**
(`dim-storage` **691** iki OS'ta birebir: macos-15 **371,76 s** / ubuntu-latest **670,48 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**,
arayüz kodu değişmedi; `masaüstü kabuk` + worker + `windows güvenli depo` + SBOM + kaynak
arşivi yeşil). Belge CI'ı `65577e8` (v47 bölümü) `43958581152` **10/10 yeşil** — önceki
koşunun açık bıraktığı kanıt bu turda ölçüldü.

## F08-T04v — emsal notunun kapsamı, konusu ve metni kanıtlanabilir olmalı (şema v49, 2026-11-17)

**Neden.** `precedent_notes` hukukçunun **iş ürünüdür**. Arayüz her notun yanında kapsam
rozetini basar: `caseIdOptional` doluysa «dosyaya özel», boşsa «kütüphane» notu
(`screens/Precedents.tsx`); sözleşme `create_precedent_note` yorumunda yazılıdır —
«verilirse o dosyaya özeldir; verilmezse kütüphane notudur; başka dosyanın özel notu bu
yolla taşınmaz». Rozetin yanında `createdAt` gösterilir ve üç metin alanı (benzerlik /
ayırt edici / sonuç) okunur. Çekirdek yalnız **INSERT** eder: not doğduktan sonra
düzeltme/silme yolu yoktur. Şemada ise tablo 0001'den beri **hiçbir tetikleyici
taşımıyordu** — kapsam, konu, metin ve zaman doğrudan SQL ile serbestçe çevrilebiliyordu
(kapsam rozeti bir gizlilik iddiasıdır; serbest UPDATE onu sessizce genişletir).

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v48; sonda
`~/.hermes/cache/scratch/dim-v49-probe/measure_v49.py`, log `measure_before.log`):**
18 kusur yazımının **15'i KABUL** edildi (3 RED yalnız FK/NOT NULL'ın kendisiydi):
(1) not **silinmiş** dosyaya bağlandı (çekirdek reddeder, şema kabul etti) — rozet
«dosyaya özel» der, dosya yoktur; (2) `id=''`; (3) `created_at=''` — ekranda tarih boş
basılır; (4) kırpılmamış metin (`'  boşluklu  '`; çekirdek `trim` eder); (5) yalnız
boşluktan `useful_passage_anchor`; (6) **dosyaya özel not `case_id_optional=NULL`
yapılarak kütüphaneye çevrildi**; (7) not **başka dosyaya** taşındı; (8) not **başka
emsale** taşındı — metin artık başka kararın altında okunur; (9) benzerlik metni;
(10) sonuç metni; (11) ayırt edici metin sonradan değiştirildi; (12) `created_at`
geriye alındı (not sonradan «eski» gösterildi); (13) sonradan çapa eklendi;
(14) **kütüphane notu dosyaya** çevrildi (kapsam daraltma da serbestti); (15) `id`
değiştirildi. FK iki yazımı (olmayan emsal, olmayan dosya) ve NOT NULL bir yazımı zaten
durduruyordu; kapı bunları **yedekler** (FK'siz bağlantıda da doğru hüküm verir).

**Yazılan (şema v49, `0049_precedent_note_content_scope_guard.sql`, 2 tetikleyici).**
`precedent_notes_content_guard_bi`: `id`/`precedent_id` dolu; `precedent_id` **var olan**
emsali gösterir (emsal silme yolu yok → fail-closed, 0045/0046 kararı); `case_id_optional`
NULL ya da **silinmemiş** bir dosya (çekirdeğin doğum denetimi); `created_at` ISO-8601
damgası (`YYYY-MM-DDTHH:MM:SS…`; boş/boşluk bu kurala zaten takılır); üç metin alanı
**kırpılmış** (çekirdek `trim` yazar); dolu çapa boşluktan ibaret olamaz.
`precedent_notes_content_guard_bu`: sekiz sütunun **hepsi** donar — not append-only
sicildir; düzeltme yolu **yeni not** yazmaktır (çekirdekte de tek yol budur). Ret
`DIM_INTEGRITY:precedent_notes` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
Tablo `case_id` değil **`case_id_optional`** taşır; kural dosya zinciri değil **not
bütünlüğüdür**, bu yüzden `scope_violations()` alanı **eklenmedi** — `GUARDED_TABLES`
**25 kalır** (0044–0048 ile aynı sınıf).

**Testler.** `precedents` **+1** (`a_precedent_note_cannot_be_rewritten_or_detached_from_its_subject`):
sekiz doğum kusuru (olmayan emsal/dosya, silinmiş dosya, boş kimlik, boş/bozuk zaman,
kırpılmamış metin, boşluk çapa) `E_DB_INTEGRITY`; dokuz UPDATE yolu (kapsam iki yön,
emsal taşıma, üç metin, zaman, çapa) reddedilir; değişmeyen yazma takılmaz; tarama 0.
`upgrade_tests` **+1** (`upgrade_keeps_precedent_notes_written_before_the_content_gate`):
v48 kasasındaki kırpılmamış/tarihsiz notlar **silinmez**, UPDATE ile **onarılamaz**, yeni
bozuk yazım kapıda durur, çekirdeğin biçimi geçer. Toplam `dim-storage --lib`
**691 → 693** (yerel **693/693 yeşil, 460,97 s**). Biçim (iki manifest) / clippy
`-D warnings` / masaüstü `cargo check --all-targets` (1 dk 14 s) temiz.

**Kapı kanıtı.** **16/16 mutant KIRMIZI**, taban **20/20 RED** iken
(`~/.hermes/cache/scratch/dim-v49-probe/gates_v49.py`, log `gates_v49.log`): BI'ın sekiz
cümlesi (emsal varlığı · dosya varlığı+yaşamı · `id` dolu · damga biçimi · üç `trim`
kuralı · çapa) ve BU'nun sekiz sütunu tek tek çıkarıldı; her mutantta **yalnız hedef
saldırı** geçti. Saldırılar `PRAGMA foreign_keys=OFF` ile koştu: FK'nın zaten durdurduğu
iki yazımı kapının **kendi başına** durdurduğu görülsün. **Dürüst (ilk koşu):** (a) ölçüm
sondasının ilk sürümü sıralama hatası taşıyordu — `id` değiştiren yazım sonda değildi,
kendisinden sonraki üç UPDATE **0 satır** eşleştirdi (üç «KABUL» hükmü sahteydi); sıra
düzeltildi, taban yeniden ölçüldü, ilk koşunun hükümleri kanıt sayılmadı. (b) Kapıda
`created_at` için **iki** cümle yazmıştım (`length(trim(...))=0` **ve** `NOT LIKE` damga);
mutant denemesi birincisinin **etkisiz** olduğunu gösterdi (boş metin damga kuralına da
takılır) — yüklü olmayan cümle kapıdan **çıkarıldı**, 16/16 yeniden ölçüldü.

**Dürüst sınırlar.** (a) v49 öncesi kapsamı/metni bozuk notlar **silinmez** ve içerik
donduğu için **onarılamaz** — onarım yolu aynı emsale **yeni not** yazmaktır; temizlik
ürün kararı. (b) Kapı notun **doğruluğunu** değil **değişmezliğini** kanıtlar: yazan taraf
yalan bir notu bir kez yazabilir. (c) `useful_passage_anchor`ın hedef metinde karşılığı
**doğrulanmaz** (bugün arayüz daima `NULL` gönderir; dolu çapa yalnız biçimce denetlenir).
(d) Dosyanın yaşamı yalnız **doğuşta** denetlenir: not bağlandıktan sonra dosya
soft-delete edilebilir (tarihsel gerçek). (e) SQLite `trim()` yalnız ASCII boşlukları
kırpar; U+00A0'dan oluşan metin bu kapıdan geçer (çekirdek `str::trim` reddeder).
(f) Tetikleyicileri anahtarı olan saldırgan düşürebilir. (g) Arayüz **davranışı**
değişmedi (rozetler aynı sütunlar); gerçek pencere + görsel QA **insan bekliyor**.

**CI (ölçüldü).** kod `c55d80a` (4 dosya, **+417/−1**) → run `44047888298` **10/10 YEŞİL**
(`dim-storage` **693** iki OS'ta birebir: macos-15 **400,08 s** / ubuntu-latest **622,18 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**,
arayüz kodu değişmedi; `masaüstü kabuk`, `windows güvenli depo` 28, SBOM, sözleşmeler,
belge worker'ı, sürüm paketi manifesti ve kaynak arşivi yeşil).

## F08-T04w — maskeleme bölgesi kanıtı doğuşta tutarlı, sonrasında değişmez olmalı (şema v50, 2026-11-17)

**Neden.** `redaction_regions` satırı **maskenin kendisidir**: hangi sayfada hangi
dikdörtgenin karartıldığı. Arayüz planın altında bölge sayısını («N bölge»), açılınca
sayfa kırılımını («Sayfa 3: 2 bölge») ve geometrisi okunamayan bölge sayısını kırmızı
uyarıyla basar (`screens/Redaction.tsx`; `list_redaction_regions` → `rects: null`).
Gönderim maskesi de terimlerini bu satırlardan üretir: `outbound_terms_from_plan` yalnız
`user_decision='redact'` bölgelerin dikdörtgenleriyle kesişen bloklardan metin çıkarır.
Çekirdek bölgeyi yalnız **INSERT** eder (`add_redaction_regions`, plan `draft`ken; sonra
plan `applied` olur) — düzeltme, taşıma veya silme yolu yoktur. Şemada ise tablo 0001'den
beri **hiçbir tetikleyici taşımıyordu**: sayfa, geometri ve karar doğrudan SQL ile
serbestçe çevrilebiliyor, bölge başka plana (başka dosyaya) taşınabiliyordu.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v49; sonda
`~/.hermes/cache/scratch/dim-v50-probe/measure_v50.py`, log `measure_before.log`):**
26 kusur yazımının **24'ü KABUL** edildi (2 RED yalnız FK'nın kendisiydi): (1) bölge
**başka plana** taşındı — başka dosyanın planı; (2) sayfa numarası değiştirildi (2 → 0);
(3) dikdörtgen geometrisi değiştirildi (başka bölge taranır); (4) karar `redact`→`keep`
çevrildi (maske yolu o bölgeyi sessizce atlar); (5) kategori boşaltıldı; (6) koordinat
uzayı `image_pixels`e çevrildi; (7) terim özeti değiştirildi; (8) kimlik değiştirildi;
(9) geometri bozuk JSON'a çevrildi (arayüz «okunamadı» der); (10) negatif sayfaya bölge
yazıldı; (11) **kapanmış (`applied`) plana** yeni bölge eklendi — planın bölge sayısı
sonradan büyüdü; (12) boş kimlik; (13) geçersiz JSON; (14) nesne; (15) boş dizi;
(16) düz sayı elemanı (`[1,2,3,4]`); (17) üçlü dikdörtgen; (18) metinli koordinat;
(19) yanlış koordinat uzayı; (20) boş kategori; (21) boşluk kategori; (22) boş terim
özeti; (23) **doğuştan `keep`**; (24) **doğuştan `pending`**. FK iki yazımı (olmayan
plan, boş plan kimliği) zaten reddediyordu; kapı bunları **yedekler** (FK'siz bağlantıda
da doğru hüküm verir).

**Yazılan (şema v50, `0050_redaction_region_evidence_guard.sql`, 2 tetikleyici).**
`redaction_region_evidence_guard_bi`: `id` dolu; plan **var ve `draft`** (bölge seti plan
`applied` olmadan önce tamamlanır; kapanmış plana sonradan kanıt eklenemez); `page_index`
negatif değil; `quads` **geçerli JSON dizi**, boş değil, her elemanı **tam dört sayı**
(integer/real) — arayüzün «geometri okunamadı» saydığı satır doğuşta yazılamaz; koordinat
uzayı `pdf_page_points` (çekirdek sabit yazar); kategori kırpılmış ve boş değil;
`detected_text_hash` NULL ya da kırpılmış-boş-değil; `user_decision='redact'`.
`redaction_region_evidence_guard_bu`: sekiz sütunun **hepsi** değişmez — bölge doğduğu
anda donar. Değişmeyen yazma (`SET quads=quads`) takılmaz; düzeltme yolu yeni plandır
(çekirdekte de tek yol budur). Ret `DIM_INTEGRITY:redaction_regions` →
`StorageError::Integrity` → **`E_DB_INTEGRITY`**. Tablo `case_id` taşımaz →
`scope_violations()` alanı **eklenmedi**, `GUARDED_TABLES` **25 kalır** (0044–0049 ile
aynı sınıf).

**Testler.** `redactions` **+1** (`redaction_region_evidence_is_frozen_and_well_formed`):
15 doğum kusuru (olmayan plan, kapanmış plan, boş kimlik, negatif sayfa, geçersiz/nesne/
boş dizi/düz sayı/üçlü/metinli geometri, yanlış uzay, boş ve boşluk kategori, boş özet,
doğuştan `keep`/`pending`) `E_DB_INTEGRITY`; sekiz UPDATE yolu (plan, sayfa, geometri,
karar, kategori, uzay, özet, kimlik) reddedilir; değişmeyen yazma takılmaz; satır eski
değerleriyle okunur. `upgrade_tests` **+1**
(`upgrade_keeps_redaction_regions_written_before_the_evidence_gate`): v49 kasasındaki
bozuk bölgeler **silinmez**, UPDATE ile **onarılamaz**, kapanmış plana sonradan bölge
eklenemez. Toplam `dim-storage --lib` **693 → 695** (yerel **695/695 yeşil, 457,94 s**).
Biçim (iki manifest) / clippy `-D warnings` temiz.

**Kapı kanıtı.** **22/22 mutant KIRMIZI**, taban **24/24 saldırı RED + meşru yazma KABUL**
iken (`~/.hermes/cache/scratch/dim-v50-probe/gates_v50.py`, log `gates_v50.log`): BI'ın on
dört cümlesi ve BU'nun sekiz sütunu tek tek çıkarıldı; her mutantta **yalnız hedef
saldırı** geçti. Ölçüm sonrası (v50): 26 saldırının **26'sı RED**, dört meşru yazımın
dördü KABUL (`measure_after.log`).

**Dürüst sınırlar.** (a) v50 öncesi tutarsız/bayat bölgeler **silinmez** ve içerik
donduğu için **onarılamaz** — temizlik ürün kararı. (b) Kapı bölgenin **gerçekten
maskelendiğini** doğrulamaz: plan `draft`ken sahte ama biçimce geçerli bir bölge
eklenebilir; kapı sonraki değişikliği ve kapanmış plana eklemeyi durdurur. (c) `quads`
koordinatlarının sayfa sınırları içinde olduğu doğrulanmaz. (d) `detected_text_hash`in
**içeriği** doğrulanmaz (SQLite'ta SHA-256 yok). (e) `plan_id` boşsa zaten var olan
plan FK'sine takılır; ayrı bir «boş olamaz» cümlesi yazılmadı. (f) Tetikleyicileri
anahtarı olan saldırgan düşürebilir. (g) Arayüz **davranışı** değişmedi; gerçek pencere
+ görsel QA **insan bekliyor**.

**CI (ölçüldü).** kod `45708a7` (4 dosya, **+595/−4**) → run `44094362533` **10/10 YEŞİL**
(`dim-storage` **695** iki OS'ta birebir: macos-15 **454,57 s** / ubuntu-latest **620,74 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**,
arayüz kodu değişmedi; `masaüstü kabuk`, `windows güvenli depo` 28, SBOM, sözleşmeler,
belge worker'ı, sürüm paketi manifesti ve kaynak arşivi yeşil). Yerel doğrulama da aynı
sonucu verdi: `cargo test -p dim-storage --lib` **695/695, 457,94 s**; iki manifestte
`cargo fmt --check` ve `clippy -D warnings` temiz; masaüstü `cargo check --all-targets`
temiz (0,75 s, önbellekli).

## F08-T04x — maskeleme planının kanıt bağı doğuşta tutarlı, sonrasında değişmez olmalı (şema v51, 2026-11-17)

**Bu turun özel durumu.** Önceki dört koşu (18:12, 19:58, 21:09, 22:02) API
kesintisi/zaman aşımıyla düştü; iş **ağaçta kalmıştı** — `0051` göçü + dört crate dosyası
yazılmış, ölçüm üretilmiş, ama **commit edilmemiş**, tam paket hiç yeşil ölçülmemiş
(`libtest_v51_full.log` 18:12'de `EXIT=143` ile kesik) ve mutasyon kapısı hiç yazılmamıştı.
Bu tur: işi denetledim, eksik kapıyı yazdım, tam paketi **kendim koştum** (697/697,
474,08 s), fmt/clippy/masaüstü check temiz, commit + push + CI. Tur başında
`HEAD == origin/main` = `c118be8`; disk **6,9 G**; ağaçta ayrıca **başka ajanın 9 arayüz
dosyası** (2026-11-07'den beri) — dokunulmadı. `cargo fmt --all` yalnız benim Rust
dosyalarıma değdi.

**Neden bu dilim.** `redaction_plans` satırı maskelemenin **künyesidir**: hangi belge
sürümünün hangi render'ına ait olduğu (`render_hash`), hangi çıktıyı ürettiği
(`output_version_id` + silinme izi `output_version_ref`) ve kapanıp kapanmadığı (`state`).
Üç okuma yolu bu sütunlara yaslanır: (i) `screens/Redaction.tsx` render özetinin ilk 16
hanesini basar (`plan.renderHash`) — «bu geometri **bu** render'a ait» iddiası ekranda
görünür; (ii) yeniden inceleme nedeni `render_changed` **yalnız** bu sütunla
`document_versions.original_sha256` karşılaştırılarak hesaplanır — özet çevrilebilirse
uyarı sessizce kaybolur; (iii) `output_link_state` (`linked`/`removed`) bağdan ve
**izden** okunur; 0029 «çıktı vardı, sonra silindi» ile «hiç bağlanmadı»yı ayırmak için
yazıldı — iz sahtelenebilirse ayrım anlamsızlaşır. Çekirdek planı **yalnız**
`create_redaction_plan` ile doğurur ve `render_hash` olarak kaynak sürümün
`original_sha256`sini yazar; `output_version_id` verilmişse çıktının kaynağın **alt
sürümü** olduğunu doğrular. Sonrasında tek meşru yazma `add_redaction_regions`ın
`draft`→`applied` geçişidir. Şemada ise plan tablosunun kapısı yalnız **kapsam**tı
(0031/0034) — kanıt sütunları ve kapanış serbestti.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v50; sonda
`~/.hermes/cache/scratch/dim-v51-probe/measure_v51.py`, log `measure_before.log`):**
26 kusur yazımının **26'sı KABUL** (meşru 8 yazma da KABUL — taban 34/34):
doğuşta `render_hash` kaynak sürümün özeti **değil** (`'h'`, hatta başka 64 hex) — renew
«render değişti» uyarısı hiç doğmaz; koordinat uzayı `page`; çıktı profili boş; boşluk;
`plan_version=0`; `created_at=''`; `updated_at=''`; çıktı izi **başka** kimlik; bağ dolu
iken iz NULL (silinme izi yok); **çıktı = kaynağın kendisi**; boş plan kimliği; bağ yokken
**uydurma** iz; `render_hash` UPDATE ile çevrildi (uyarı susturuldu); koordinat uzayı,
çıktı profili ve plan sürümü çevrildi; `created_at` geriye alındı; plan **aynı dosyada
başka belgeye** taşındı (kapsam kapısı aynı dosyayı serbest bırakır) — «hangi belgede
maskeleme yapıldı» yanıtı değişti; plan başka belgenin **ayrıştırmasına** bağlandı; sicil
satırı **başka dosyaya** taşındı (`case_id`+sürüm birlikte); çıktı bağı aynı dosyada
**başka sürüme** çevrildi; çıktı bağı **sonradan dolduruldu**; çıktı **izi** başka kimliğe
çevrildi; **kapanmış plan yeniden açıldı** (`applied`→`draft`) — v50'nin «kapanmış plana
bölge eklenemez» cümlesi bu geçişle boşa düşüyordu; `applied`→`approved`.

**Yazılan (şema v51, `0051_redaction_plan_evidence_guard.sql`, 2 tetikleyici).**
`redaction_plan_evidence_guard_bi`: `id` kırpılmış-dolu; `render_hash` **kaynak sürümün
`original_sha256`si** (kanıt bağı: geometri bu render'a aittir); koordinat uzayı
`pdf_page_points` (çekirdek sabit yazar, v50 bölge kapısı da aynı uzayı ister); çıktı
profili kırpılmış-dolu; `plan_version ≥ 1`; `created_at`/`updated_at` kırpılmış-dolu;
`output_version_ref` **ya NULL ya da `output_version_id` ile aynı** (silinme izi bağın
kimliğidir, başka kimlik değil); çıktı bağı kaynak sürümün **kendisi olamaz** (çekirdek
de reddeder). `redaction_plan_evidence_guard_bu`: `id`, `case_id`,
`document_version_id`, `parse_run_id`, `render_hash`, `coordinate_space`,
`output_profile`, `plan_version`, `created_at`, `output_version_ref` **donar**;
`output_version_id` yalnız **NULL'a çözülebilir** (0028'in `ON DELETE SET NULL`ı: çıktı
silinince bağ çözülür, iz kalır) — başka sürüme çevrilemez, NULL'dan doldurulamaz.
`state='applied'` **terminaldir**: uygulanmış maskeleme yeniden açılamaz (v50 kapanışı
buna dayanır); diğer geçişler, `reviewer` ve `updated_at` serbest. Değişmeyen yazma
takılmaz. Ret `DIM_INTEGRITY:redaction_plans` → `StorageError::Integrity` →
**`E_DB_INTEGRITY`**. Tablo `case_id` taşıyor ve zaten taranıyor → `GUARDED_TABLES`
**25 kalır**.

**Testler.** `redactions` **+1** (`redaction_plan_evidence_is_bound_at_birth_and_frozen`):
**13 doğum kusuru** (boş kimlik, `'h'` ve başka 64 hex render özeti, `page` uzayı, boş ve
boşluk profil, `plan_version=0`, boş `created_at`/`updated_at`, iz başka kimlik, iz NULL,
çıktı = kaynak, uydurma iz) `E_DB_INTEGRITY`; **11 UPDATE yolu** (render özeti, uzay,
profil, plan sürümü, `created_at`, belge sürümü, ayrıştırma koşusu, çıktı bağını sonradan
doldurma, çıktı bağını başka sürüme çevirme, `applied`→`draft`, `applied`→`approved`)
reddedilir; değişmeyen yazma, `reviewer` ve `updated_at` takılmaz. Kimlik ve `case_id`
donması testte ayrıca ölçülmez (kapı kanıtında iki mutantla ölçüldü). `upgrade_tests` **+1**
(`upgrade_keeps_redaction_plans_written_before_the_evidence_gate`): v50 kasasındaki
tutarsız planlar **silinmez**, UPDATE ile **onarılamaz**, yeni tutarsız plan doğamaz.
Toplam `dim-storage --lib` **695 → 697** (yerel **697/697 yeşil, 474,08 s**). **Test
düzeltmesi (dürüst):** mevcut fikstürler `render_hash: "abc"`/`"b".repeat(64)` gibi
kaynağın özeti olmayan değerlerle ve izsiz çıktı bağıyla yazıyordu → kaynak sürümün
özeti + `output_version_ref` eklendi; iki test v51 kapısını **bilinçli düşürür** (kapının
kendi kuralını değil eski kasa görüntüsünü ölçtükleri yorumda yazılı). Biçim (iki
manifest) / clippy `-D warnings` / masaüstü `cargo check --all-targets` temiz.

**Kapı kanıtı.** **21/21 mutant KIRMIZI**, taban **23/23 saldırı RED + 8/8 meşru yol
KABUL** iken (`~/.hermes/cache/scratch/dim-v51-probe/gates_v51.py`, log `gates_v51.log`):
BI'ın dokuz cümlesi ve BU'nun on iki cümlesi tek tek nötrlendi; her mutantta **yalnız
hedef saldırı** geçti. Ölçüm sonrası (v51): 26 saldırının **26'sı RED**, sekiz meşru
yazımın sekizi KABUL (`measure_after.log`). **Dürüst (ilk koşu 19/21):** iki mutant YEŞİL
kaldı çünkü hedef saldırı **başka bir cümleye** takılıyordu — (i) yalnız `case_id`
taşıması `document_version_id` cümlesine ve 0031 kapsam kapısına, (ii) yalnız
`output_version_id` değişimi `output_version_ref` cümlesine. Saldırılar izole edildi
(yalnız `case_id`, kapsam kapısı bilinçli düşürülmüş; yalnız `output_version_id`, iz sabit)
→ **21/21** yeniden ölçüldü; ilk koşunun 19 hükmü kanıt sayılmadı.

**Dürüst sınırlar.** (a) v51 öncesi tutarsız planlar **silinmez** ve donduğu için
**onarılamaz** — temizlik ürün kararı. (b) Kapı `render_hash`in gerçekten o render'ın
özeti olduğunu **yeniden hesaplamaz** (SQLite'ta SHA-256 yok): doğuşta kaynak sürümün
sütunuyla **eşitlik** ölçülür; sürümün özeti de yanlışsa plan onu devralır. (c)
`document_versions.original_sha256` biçimi (64 hex) burada denetlenmez. (d)
`output_version_id`nin kaynağın **alt sürümü** olduğu bu kapıda doğrulanmaz (çekirdek
doğrular; kapsam kapısı yalnız aynı dosyayı ister) — bilinçli: geçmiş fikstürlerde alt
sürüm bağı her yerde kurulu değil. (e) Doğuşta `state` serbesttir: bölgesiz `applied` bir
plan «uygulandı» der — kapanış kapısı bunu **durdurmaz**, yalnız sonradan açılmasını
durdurur; geçmiş kasaları taklit eden yükseltme fikstürleri bu yüzden serbest kalmalı.
(f) `updated_at ≥ created_at` sırası denetlenmez (yalnız ikisi de dolu). (g)
Tetikleyicileri anahtarı olan saldırgan düşürebilir. (h) Arayüz **davranışı** değişmedi;
gerçek pencere + görsel QA **insan bekliyor**.

**CI (ölçüldü).** kod `76c68ca` (5 dosya, **+732/−32**) → run `44174856142` **10/10
YEŞİL** (`dim-storage` **697** iki OS'ta birebir: macos-15 **416,70 s** / ubuntu-latest
**818,71 s**; `arayüz` **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**,
arayüz kodu değişmedi; `masaüstü kabuk`, `windows güvenli depo`, SBOM, sözleşmeler, belge
worker'ı, sürüm paketi manifesti ve kaynak arşivi yeşil).


## F08-T04y — belge sürümünün kimlik kanıtı doğuşta tutarlı, sonrasında değişmez olmalı (şema v52, 2026-11-18)

**Neden bu dilim.** `document_versions` satırı **kanıt zincirinin köküdür**: hangi baytlar
(`blob_id`), o baytların özeti (`original_sha256`), boyutu (`byte_size`), türü
(`detected_mime`) ve ne zaman alındığı (`imported_at`). Üstündeki bütün kanıt kapıları bu
sütuna yaslanır: (i) `redaction_plans.render_hash` doğuşta **bu özetle eşit** olmak zorunda
(0051) — «geometri **bu** render'a aittir» iddiasının dayanağı budur; (ii) yeniden inceleme
nedeni `render_changed` (`v.original_sha256 <> p.render_hash`) **yalnız** bu sütunla
hesaplanır (`get_redaction_plan`/`list_redaction_plans`, `screens/Redaction.tsx`); (iii)
`documents.find_version_by_sha` aynı baytın yeniden içe aktarılmasını bu sütunla tanır —
özet çevrilebilirse aynı dosya iki kez içe alınır; (iv) `parsing` blob'u sürümün künyesiyle
(`detected_mime`, `byte_size`) açar. v51'in kapanış sınırı (c) bunu **açıkça** bırakmıştı:
«`document_versions.original_sha256` biçimi (64 hex) burada denetlenmez». Şemada bu tablonun
kapısı yalnız **kapsam**tı (0031/0034/0035) ve `original_sha256` için yalnız
`CHECK (length = 64)` vardı. Tur başında `HEAD == origin/main` = `b5e064c`; disk **5,9 G**;
ağaçta yalnız **başka ajanın 9 arayüz dosyası** (2026-11-07'den beri) — dokunulmadı.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v51; sonda
`~/.hermes/cache/scratch/dim-v52-probe/measure_v52.py`, log `measure_before.log`):**
23 yazımın **17'si kusurdu ve 17'si KABUL** (1 yazım zaten RED — yalnız `length` CHECK'i — ve
5 meşru yazma KABUL): doğuşta özet **64 hex değil** (`'Z'*64`), **büyük harf** hex
(`'A'*64` — `find_version_by_sha` eşleşmez), algılanan mime boş / yalnız boşluk /
**kırpılmamış** (`' application/pdf'`), içe aktarma damgası boş / **ISO değil** (`'dün'`),
**boş sürüm kimliği**; özet UPDATE ile çevrildi (planın bağlandığı render sessizce
başkalaştı), `blob_id` **başka baytlara** çevrildi (özet ile dosya ayrıştı), `byte_size`,
`detected_mime` (`application/pdf` → `text/plain`), `imported_at`, `is_original` (1→0),
`version_no`, `original_filename_encrypted` sonradan eklendi, `id` çevrildi.

**Yazılan (şema v52, `0052_document_version_identity_guard.sql`, 2 tetikleyici).**
INSERT — `id` kırpılmış-dolu; `original_sha256` **64 küçük harf hex** (`length` CHECK'i 64'ü
zorlar, burada karakter kümesi denetlenir); `detected_mime` kırpılmış ve dolu; `imported_at`
ISO-8601 damgası (`YYYY-MM-DDTHH:MM:SS…`, 0049 ile aynı biçim). UPDATE — `id`, `version_no`,
`blob_id`, `original_sha256`, `byte_size`, `detected_mime`, `imported_at`, `is_original`,
`original_filename_encrypted` **donar**; değişmeyen yazma takılmaz. Ret
`DIM_INTEGRITY:document_versions` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
**Çekirdek hizası:** `add_version_with_parent` özeti `is_ascii_hexdigit()` ile kabul ediyordu
(büyük harf geçerdi → aynı bayt iki farklı özetle yazılır, şema kapısında `E_DB_INTEGRITY`
ile düşerdi); kapı **küçük harf hex**e çekildi, ret `InvalidSetting` (`E_DB_SETTING`) —
yazma hiç denenmez, şema hatası görünmez. Tablo `case_id` taşıyor ve **zaten** taranıyor →
`GUARDED_TABLES` **25 kalır**.

**Testler.** `documents` **+1** (`document_version_identity_is_frozen_and_well_formed`:
8 doğum kusuru + 9 UPDATE yolu reddedilir, no-op ve ebeveyn çözülmesi takılmaz, saldırılar
sonrası satır **hiç değişmemiş**; üstüne çekirdeğin büyük harfi `E_DB_SETTING` ile
reddettiği), `upgrade_tests` **+1**
(`upgrade_keeps_document_versions_written_before_the_identity_gate`: v51'de yazılmış
tutarsız sürüm **silinmez ve sessizce düzeltilmez**, donar; yeni tutarsız satır doğamaz)
→ `dim-storage --lib` **697 → 699**; yerelde **699/699 yeşil, 527,92 s**; fmt (iki manifest)
/ clippy `-D warnings` / masaüstü `cargo check --all-targets` (2 dk 11 s) temiz.
**Test düzeltmesi (dürüst):** `redactions`'ın iki testi (`plan_bound_to_another_render_is_rejected`,
`render_mismatch_requires_reinspection`) sürümün özetini **doğrudan UPDATE** ile çeviriyordu;
v52 bunu dondurduğu için testlerde tetikleyici **bilinçli düşürüldü** (`DROP TRIGGER`, niyet
yorumda) — okuma davranışı ölçülmeye devam ediyor, uyuşmazlık artık yalnız **v52 öncesi**
satırlarda doğabilir.

**Kapı kanıtı.** Taban **8/8 yeşil** (mutasyonsuz, filtre `identity`), ardından **15/15
mutant KIRMIZI** (`~/.hermes/cache/scratch/dim-v52-probe/gates_v52.py`, log `gates_v52.log`):
BI'ın beş ve BU'nun dokuz cümlesi tek tek nötrlendi, ayrıca çekirdeğin küçük-harf kapısı
(`is_ascii_hexdigit`e geri alındı → test kırmızı). Her mutantta **yalnız hedef test** düştü
(yalnız `original_sha256` donması iki testi birden düşürür: kimlik testi + yükseltme testi).
Kaynak **byte-birebir** geri yazıldı (`sha256` önce = sonra). Ölçüm sonrası (v52): 18 yazımın
**18'i RED** (17 hedef kusur + CHECK'in kendi reddi), 5 meşru yazımın beşi KABUL
(`measure_after.log`). **Dürüst (ilk koşu 0/15):** betiğin "derleme hatası" kapısı
`error: test failed` satırını da yakalıyordu → 15 mutantın hepsi **ÖLÇÜLEMEDİ** sayıldı ve
hiçbiri kanıt olmadı; ayrıştırma `could not compile` / `error[E…]` ile sınırlandı, filtre
`identity`ye genişletildi (yükseltme testi de seçilsin), **15/15** yeniden ölçüldü.

**CI (ölçüldü).** kod `4c4a3e3` (5 dosya, **+424/−4**) → run `44193152146` **10/10 YEŞİL**
(`dim-storage` **699** iki OS'ta birebir: macos-15 **438,17 s** / ubuntu-latest **818,63 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**,
arayüz kodu değişmedi; `masaüstü kabuk`, `windows güvenli depo`, SBOM, sözleşmeler, belge
worker'ı, sürüm paketi manifesti ve kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) v52 öncesi tutarsız sürümler **silinmez** ve artık **onarılamaz**
(donmuş) — temizlik ürün kararı. (b) Kapı özetin gerçekten **baytların** SHA-256'sı olduğunu
doğrulamaz: `blobs` özet taşımaz ve SQLite'ta SHA-256 yok; ölçülen **biçim + değişmezlik**tir.
(c) `document_id` ve `case_id` bu kapının dışında: sütunlar **kapsam** tetikleyicilerinin
(0031/0035) ve sürümün **aynı dosyada başka belgeye taşınması** orada bilinçli serbest
(`scope_guard.rs` testi bunu sabitler) — bu turda o sözleşme değiştirilmedi. (d)
`parent_version_id` de kapsamın (0034) ve `ON DELETE SET NULL`ın sütunu: doldurma/çevirme
burada durdurulmaz. (e) `byte_size`ın `blobs.encrypted_size` ile ilişkisi denetlenmez
(şifreleme zarfı farkı ölçülmez). (f) SQLite `trim()` yalnız ASCII boşluk kırpar (U+00A0
artığı geçer). (g) Doğuşta `imported_at` **gelecekte** olabilir (sıra/zaman denetimi yok).
(h) `render_changed` uyarısı v52 sonrası **yeni** veride ulaşılamaz (özet dondu) — eski/
onarılmamış satırlar ve kapının bilinçli düşürüldüğü okuma testleri için kalır. (i)
Tetikleyicileri anahtarı olan saldırgan düşürebilir. (j) Arayüz **davranışı** değişmedi;
gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04z — kaynak bağının kanıtı doğuşta tutarlı, sonrasında değişmez olmalı (şema v53, 2026-11-18)

**Neden bu dilim.** `source_anchors` satırı **kalıcı kaynak bağıdır** (F06-T09): bir alıntının
hangi belge sürümünde, hangi ayrıştırma koşusunda, hangi segmentte ve hangi Unicode scalar
aralığında durduğunu söyler. Kasadaki bütün kanıt yolları bu satıra yaslanır: `locate_anchor`
alıntıyı **segment metninde** bu kayıttaki ofsetle arar ve `quote_intact` yalnız bu satırın
alanlarıyla hesaplanır; okuyucunun «kaynağa git» bağı fiziksel sayfayı `page_id`den çözer;
`rebind_suggestion` alıntıyı güncel sürümde `exact_quote` ile arar — yanlış alıntı metni
«bulunamadı» hükmünü yanıltır; metin seçimi (`mark_from_selection`, `note_from_selection`) ve
AI aday terfisi (`ensure_anchor`) aynı `create_anchor` yolundan geçer; `claims`/`evidence_links`/
`event_sources`/`tasks` bu kimliğe bağlanır (0032) ve not/inceleme JSON listeleri bu kimlikleri
taşır (0033). Şemada bu tablonun kapısı yalnız **kapsam**tı (0031: sürüm/koşu/segment aynı
dosyada, sayfa aynı koşuda) ve `quote_hash` için yalnız `CHECK (length = 64)` vardı. Tur
başında `HEAD == origin/main` = `3246b07`; disk **6,1 G**; ağaçta yalnız **başka ajanın 9
arayüz dosyası** (2026-11-07'den beri) — dokunulmadı.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v52; sonda
`~/.hermes/cache/scratch/dim-v53-probe/measure_v53.py`, log `measure_before.log`):**
40 yazımın **27 kusuru 27/27 KABUL** (3'ü zaten RED — biri 0031 kapsam, biri `length = 64`
CHECK'i, biri 0031 segment kapsamı — ve 10 meşru yazma KABUL): doğuşta özet **64 hex değil**
(`'Z'*64`), **büyük harf** hex (`'A'*64`), alıntı **boş**, alıntı **yalnız boşluk**, alıntı
uzunluğu ofset aralığıyla **uyuşmuyor** (15 ↔ 16), alıntı **iddia edilen ofsette değil**
(0..16 yazıldı, alıntı gerçekte 8..24'te — okuyucu yanlış aralığı vurgular, `locate_anchor`
«kopmuş» der ama kayıt bunu **yanlış konumla** söyler), `page_id` aynı koşuda ama **segmentin
sayfası değil** (p3 ≠ p1), `document_version_id` **koşunun sürümü değil** (v2 ↔ r1/v1 — bağ
başka sürümün ekranında görünür), **boş çapa kimliği**, oluşturma damgası **boş**, güncelleme
damgası **ISO değil** (`'dün'`), çapa **doğuştan `rebound`** («yeniden doğrulanmış» iddiası),
çevre **öneki uydurma**, çevre **soneki uydurma**, alıntı **başka segmentin metninden** (s3);
UPDATE ile alıntı metni, özet, başlangıç/bitiş ofseti, sayfa, sürüm, çapa kimliği, oluşturma
damgası, önek ve sonek çevrildi; **doğrulanmamış çapa terfi ettirildi**: `active`→`unresolved`
(dürüst düşürme) sonra **konum değişmeden** `active` — «yeniden doğrulandı» iddiası hiçbir
doğrulama olmadan geri gelir.

**Yazılan (şema v53, `0053_source_anchor_evidence_guard.sql`, 2 tetikleyici).** BI — `id`
kırpılmış-dolu; `anchor_status` **`active`** (doğuşta yeniden doğrulanmış/çözülememiş doğamaz —
`create_anchor` daima `'active'` yazar); `exact_quote` boşluktan ibaret değil ve
`length(exact_quote) = end_offset - start_offset` (SQLite `length()` metinde **karakter**
sayar = çekirdeğin `chars().count()`u); `quote_hash` **64 küçük harf hex** (CHECK 64'ü zorlar,
karakter kümesi burada — 0052'nin `original_sha256` deseni); iki damga ISO-8601 (0049 ile aynı
biçim); `document_version_id` **koşunun sürümü**; `page_id` **segmentin sayfası** (NULL'lar `IS`
ile eşleşir); `exact_quote` **segment metninde tam o ofsette**; `prefix`/`suffix` **segment
metninden** üretilmiş çevre (yarıçap 24 — `context_around(..., 24)` sözleşmesi; start=0 → önek
boş, metin sonu → sonek boş). BU — `id`, `exact_quote`, `quote_hash`, `created_at` **donar**
(yeniden bağlama aynı alıntıyı taşır; farklı alıntı yeni satırdır); `updated_at` değişiyorsa
ISO-8601 olmalı. **Terfi kapısı:** doğrulanmamış (`active`/`rebound` dışı) bir çapa **konumu
değişmeden** yeniden `active`/`rebound` olamaz (dürüst düşürme serbest; gerçek yeniden bağlama
konumu değiştirir ve tutarlılık kapısından geçer). **Tutarlılık kapısı:** konum/çevre
sütunlarından biri değişiyorsa `document_version_id` koşunun sürümü, `page_id` segmentin
sayfası, uzunluk = ofset farkı ve `exact_quote`/`prefix`/`suffix` **yeni segment metniyle**
birebir olmalı — yeniden bağlama ancak alıntının gerçekten yeni konumda durduğu yazımı geçer;
değişmeyen yazma takılmaz. Ret `DIM_INTEGRITY:source_anchors` → `StorageError::Integrity` →
**`E_DB_INTEGRITY`**. Tablo `case_id` taşıyor ve **zaten** taranıyor → `GUARDED_TABLES` **25
kalır**; saf **kapsam** ihlalleri bilinçli olarak 0031'in sınıfında bırakıldı (iki kapı da aynı
yazımı reddederdi, ret sınıfı tetikleyici sırasına bağlı kalırdı).

**Çekirdek hizası.** `create_anchor` alıntıyı `verify_quote` ile doğruluyordu; bu işlev **ipucu
tutmazsa tüm segmenti tarar** ve tek eşleşmede `Exact` döner — yani çağıran yanlış `start_offset`
verse de satır yazılıyordu (ölçülen kusur: 0..16 aralığına 8..24'teki alıntı yazılabiliyordu).
`quote_starts_at` yardımcısı **iddia edilen konumda birebir eşleşme** ister; `rebind_anchor`ın
zaten yaptığı kontrol aynı tanıma alındı. Yeni ret **`E_QUOTE_NOT_FOUND`** (yazma hiç
denenmez), şema kapısı `E_DB_INTEGRITY` (doğrudan SQL yolu).

**Testler.** `anchors` +1 (`source_anchor_evidence_is_coherent_at_birth_and_identity_is_frozen`:
15 doğum kusuru + 14 UPDATE yolu reddedilir, no-op/tutarlı yeniden bağlama/meşru doğuş geçer),
`upgrade_tests` +1 (`upgrade_keeps_source_anchors_written_before_the_evidence_gate`: v52
tutarsız çapa silinmez/düzeltilmez, alıntı kimliği donar, yeni tutarsız doğamaz) →
`dim-storage --lib` **699 → 701**; yerelde **701/701 yeşil, 500,94 s**; fmt (iki manifest) /
clippy `-D warnings` / masaüstü `cargo check --all-targets` (1 dk 26 s) temiz. **Test düzeltmesi gerekmedi:** doğrudan SQL ile çapa yazan mevcut fikstürler (scope_guard
zincirleri, ai yabancı zinciri, maliyet testi) v53 kapısıyla çakışmadı — tam paket
**701/701** yeşil; çakışan olsaydı kapı gevşetilmeyecek, fikstür düzeltilecekti.

**Kapı kanıtı.** Taban **2/2 yeşil** (mutasyonsuz, filtre `source_anchor`), ardından **25/25
mutant KIRMIZI** (`~/.hermes/cache/scratch/dim-v53-probe/gates_v53.py`, log `gates_v53.log`):
BI'ın 12 cümlesi ve BU'nun 12 cümlesi tek tek nötrlendi, ayrıca çekirdeğin `quote_starts_at`
kapısı. Kaynak **byte-birebir** geri yazıldı (`sha256` önce = sonra:
`0053…sql 7433a130cd1ba1c0…`, `anchors.rs 0aa342d347a870d1…`). Ölçüm sonrası (v53): 40 yazımın
**30'u RED** (27 hedef kusur + zaten RED olan 3), 10 meşru yazımın onu KABUL
(`measure_after.log`) — dürüst düşürme (`'active' → 'ambiguous'`), tutarlı yeniden bağlama
(v2/r2/s2, `'rebound'`), yalnız `updated_at` tazeleme ve başta/sonda boşluklu alıntılar dahil.

**CI (ölçüldü).** Kod `e7e6180` (4 dosya, **+915/−8**) → run `44269861491` **10/10 YEŞİL**
(`dim-storage` **701** iki OS'ta birebir: macos-15 **414,43 s** / ubuntu-latest **806,39 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**, arayüz
kodu değişmedi; `masaüstü kabuk`, `windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı,
sürüm paketi manifesti ve kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) v53 öncesi tutarsız çapalar **silinmez**; alıntı kimliği donduğu için
eski satırlarda değiştirilemez, onarım yolu **yeni çapa** yazmaktır — temizlik ürün kararı.
(b) Kapı `quote_hash`in **alıntının** SHA-256'sı olduğunu doğrulamaz (SQLite'ta SHA-256 yok) —
biçim + alıntı/konum tutarlılığı ölçülür; özet ile metni birlikte yazan yol tutarlı görünen
sahtecilik üretir. (c) Tutarlı bir «sessiz yeniden bağlama» (segment + ofset + çevre hep
birlikte, alıntı yeni segmentte gerçekten duruyorsa) şemadan ayırt edilemez; `rebind_anchor`ın
denetim kaydı (`audit_events`) zorunluluğu şemada değil çekirdektedir. (d) SQLite `trim()`
yalnız ASCII boşluk kırpar; yalnız U+00A0'dan oluşan alıntı bu kapıdan geçer (çekirdek
`str::trim` reddeder). (e) SQLite `length()`/`substr()` NUL'da durur; PDF metninde NUL
beklenmez, bu sınır ölçülmedi. (f) Çevre yarıçapı **24** şemada sözleşme olarak durur; çekirdek
yarıçapı değiştirirse göç de değişmelidir (tek kaynak `context_around` çağrısı). (g)
`text_segments` metni doğrudan UPDATE ile değiştirilirse eski çapalar «kopmuş» görünür
(`locate_anchor` bunu dürüstçe bildirir); bu kapı satırı silmez. (h) Korunan sütunlar
`anchor_status`/`updated_at` dışında tümüyledir; durum geçişleri yalnız **terfi** yönünde
kapılıdır. (i) Tetikleyicileri anahtarı olan saldırgan düşürebilir. (j) Arayüz **davranışı**
değişmedi; gerçek pencere + görsel QA **insan bekliyor**. (k) Saf **kapsam** ihlalleri
(segment/sayfa başka koşuda, koşu başka dosyada) bilinçli olarak **0031'in sınıfında** kalır:
bu kapı yalnız aynı koşudaki satırın kanıtını ölçer.

## F08-T04aa — maskeleme bölgesi sayfanın kanıtıdır: var olan sayfa, pozitif alan, kırpma kutusu (şema v54, 2026-11-18)

**Neden bu dilim.** `redaction_regions` satırı **maskenin kendisidir**: hangi sayfada
hangi dikdörtgenin karartıldığı. 0050 bu satırın biçimini ve değişmezliğini kapıladı, ama
kapanış sınırı (c) açıkça şunu bıraktı: «`quads` koordinatlarının sayfa sınırları içinde
olduğu veya sayfa sayısıyla tutarlılığı doğrulanmaz». Oysa ekran plan kartında «N bölge»
ve sayfa kırılımını («Sayfa 5: 2 bölge») doğrudan bu satırlardan basar;
`outbound_terms_from_plan` bölgenin `page_index`ini planın maskede kullanacağı ayrıştırma
koşusunda arar — sayfa yoksa maskeleme **komple** reddedilir; `block_terms` yalnız
bölgeyle kesişen bloklardan terim üretir — sayfa dışına taşan kutu hiçbir metinle
kesişmez («kesişen metin bloğu yok» hatası), sayfayı aşan kutu ise gereğinden fazla
maskeler. Kusur sessiz sızıntı değil **kanıt yalanıdır**: plan «sayfa 5'i maskeledim»
der, oysa belge 2 sayfadır. Tur başında `HEAD == origin/main` = `5349c9a`; disk **4,7 G**;
ağaçta yalnız **başka ajanın 9 arayüz dosyası** (2026-11-07'den beri) — dokunulmadı.
**Önceki koşunun artığı:** v53 işi ağaçta commit'lenmiş ama durum dosyası yazılamadan
koşu düşmüştü; bu tur v53'ün **iki CI'sını** (`44269861491`, `44272796359`) **10/10 yeşil**
ölçtü ve durum dosyasını tamamladı.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v53; sonda
`~/.hermes/cache/scratch/dim-v54-probe/measure_v54.py`, log `measure_before.log`):**
25 yazımın **12 kusuru 12/12 KABUL** (11 meşru yazma KABUL; 2 yazım zaten RED — kapanmış
plan ve 0050'nin UPDATE dondurması): koşuda olmayan sayfa (3 ve 9); planın **bildirdiği**
koşuda olmayan ama en yeni koşuda olan sayfa (2); ters dikdörtgen (`x0 > x1`); sıfır
genişlik; sıfır yükseklik; genişliği aşan kutu (900 > 595); negatif x (−50); negatif y
(−50); yüksekliği aşan kutu (900 > 842); iki dörtlüden yalnız biri taşan; 2 punto taşma;
−1,5 punto negatif taşma.

**Yazılan (şema v54, `0054_redaction_region_page_bound_guard.sql`, tek tetikleyici —
0050'nin INSERT kapısı `DROP TRIGGER` + `CREATE TRIGGER` ile genişletildi; aynı sütun
kümesinde ikinci bir BEFORE INSERT tetikleyicisi sıra belirsizliği ve farklı hata sınıfı
üretirdi).** 0050'nin bütün cümleleri aynen durur, üstüne üç cümle eklenir:
1. **Sayfa var olmalı:** bölge, planın maskede kullanılacak koşusunda var olan bir
   `page_index` göstermeli — plan `parse_run_id` **bildirdiyse o**, bildirmediyse
   çekirdeğin seçtiği **en yeni sayfalı koşu** esastır; sayfa kümesi yalnız
   **tamamlanmış** (`status='succeeded'`) koşuda belgelenmiş sayılır (yarım kalmış
   koşuda eksik sayfa «yalan» değildir).
2. **Dikdörtgen pozitif alanlı olmalı:** her dörtlüde `x0 < x1` ve `y0 < y1` —
   ters/çakışık kutu bugüne dek `rects_overlap`ın min/max normalleştirmesi yüzünden
   sessizce kabul ediliyordu.
3. **Kutu sayfanın içinde kalmalı:** sayfanın `width`/`height` bilinen ekseninde her
   dörtlü `[−1, width+1] × [−1, height+1]` aralığında. **1 punto yuvarlama payı
   bilinçli:** önizleme ölçeği `72/dpi` ve piksel yuvarlaması, görüntü kenarına dayanan
   seçimde kutuyu bir puana kadar taşırabilir; pay gerçek taşmayı (≥2 punto) yakalar.
   Boyut NULL olan eksen karşılaştırmada NULL'a düşer ve **denetlenmez** — ayrı bir
   `IS NOT NULL` cümlesi etkisiz olurdu, yazılmadı.
Değişmeyen yazma takılmaz; UPDATE kapısı 0050'deki gibi sekiz sütunu dondurmaya devam
eder (bu göç ona dokunmadı). Ret `DIM_INTEGRITY:redaction_regions` →
`StorageError::Integrity` → **`E_DB_INTEGRITY`**. Tablo `case_id` taşımaz →
`GUARDED_TABLES` **25 kalır**.

**Çekirdek hizası.** Ek kapı gerekmedi: `add_redaction_regions` bölgeleri ya worker'ın
**gerçek isabetlerinden** (terim maskesi; sayfa/rect PDF'ten gelir) ya kullanıcının
önizlemede çizdiği dikdörtgenlerden (bölge maskesi; UI min/max normalleştirir ve ≥2 punto
ister) yazar. Şema kapısı doğrudan SQL yolunu da kapsayan **yedek**tir.

**Testler.** `redactions` +1 (`redaction_region_must_lie_on_a_parsed_page_inside_its_box`:
12 doğum kusuru reddedilir; kenara dayanan kutu (+0,5 punto), negatif yuvarlama payı
(−0,5), iki geçerli dörtlü, geometrisi NULL sayfa, yarım (`partial`) koşu ve tamamlanmış
ama **sayfasız** koşu kabul edilir; çekirdeğin yolu da aynı kapıdan geçer), `upgrade_tests`
+1 (`upgrade_keeps_redaction_regions_written_before_the_page_bound_gate`: v53'te yazılmış
sayfa dışı bölge silinmez/düzeltilmez, donar; yeni sayfa dışı bölge doğamaz, meşru
doğar) → `dim-storage --lib` **701 → 703**; yerelde **703/703 yeşil, 572,96 s**; fmt (iki
manifest) / clippy `-D warnings` `--all-targets` (1 dk 35 s) / masaüstü
`cargo check --all-targets` (2 dk 20 s) temiz. **Mevcut test düzeltmesi gerekmedi**
(fikstürlerde sayfa dışı meşru bölge yoktu).

**Kapı kanıtı.** Taban **4/4 yeşil** (mutasyonsuz, filtre `redaction_region`), ardından
**10/10 mutant KIRMIZI** (`~/.hermes/cache/scratch/dim-v54-probe/gates_v54.py`, log
`gates_v54.log`): sayfa eşleşmesi, bildirilen koşunun yok sayılması, `status='succeeded'`
koşulu, «koşuda sayfa var mı» ön koşulu, pozitif alanın x ve y ekseni, sınırın dört
karşılaştırması (negatif x, negatif y, genişlik, yükseklik) tek tek nötrlendi. Kaynak
**byte-birebir** geri yazıldı (`sha256` önce = sonra
`0054…sql e0b90c8e2e715836…`). Ölçüm sonrası: 25 yazımın **14'ü RED** (12 hedef + 2 zaten
RED), 11 meşru KABUL (`measure_after.log`). **Dürüst (ilk koşu 8/9):** M6 (negatif y
sınırı) YEŞİL kaldı çünkü testte yalnız **x** negatifti; `[[10,−50,50,100]]` satırı
eklendi, ölçüm ve kapı yeniden koşuldu (10/10) — ilk 8 hükmün M6'sı kanıt sayılmadı.

**CI (ölçüldü).** Kod `2d161da` (4 dosya, **+514/−1**) → run `44291069814` **10/10
YEŞİL** (`dim-storage` **703** iki OS'ta birebir: macos-15 **371,26 s** / ubuntu-latest
**834,60 s**; `arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB —
**değişmedi**, arayüz kodu değişmedi; `masaüstü kabuk`, `windows güvenli depo`, SBOM,
sözleşmeler, belge worker'ı, sürüm paketi manifesti ve kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) Ayrıştırılmış **hiç sayfası olmayan** ya da ayrıştırması
**tamamlanmamış** (`running`/`partial`/`failed`/`cancelled`) sürümde sayfa varlığı
denetlenmez — maske PDF'i zaten maskeler, yarım koşuda eksik sayfa yalan sayılmaz;
(b) `width`/`height` NULL olan eksende sınır denetlenmez; kutu biçimi yine denetlenir;
(c) kutu sayfadaki metni **gerçekten** kapsıyor mu doğrulanmaz (kesişim
`outbound_terms_from_plan` ve maskeleme doğrulamasının işidir); (d) rotasyon
denetlenmez — döndürülmüş sayfa maskede zaten reddedilir, bu kapı yalnız kırpma kutusu
uzayını ölçer; (e) v54 öncesi sayfa dışı bölgeler **silinmez** ve donmuş oldukları için
onarılamaz — onarım yolu yeni plandır, temizlik ürün kararı; (f) `parse_run_id` boş
metinse (`''`) bildirilmemiş sayılır ve en yeni sayfalı koşuya düşülür; (g) tetikleyicileri
anahtarı olan saldırgan düşürebilir; (h) arayüz davranışı değişmedi; gerçek pencere +
görsel QA **insan bekliyor**.

## F08-T04ab — aktarım makbuzu dışarıya ne çıktığının kanıtıdır: doğuşta `attempted`, sonuç terminal (şema v55, 2026-11-18)

**Neden bu dilim.** `transfer_receipts` satırı **dışarıya ne çıktığının makbuzudur** —
«nereye, kaç öğe, kaç bayt, hangi içerik özetiyle, ne zaman». Aktarım zincirinin son
halkasıdır: `begin_transfer_receipt` makbuzu **deneme anında** açar (`attempted`;
gönderim zaman aşımına düşse de içerik çıkmış olabilir), `finish_transfer_receipt` tek
koşullu UPDATE ile `attempted`ı `dispatched`/`failed`a bağlar, açılış uzlaştırması
(`reconcile_attempted_receipts`) sonucu yazılmamış denemeleri `interrupted` yapar.
Ekran (`Ai.tsx`) son gönderimleri bu satırdan basar; `list_transfer_receipts` dosya
kapsamını `ai_runs.case_id` üzerinden kurar. Zincirin komşuları kapılıydı
(`transfer_segments` v45/v47, onay v43/v45, `ai_runs` v43) ama **makbuzun kendisi
değildi**: `status` için CHECK bile yok, `item_count`/`byte_count` negatif doğabilir,
`dispatched_at` serbest metin, `outbound_hash` biçimi denetlenmez ve satır sonradan
**her** sütunuyla değiştirilebilir; makbuz başka dosyanın çalıştırmasına taşınabilir.
Tur başında `HEAD == origin/main` = `a76e3ee`; disk **7,6 G**; ağaçta yalnız **başka
ajanın 9 arayüz dosyası** (2026-11-07'den beri) — dokunulmadı.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v54; sonda
`~/.hermes/cache/scratch/dim-v55-probe/measure_v55.py`, log `measure_before.log`):**
16 doğum kusurunun **14'ü KABUL** (2'si zaten RED — yalnız FK), 12 UPDATE saldırısının
**12'si KABUL**, 7 meşru yazma KABUL. Doğum: `dispatched` (hiç denenmemiş gönderim
"gönderildi" der), bilinmeyen durum (`sent`), boş / yalnız boşluk hedef etiketi,
63 haneli / büyük harf / hex olmayan özet, `item_count` 0 ve −1, `byte_count` 0 ve −5,
damga `dün` ve boş, boş makbuz kimliği, var olmayan / boş çalıştırma kimliği. UPDATE:
hedef etiketi çevirme, içerik özeti çevirme, öğe/bayt sayısı değiştirme, damgayı geriye
yazma, makbuzu **başka dosyanın** çalıştırmasına taşıma, kimlik takası, bilinmeyen
durum yazma, `dispatched`→`attempted` (sonuç geri alındı), `failed`→`dispatched`
(başarı uyduruldu), `interrupted`→`dispatched`, sonuçlanmış satırın hedefini çevirme.

**Yazılan (şema v55, `0055_transfer_receipt_evidence_guard.sql`, 2 tetikleyici).**
BI — `id` kırpılmış-dolu; `ai_run_id` **var olan** çalıştırmayı gösterir (fail-closed;
FK zaten bağlar, kapı FK kapalıyken de doğru hüküm verir; NULL/boş kimlik `NOT EXISTS`
ile aynı cümleye düşer, ayrı cümle yazılmadı); `destination_label` boşluktan ibaret
değil; `outbound_hash` **64 küçük harf hex** (v47/v53 ile aynı); `item_count ≥ 1` ve
`byte_count ≥ 1` (çekirdek boş kapsamı makbuz açılmadan reddeder — `ai_run_execute`;
sıfır öğeli "gönderim" makbuz değildir); doğuşta `status='attempted'`; `dispatched_at`
ISO-8601 damgası. BU — `id`, `ai_run_id`, `destination_label`, `outbound_hash`,
`item_count`, `byte_count`, `dispatched_at` **donar** (yeniden ölçüm yeni satır ister);
`status` yalnız `attempted` → `dispatched` | `failed` | `interrupted` **ilerler**,
sonuçlanmış satırın durumu **terminal**dir (geri alma ve sonuç uydurma kapalı);
değişiklik yoksa takılmaz. Ret `DIM_INTEGRITY:transfer_receipts` →
`StorageError::Integrity` → **`E_DB_INTEGRITY`**. Tablo `case_id` taşımaz; dosya
kapsamı `ai_runs` üzerindendir ve `ai_run_id` bu kapıda donduğu için makbuz başka
çalıştırmaya taşınamaz → kapsam taramasına sınıf eklenmedi, `GUARDED_TABLES` **25
kalır**.

**Çekirdek hizası.** Ek kapı gerekmedi: üç yazma yolu (`begin_transfer_receipt`,
`finish_transfer_receipt`, `reconcile_attempted_receipts`) zaten tam bu biçimi üretir;
`ai_run_execute` boş kapsamı makbuz açılmadan reddeder (`scope.segments.is_empty()`),
öğe/bayt sayıları `i64::try_from(...)` ile pozitiftir. **Tek test düzeltmesi (dürüst):**
`receipts_are_listed_newest_first` sıra sözleşmesini satırın `dispatched_at`ini
**sonradan UPDATE ile** yazarak sabitliyordu — v55 damgayı dondurduğu için bu yol
kapandı; test artık iki satırı sabit ISO damgalarla **doğuşta** yazıyor (BI onları
kabul eder) ve çekirdek yolu ayrı testte ölçülüyor.

**Testler.** `ai` +1 (`transfer_receipt_is_born_attempted_and_frozen_after_its_verdict`:
16 doğum kusuru; 8 dondurma yolu; `attempted` satırda ve sonuçlanmış satırda bilinmeyen
durum; başarı uydurma; no-op yazma geçer; çekirdeğin üç geçişi ve açılış uzlaştırması
geçer; saldırılardan sonra kanıt sütunları doğduğu gibi), `upgrade_tests` +1
(`upgrade_keeps_transfer_receipts_written_before_the_evidence_gate`: v54'te yazılmış
bozuk makbuz silinmez/düzeltilmez, donar; yeni bozuk makbuz doğamaz; eski `attempted`
satır kapıya rağmen uzlaştırmayla kapanır — kapı çekirdeğin temizliğini durdurmaz) →
`dim-storage --lib` **703 → 705**; yerelde **705/705 yeşil, 557,77 s**; fmt (iki
manifest) / clippy `-D warnings --all-targets` / masaüstü `cargo check --all-targets`
(2 dk 24 s) temiz.

**Kapı kanıtı.** Taban **yeşil** (mutasyonsuz), ardından **19/19 mutant KIRMIZI**
(`~/.hermes/cache/scratch/dim-v55-probe/gates_v55.py`, log `gates_v55.log`): kimlik boş
denetimi, çalıştırma varlığı, hedef etiketi, özet uzunluğu, özet hex kümesi,
`item_count` ve `byte_count` pozitifliği, doğuşta `attempted`, damga biçimi, yedi
dondurma sütunu tek tek, durum geçiş kuralının tamamı, durum kümesinin açılması ve
yalnız `dispatched`a izin (narrowing). Kaynak **byte-birebir** geri yazıldı (`sha256`
önce = sonra `0055…sql d3ec078ac8dc49fd…`). Ölçüm sonrası: **16/16 doğum kusuru RED**,
**12/12 saldırı RED**, 7 meşru KABUL (`measure_after.log`). **Dürüst (ilk koşu
18/20):** (a) «boş çalıştırma kimliği» cümlesi **eşdeğer mutant** çıktı — `NOT EXISTS`
zaten NULL/boşu kapsıyordu; cümle kapıdan **çıkarıldı** (test zayıflatılmadı, gereksiz
cümle silindi); (b) kapalı durum kümesi mutantı YEŞİL kaldı çünkü bilinmeyen durum
saldırısı yalnız **sonuçlanmış** satırda ölçülüyordu (`dispatched` satırda geçiş kuralı
tek başına yakalıyor); `attempted` satıra `status='sent'` saldırısı eklendi ve **19/19**
yeniden ölçüldü — ilk koşunun 18 hükmü kanıt sayılmadı.

**CI (ölçüldü).** Kod `11fa866` (4 dosya, **+585/−17**) → run `44308711465` **10/10
YEŞİL** (`dim-storage` **705** iki OS'ta birebir: macos-15 **456,18 s** / ubuntu-latest
**1077,68 s**; `arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB —
**değişmedi**, arayüz kodu değişmedi; `masaüstü kabuk`, `windows güvenli depo`, SBOM,
sözleşmeler, belge worker'ı, sürüm paketi manifesti ve kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) v55 öncesi bozuk makbuzlar **silinmez, düzeltilmez**; donmuş
oldukları için onarılamaz — onarım yolu **yeni gönderim**dir, temizlik ürün kararı;
(b) `outbound_hash`in gerçekten gönderilen baytların SHA-256'sı olduğu **yeniden
hesaplanmaz** (SQLite'ta SHA-256 yok; özeti çekirdek üretir) — biçim ve değişmezlik
ölçülür; (c) `item_count`/`byte_count`in `transfer_segments` ile tutarlılığı
doğrulanmaz (deneme kapsamı yeniden gönderimde farklı olabilir; makbuz başına ikinci
bir kapsam kaynağı yok); (d) `dispatched_at` **gelecekte** olabilir — damga biçimi
denetlenir, mantığı değil; (e) bilinmeyen durumlu **eski** satır donar (ilerletilemez);
(f) aynı çalıştırma için **birden çok** makbuz meşrudur (her deneme ayrı satır) — kapı
bunu engellemez; (g) tetikleyicileri anahtarı olan saldırgan düşürebilir; (h) arayüz
davranışı değişmedi (rozetler aynı sütunlar); gerçek pencere + görsel QA **insan
bekliyor**.

## F08-T04ac — emsal künyesi «hangi karar bu» iddiasıdır: rozet kanıtla tutarlı, künye donmuş (şema v56, 2026-11-18)

**Neden bu dilim.** `precedents` satırı **hangi karar olduğunun iddiasıdır** — mahkeme,
daire, esas/karar/başvuru numarası, karar tarihi, başlık, kaynak bağlantısı ve
`completeness` rozeti. Dört okuyucu bu satıra yaslanır: (1) `screens/Precedents.tsx` her
satırda rozeti (`ok`/`warn`) ve `decisionDate`i **ham** basar; (2) `precedent_bibliography`
kopyalanabilir hukuki künye metnini bu sütunlardan kurar — **tarih ISO değilse «olduğu
gibi» yazılır**, eksik alan `missing` listesine girer, yani rozet ile `missing` **aynı
ekranda** çelişebilir; (3) `find_precedent_by_identity` aynı kararın nüshalarını mahkeme
+ daire + numaralarla eşler, künye çevrilebilirse yazılmış nüsha (`precedent_versions`)
sessizce **başka karara** bağlanır; (4) `search_precedents` künye numaralarında arar.
Zincirin komşuları kapılıydı (`precedent_versions` v44/v48, `precedent_notes` v49,
`citation_checks` v37/v46) ama **künyenin kendisi değildi**: tablonun 0001'den beri
**hiçbir** tetikleyicisi yoktu; yalnız `completeness` ve 0016'nın iki kapalı kümesi
CHECK'ti.

**Ölçülen kusur (gerçek SQLite v55; `~/.hermes/cache/scratch/dim-v56-probe/measure_v56.py`,
log `measure_before.log`).** 47 yazımın **47'si KABUL** — 37 kusur + 10 meşru yol.
Doğum (24): boş kimlik; boş / yalnız boşluk / kırpılmamış mahkeme; **rozet yalanı**
(`complete` + iki numara da NULL; `complete` + karar no yok); **rozet eksik-beyanı**
(`incomplete` + iki numara dolu); tarih `dün`, `02.05.2025`, `2026-02-30`, `2023-02-29`,
`2026-1-5`, ISO + saat; daire boş / yalnız boşluk / kırpılmamış; esas no, karar no,
başlık, başvuru no ve kaynak url kırpılmamış; konu yalnız boşluk; etiket bozuk JSON /
nesne / sayı dizisi (okuyucu `unwrap_or_default` ile **sessizce boşaltır**); boş
`created_at`, `created_at='dün'`. UPDATE (13): mahkeme, esas no, karar no, karar tarihi,
başlık, kaynak url, etiketler, konu ve `created_at` çevrildi; daire NULL'a çekildi;
başvuru no eklendi; rozet sonradan çevrildi; **kimlik takası** (`id`).

**Yazılan (`0056_precedent_identity_guard.sql`, 2 tetikleyici).** BI — `id` kırpılmış-dolu;
`court` kırpılmış ve dolu; `completeness` **kanıtla tutarlı** (`complete` ⇔ kırpılmış esas
**ve** karar no dolu; çekirdek zaten böyle hesaplıyor); `decision_date` NULL ya da
**gerçek takvim günü** `YYYY-AA-GG` (`date()` eşitliği: `2026-02-30`, `2023-02-29`,
`2026-1-5`, `02.05.2025`, `dün` ve saat taşıyan damga reddedilir — hukuki tarih **gün**
taşır); isteğe bağlı yedi metin sütunu NULL ya da **kırpılmış ve boşluk değil**;
`user_tags` **metin dizisi** (`json_valid` + `json_type='array'` + her elemanın
`typeof='text'`); `created_at` ISO-8601 damgası (0052 ile aynı desen). BU — on üç
**kimlik** sütunu (`id`, `court`, `chamber`, `docket_no`, `decision_no`, `application_no`,
`decision_date`, `title`, `source_url`, `user_tags`, `legal_issue`, `completeness`,
`created_at`) **donar**; değişmeyen yazma takılmaz. **Kullanım koşulu** sütunları
(`usage_basis`, `redistribution`, `terms_url`, `terms_note`, `terms_recorded_at`) bilinçli
olarak kapının **dışında** — `set_precedent_usage_terms` onları meşru olarak günceller
(test bunu ölçer). Ret `DIM_INTEGRITY:precedents` → `StorageError::Integrity` →
**`E_DB_INTEGRITY`**. Tablo `case_id` taşımaz → kapsam taramasına sınıf eklenmedi,
`GUARDED_TABLES` **25 kalır**.

**Çekirdek hizası (davranış değişikliği).** `create_precedent` karar tarihini artık **ISO
güne** çevirir (`GG.AA.YYYY`, `GG/AA/YYYY`, `YYYY/AA/GG` → `YYYY-AA-GG`; sıra **tahmin
edilmez**). Gerekçe somut: toplu künye içe alma (`parse_manifest`) tarih hücresini **ham**
geçiriyordu — `tarih` sütununa `02.05.2025` yazan bir CSV, bibliyografiye **olduğu gibi**
giren bir tarih üretiyordu. Çözülemeyen değer olduğu gibi bırakılır ve **kapı reddeder**
(fail-closed); böylece ne sessiz bozuk künye ne de sessiz kayıp olur.

**Testler.** `precedents` +1 (`precedent_identity_is_coherent_at_birth_and_frozen_afterwards`:
24 doğum kusuru — her biri kendi satırında, rozet yalanı/eksik-beyanı dahil; 13 UPDATE
saldırısı; meşru doğumlar (tam/eksik künye, artık yıl, TR tarih → ISO, eski tarih, Türkçe
etiketler, başvuru no) ve meşru UPDATE'ler — no-op ve **kullanım koşulu yazımı** — geçer),
`upgrade_tests` +1 (`upgrade_keeps_precedents_written_before_the_identity_gate`: v55'te
yazılmış tutarsız künye silinmez/düzeltilmez, donar; yeni rozet yalanı doğamaz; meşru
künye ve koşul yazımı çalışır) → `dim-storage --lib` **705 → 707**; yerelde **707/707
yeşil, 481,35 s**; fmt (iki manifest) / clippy `-D warnings --all-targets` / masaüstü
`cargo check --all-targets` temiz. **Tek test düzeltmesi (dürüst, ilk tam koşu 706/707):**
`bibliography_keeps_non_iso_date_verbatim_and_warns` **eski yazma yolunu** sabitliyordu —
`create_precedent`e `01.06.2025` verip «tarih olduğu gibi saklanır + uyarı düşer»i
bekliyordu; v56 çekirdeği tarihi ISO'ya çevirdiği için uyarı artık **düşmüyor** (istenen
davranış). Okuyucu sözleşmesi kaybolmasın diye test ikiye ayrıldı: eski satır artık
tetikleyici **bilinçli düşürülerek** yazılıyor (v52'nin deseni) ve «olduğu gibi + uyarı»
yine ölçülüyor; ayrıca çekirdek yolun **sessiz ve doğru** olduğu (ISO'ya çevrilir, ISO
uyarısı düşmez) ayrı assert ediliyor. Kapı gevşetilmedi; kusuru pinleyen beklenti
düzeltildi ve nedeni burada yazılı.

**Kapı kanıtı.** Taban **yeşil**, **25/25 mutant KIRMIZI**
(`~/.hermes/cache/scratch/dim-v56-probe/gates_v56.py`, log `gates_v56.log`): kimlik boş,
mahkeme boş/kırpma, rozet tutarlılığı, tarih takvimi, tarih NULL guard'ının **yönü**,
daire kırpma/boşluk ve boş-metin, etiket JSON/dizi türü/eleman türü, damga biçimi, on üç
dondurma sütunu tek tek. Kaynak **byte-birebir** geri yazıldı (`sha256` önce = sonra
`0056…sql 3a95fe382b5594d2…`). **Dürüst (ilk koşu KANIT SAYILMADI):** ilk 25/25 koşusundan
sonra her mutantın **geçerli SQL** ürettiğini ayrıca ölçtüm
(`verify_mutants_v56.py`, log `verify_mutants_v56.log`): M6 ilk yazımı **parantez dengesi
bozuk** SQL üretiyordu — göç hiç yüklenmiyor, **her** yazma düşüyordu, yani KIRMIZI
sahteydi. Mutant tek belirteçlik **yön çevirmeye** (`IS NOT NULL` → `IS NULL`) dönüştürüldü;
doğrulama her mutant için üç şeyi ölçer: (a) SQL yükleniyor, (b) hedef kusur(lar) açılıyor,
(c) meşru yazmalar KABUL kalıyor. M6 meşru **NULL tarihli** künyeyi düşürür — testi kırmızı
yapan tam olarak budur; guard'ın yönü böyle kanıtlandı. İlk koşunun 25 hükmü kanıt
sayılmadı, 25/25 yeniden ölçüldü. Ölçüm sonrası: **37/37 kusur RED**, 10 meşru KABUL
(`measure_after.log`).

**CI (ölçüldü).** Kod `397aa11` (5 dosya, **+555/−8**) → run `44363634559` **10/10 YEŞİL**
(`dim-storage` **707** iki OS'ta birebir: macos-15 **457,17 s** / ubuntu-latest **440,92 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**;
`masaüstü kabuk`, `windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı, sürüm paketi
manifesti, kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) v56 öncesi tutarsız künyeler **silinmez, düzeltilmez**; kimlik
donduğu için onarılamaz — onarım yolu **yeni kayıt**, temizlik ürün kararı. (b) **Kullanım
koşulu iddiası** (`allowed` ⇒ dağıtılabilir dayanak + yazılı dayanak + damga) bu turda
**kapsam dışı**; Rust doğrular (`set_precedent_usage_terms`), şema doğrulamaz — sıradaki
aday. (c) `source_url` yalnız kırpılmış-dolu denetlenir; **URL biçimi** (şema, kontrol
karakteri) doğrulanmaz. (d) Tarih **takvimde var mı** denetlenir, **gelecekte** mi diye
bakılmaz. (e) SQLite `trim()` yalnız ASCII boşluk kırpar — sekmeyle (`\t`) doldurulmuş bir
alan bu kapıdan geçer. (f) Etiketlerin **içeriği** (boş metin, yinelenen, uzunluk)
denetlenmez, yalnız biçimi. (g) `completeness` yalnız **kanıtla tutarlılık** yönünden
denetlenir; künyenin gerçekten o karara ait olduğu (mahkeme/numaraların doğruluğu)
doğrulanmaz. (h) Tetikleyicileri anahtarı olan saldırgan düşürebilir. (i) Arayüz davranışı
değişmedi (rozet ve tarih aynı sütunlardan); gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04ad — kullanım koşulu iddiası «bu metni dağıtabilirim»in kanıtı olmalı (şema v57, 2026-11-18)

**Neden bu dilim.** `precedents`in beş sütunu — `usage_basis`, `redistribution`,
`terms_url`, `terms_note`, `terms_recorded_at` — birlikte **«bu kararın tam metnini
yeniden dağıtabilirim»** iddiasıdır. İddianın okuyucusu onu **yeniden denetlemiyordu**:
`build_precedent_corpus` yalnız `redistribution` sütununa bakıp `allowed` satırın metnini
**dosyaya yazıyor** (`CorpusItem.text`), `precedent_corpus_coverage` izinli sayısını aynı
sütundan sayıyor, `Precedents.tsx` liste rozeti (`termsBadge`) ve koşul paneli damgayı
doğrudan basıyor. 0016 sütunları eklerken yalnız iki kapalı küme CHECK'i koymuştu; kuralın
kendisi (izin ⇒ dağıtılabilir dayanak + yazılı dayanak + damga; url biçimi) **şemada
yoktu** — 0056 bunu bilinçli olarak dışarıda bırakmıştı (kapanış sınırı (b)).

**Ölçülen kusur (gerçek SQLite v56; `dim-v57-probe/measure_v57.py`, log
`measure_before.log`).** 55 yazımın **55'i KABUL**: 37 kusur + 14 meşru + 4 eski satır yolu.
Doğum (22): `allowed` + dayanak `unknown`/`internal_only`; `allowed` + yazılı dayanak yok;
not yalnız boşluk; `allowed` + damga yok/`dün`/boş metin; url `ftp://`, şemasız, kırpılmamış
(sondaki boşluk), boş metin, yalnız boşluk, 2001 karakter, satır sonu/satır başı/sekme
taşıyan; not kırpılmamış, 4001 karakter; damga `dün`/boş/`01.06.2026`. UPDATE (15): dayanak
`unknown`a/`internal_only`a çevrildi; url ve not birlikte silindi; damga NULL'a çekildi/`dün`
yazıldı; url `ftp://` yapıldı; not yalnız boşluğa çevrildi; not kırpılmamış yazıldı/4001
karaktere çıkarıldı; url kırpılmamış yazıldı/satır sonu/satır başı/sekme eklendi/2001
karaktere çıkarıldı; **yazılı dayanak yokken izin sonradan açıldı** (yalnız
`redistribution` değişti).

**Yazılan (`0057_precedent_usage_terms_claim_guard.sql`, 2 tetikleyici).** BI — `allowed`
**desteklenmiş** olmalı: dayanak dağıtılabilir üçlüden (`official_public|licence|permission`;
`internal_only` ve `unknown` dağıtılamaz), yazılı dayanak var (url ya da not kırpılmış-boş
değil), `terms_recorded_at` ISO-8601 damgası taşıyor; `terms_url` NULL ya da kırpılmış
`http://`|`https://` (≤ 2000 karakter, CR/LF/TAB yok); `terms_note` kırpılmış (≤ 4000);
damga NULL ya da ISO. `unknown`/`not_allowed` **serbest** — izin vermemek kanıt istemez.
BU — beş sütundan **biri değişiyorsa** yeni satır aynı kurala uymalı; sütunlara dokunmayan
yazma (künye no-op'u) takılmaz, yani eski satır kilitlenmez ve **onarım yolu açıktır**
(dayanak + yazılı kanıt + damga yazılır; 0056'nın aksine). Ret `DIM_INTEGRITY:precedents` →
`E_DB_INTEGRITY`. Tablo `case_id` taşımaz → `GUARDED_TABLES` **25 kalır**.

**Çekirdek hizası — dışa aktarma artık sütuna güvenmiyor.** Şema kapısı yalnız **yeni**
yazmayı durdurur; v57 öncesi ya da elle değiştirilmiş bir satır `allowed` diyorsa korpus
metni yine dosyaya çıkabilirdi. Bu yüzden `CorpusRecord::permission_backed()` aynı kuralı
**okuma anında** uyguluyor: kanıtsız `allowed` **izin sayılmaz**, metni dışa aktarılmaz ve
`redistribution_unknown` («izin kaydı yok») olarak raporlanır. **Ölçüldü** (geçici sızıntı
sondası, eski mantık geri konarak): eski yolda `coverage.allowed=1 exported=1 excluded=0
secret_in_items=true` — yani metin gerçekten dosyaya yazılacaktı; yeni yolda aynı satır
`exported=0`, `excluded=1`, `reason=redistribution_unknown`, `secret_in_items=false`. Sonda
silindi, kaynak **byte-birebir** geri yazıldı (sha256 önce = sonra).

**Testler.** `precedent_terms` +2: `usage_terms_claim_is_backed_and_well_formed` (22 doğum +
15 UPDATE saldırısı reddedilir; meşru doğumlar, sıfırlama, izin geri alma ve çekirdek yolu
geçer) ve `corpus_export_does_not_trust_an_unbacked_allowed_claim` (kapı yerindeyken kanıtsız
izin `E_DB_INTEGRITY`; v57 öncesi satırda tetikleyiciler **bilinçli** düşürülüp dışa
aktarımın metni sızdırmadığı ve `all` kipinin kapandığı ölçülür). `upgrade_tests` +1
(`upgrade_keeps_precedents_written_before_the_usage_terms_gate`: dört eski satır silinmez,
kanıtsız iddia kendiliğinden düzelmez, hâlâ kanıtsız kalan değişiklik reddedilir, onarım
geçer) → **707 → 710**.

**Kapı kanıtı.** Taban **yeşil**, **29/29 mutant KIRMIZI** (`gates_v57.py`, log
`gates_v57.log`) ve her mutant için üç şey ayrıca ölçüldü: (a) SQL yükleniyor, (b) hedef
kusur **KABUL** olmaya başlıyor (eşdeğer mutant değil), (c) 11 meşru yol KABUL kalıyor —
**29/29 delik açıldı · meşru yollar korundu**. Kaynak **byte-birebir** geri yazıldı
(sha256 `14b5d34451cb…`). **Dürüst (kendi hatam):** ilk koşuda 29/29 KIRMIZI ama 28/29
«delik açıldı» çıktı — M14'ün tanığı (`izin sonradan açıldı`) **iki** cümleye birden
takılıyordu (damga da NULL'dı), yani tanık yanlıştı; tanık `url ve not birlikte silindi` ile
değiştirildi, **29/29 yeniden ölçüldü**, ilk koşunun hükümleri kanıt sayılmadı. Ayrıca ilk
yazımda iki cümle **eşdeğer** çıktı (url `''`/boşluk denetimi şema kuralınca zaten
kapsanıyordu) → cümleler kapıdan **çıkarıldı**; kapı gevşetilmedi. Ölçüm sonrası: **37/37
kusur RED**, 17 meşru/eski-yol KABUL.

**CI (ölçüldü).** Kod `b9cd768` (4 dosya, **+662/−11**) → run `44387888890` **10/10 YEŞİL**
(`dim-storage` **710** iki OS'ta birebir: macos-15 **477,99 s** / ubuntu-latest **457,67 s**;
`arayüz` 66 dosya / **1434** test, paket `index-BJpSJSmX.js` 694,50 kB — **değişmedi**;
`masaüstü kabuk`, `windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı, sürüm paketi
manifesti, kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) v57 öncesi kanıtsız `allowed` satırları **silinmez**; kapı yalnız
yeni yazmayı durdurur — onarım elle (arayüz/Rust yolu) yapılır ve artık **mümkündür**.
(b) Kapı iddianın **doğruluğunu** kanıtlamaz: yazılı dayanak metninin gerçekten o izni
verdiği, url'nin yaşadığı ve kararın o kaynaktan alındığı doğrulanmaz — yalnız iddianın
**iç tutarlılığı** ve biçimi. (c) SQLite `trim()` yalnız ASCII boşluk kırpar (U+00A0 artığı
geçer). (d) Kontrol karakterlerinden yalnız CR/LF/TAB denetlenir (gömülü NUL ve diğer C0
karakterleri SQL metninde temsil edilemez; Rust hepsini reddeder). (e) Damga **gelecekte**
olabilir ve geriye alınabilir; yalnız biçimi ve `allowed` için **varlığı** denetlenir.
(f) Notun **içeriği** ve url'nin **hedefi** denetlenmez. (g) `source_url` gibi `terms_url`
de yalnız şema denetimiyle sınırlıdır — DNS/erişim doğrulanmaz. (h) Tetikleyicileri anahtarı
olan saldırgan düşürebilir; çekirdek bu yüzden aynı kuralı **dışa aktarma anında** ayrıca
uygular (tek savunma hattı şema değildir). (i) Arayüz davranışı değişmedi; `termsBadge` yalnız
`redistribution` sütununa bakar — tahrif edilmiş/eski bir satırda rozet «izinli» diyebilir
(dışa aktarma yine izin vermez); gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04ae — «izinli» iddiası okurun gördüğü yerde de kanıt ister: liste rozeti ve koşul paneli ham sütuna güvenmez (şema v57 — değişmedi; okuma yolu, 2026-11-19)

v57 şema kapısı **yeni** `allowed` yazmasını kanıtla zorunlu kıldı; ama okurlar hâlâ ham
`redistribution` sütununa bakıyordu: liste satırındaki rozet (`termsBadge`) ve koşul
paneli `allowed` yazan **her** satırı «izinli» basıyordu. Dışa aktarma ise v57'den beri
kanıtsız iddiayı izin saymıyor. Yani **aynı satır** için iki hüküm vardı: dosyaya metin
yazılmıyor («izin kaydı yok» diye raporlanıyor), ekran ise «izinli» diyordu. v57'nin
kapanış sınırı (i) tam olarak bunu işaret ediyordu.

**Yazılan (şema değişmedi; tek kural, üç okuyucu).** `precedent_terms.rs`:
`allowed_claim_is_backed(...)` (yeniden dağıtılabilir dayanak + yazılı dayanak + kayıt
damgası) ve `effective_redistribution(...)` — `allowed` yalnız kanıtıyla ayakta kalır,
kanıtsız `allowed` **`unknown`**a düşer; `not_allowed` kanıt istemez (izin vermemek kanıt
gerektirmez). `CorpusRecord::permission_backed` bu tek işleve bağlandı: davranış aynı,
kural artık **tek yerde** (üç okuyucu ayrışmaz). `PrecedentRow` ve `PrecedentUsageTerms`
sunucuda **`redistributionEffective`** alanı taşır; `map_precedent` ham sütunları
(`terms_url`, `terms_note`) okuyup hükmü **okuma anında** yeniden hesaplar — alan, satırın
kendi verisinden türetilir, sütuna yazılmış bir iddia değildir. Arayüzde `termsBadge` ham
`redistribution`a değil bu alana bakar; alan yoksa (eski yük) izin **varsayılmaz**, yalnız
`not_allowed` olduğu gibi görünür (fail-closed). `Precedents.tsx` değişmedi; koşul formu
ham kaydı göstermeye devam eder (kullanıcı neyin kayıtlı olduğunu görür), kaydetme yolu
zaten `E_TERMS` ile kanıt ister — yani **onarım yolu açık**.

**Testler.** `precedent_terms` +1
(`readers_do_not_show_an_unbacked_allowed_claim_as_permitted`): kanıtlı izin (lisans + not)
üç okumada da `allowed`; v57 öncesi satır yolu (tetikleyiciler bilinçli düşürülüp) dört
kusur — dayanak yok, yalnız iç kullanım, yazılı dayanak yok, damga yok — hem listede hem
panelde **`unknown`** görünür, ham sütun iddiayı taşımaya devam eder; `not_allowed` olduğu
gibi kalır; IPC alan adı (`redistributionEffective`) serde ile ölçülür. Mevcut iki teste
birer assert eklendi (`redistribution_effective == "allowed"`) → `dim-storage --lib`
**710 → 711**; yerelde **711/711 yeşil, 593,67 s** (biçim düzeltmesinden sonraki son hâl
üzerinde; ilk koşu 548,58 s ve o koşu kanıt sayılmadı — üzerine iki assert eklendi).
fmt (iki manifest) / clippy `-D warnings --all-targets` / masaüstü `check --all-targets`
(2 dk 13 s) temiz. Arayüz: `precedent-terms` +1 (`kanıtsız 'izinli' iddiasını izinli
göstermez`) ve `Precedents` +1 (kanıtsız iddia satırında rozet «izin bilinmiyor», «izinli»
**yok**); fikstürler (PREC/PREC2/PREC3) `redistributionEffective` taşır — `PREC2`/`PREC3`
bunu **açıkça** ezer, aksi hâlde yayılan alan «izinsiz» satırı «izinli» gösterirdi.

**CI (ölçüldü).** Kod `6111a18` (7 dosya, **+304/−33**) → run `44406721617` **10/10 YEŞİL**
(`dim-storage` **711** iki OS'ta birebir: macos-15 **438,18 s** / ubuntu-latest **547,84 s**;
`arayüz` 66 dosya / **1436** test, paket `index-BTXhYWbb.js` **694,61 kB** — gzip 185,36 kB;
önceki `694,50 kB`, fark `termsBadge` alanı; `masaüstü kabuk`, `windows güvenli depo`, SBOM,
sözleşmeler, belge worker'ı, sürüm paketi manifesti, kaynak arşivi yeşil). Yerel arayüz
koşusu 66 dosya / **1439** test gösterir; fazladan 3 test **başka ajanın ağaçtaki** (commit
edilmemiş) dosyalarından gelir, bu commit'e girmedi.

**Dürüst sınırlar.** (a) `redistributionEffective` **yeniden hesaplanır**, ama girdisi yine
satırın kendi sütunlarıdır: not metninin gerçekten izin verdiği ve url'nin yaşadığı
doğrulanmaz (v57 sınırı (b) aynen geçerli). (b) `unknown` iki durumu birleştirir — «kayıt
yok» ve «kanıtsız izin iddiası»; rozet ikisini ayırmaz, ayrımı koşul formu (ham kayıt) ve
dışa aktarma raporu (`redistribution_unknown`) gösterir. (c) Alan **isteğe bağlı** okunur
(eski yük): yoksa `allowed` izin sayılmaz; bu fail-closed'dır ama gerçekten kanıtlı bir
satırı da «bilinmiyor» gösterirdi — aynı uygulama sürümünde alan daima gönderilir. (d)
Tetikleyicileri düşürebilen saldırgan sahte **yazılı dayanak** da yazabilir; kapı iç
tutarlılığı ölçer, doğruluğu değil. (e) v57 öncesi kanıtsız satırlar silinmez; rozet artık
onları «izin bilinmiyor» gösterir, onarım kullanıcının dayanak yazmasıyla olur. (f) Gerçek
pencere + görsel QA **insan bekliyor**.

## F08-T04af — yedek makbuzu «geri dönülebilir yedek var» iddiasıdır: kanıtı doğuşta taşır, sonuçlandıktan sonra donar (şema v58, 2026-11-19)

`backup_runs` satırı **kasa yedeğinin makbuzudur** — «hangi snapshot, hangi şema
sürümünden, hangi manifest özetiyle, doğrulandı mı, ne zaman». Üç okuma yolu bu satıra
yaslanır: `loadBackupFreshness` → `latestBackupSuccess` (`verified`/`succeeded`
satırların `completed_at ?? created_at`i = **«son başarılı yedek»**; `Settings.tsx`
bunu basar ve `nextBackupAt` **sonraki otomatik yedeğin tarihini** bu değerden öteler),
yedek geçmişi rozetleri (`backupRunStatusLabel`/`backupRunStatusTone`: «doğrulandı» ⇔
`verified`) ve `backup_run_list` IPC. Çekirdek kaydı `vault_backup` **yedeği aldıktan ve
`integrity_check` ile doğruladıktan sonra** yazar; yani satır bir **makbuzdur**, niyet
değil. Şemada bu tablonun tek kapısı `status` CHECK'iydi: manifest özeti biçimsiz, şema
sürümü negatif, damga serbest metin, `verified` kanıtsız doğabiliyor; satır sonradan
**her** sütunuyla değiştirilebiliyordu (kanıt silinebilir, `created_at` 2030'a alınabilir,
`failed`→`verified` çevrilebilir).

**Ölçülen kusur (gerçek SQLite v57; sonda `scratch/dim-v58-probe/measure_v58.py`, log
`measure_before.log`):** 47 yazımın **45'i KABUL** (2'si zaten RED — yalnız `status`
CHECK'i), 8 meşru yazma KABUL. Doğum (21): boş kimlik; snapshot boşluk / boş /
kırpılmamış / **yol ayırıcı** (`../kacis`) / sekme; şema sürümü 0 ve negatif; manifest
63 hex / **büyük harf** / hex değil / `'hash'`; hedef boşluk / boş / kırpılmamış;
`created_at`/`completed_at` `'dün'`; `running` + tamamlanma damgası; `running` + kanıt;
`verified` + tamamlanma yok; `verified` + **kanıtsız**; kanıt yalnız boşluk; bilinmeyen
durum. UPDATE (16): manifest / snapshot / şema sürümü / hedef çevrildi; `created_at`
**2030**'a alındı (tazelik ve otomatik yedek tarihi kaydı); kimlik takası;
`completed_at` geçmişe çekildi; `verified` satırın kanıtı silindi; `succeeded`→`verified`
kanıtsız; `failed`→`verified` (başarı uydurma); `running`→`succeeded` tamamlanma anı
olmadan / `'dün'` damgasıyla; bilinmeyen durum; `failed` satırın gerekçesi değiştirildi;
`running`→`verified` kanıtsız; `running`→`succeeded` kanıt yalnız boşluk.

**Yazılan (`0058_backup_run_receipt_guard.sql`, 2 tetikleyici):** INSERT — `id`
kırpılmış-dolu; `snapshot_id` **kırpılmış** ve dolu, `/` `\` CR/LF/TAB taşımaz (klasör
adıdır, yol değil); `schema_version ≥ 1`; `manifest_hash` **64 küçük harf hex**
(`manifest.json` baytlarının sha256'sı — çekirdek `hex` ile üretir); `status` kapalı
küme; `destination_label` kırpılmış-dolu; iki damga ISO-8601 (`YYYY-MM-DDTHH:MM:SS…`);
`verification_result` NULL ya da kırpılmış-dolu; `running` **ne** tamamlanma anı **ne**
kanıt taşır; terminal durumlar tamamlanma anı taşır; `verified` **kanıtlı** doğar.
UPDATE — `id`, `snapshot_id`, `schema_version`, `manifest_hash`, `destination_label`,
`created_at` **donar**; `status` yalnız `running → succeeded|failed|verified` ilerler,
`succeeded → verified` **yalnız o anda kanıt yazılıyorsa** meşrudur (sonradan doğrulama
gerçek bir ürün yoludur; kanıtsız «doğrulandı» değildir), sonuçlanmış satır başka yöne
**çevrilemez**; tamamlanma anı yalnız `running`den çıkarken ve ISO olarak yazılır; kanıt
`failed`/`verified`da donar ve her durumda kırpılmış-doludur. Değişmeyen yazma takılmaz —
v58 öncesi tutarsız satır **no-op** ile kilitlenmez. Ret `DIM_INTEGRITY:backup_runs` →
`StorageError::Integrity` → **`E_DB_INTEGRITY`**. Tablo `case_id` taşımaz (kasa düzeyi
geçmiş) → `GUARDED_TABLES` **25 kalır**.

**Çekirdek hizası (davranış sıkılaşması, `record_backup_run`):** manifest özeti artık
**64 küçük harf hex** olmalı (`E_BACKUP_INVALID`), snapshot klasör adı yol ayırıcı ve
kontrol karakteri taşıyamaz, `schema_version ≥ 1`, **kanıtsız `verified`** ve **`running`
+ kanıt** reddedilir — kurallar şema kapısıyla **aynı**; kapı düşürülse bile çekirdek
reddeder. `vault_backup` zaten `verified` + `integrity_check` yazdığı için ürün yolu
değişmedi.

**Testler:** `backup_runs` +1 (`backup_receipt_is_backed_at_birth_and_frozen`: 23 doğum
kusuru + 16 UPDATE saldırısı kapıdan RED; kanıtlı `verified`, `running`, gerekçeli
`failed`, kanıtsız `succeeded`, `running→succeeded` tamamlanma anıyla, `succeeded→verified`
kanıtla ve no-op **KABUL**; çekirdek aynı kuralları `E_BACKUP_INVALID` ile uygular),
`upgrade_tests` +1 (`upgrade_keeps_backup_runs_written_before_the_receipt_gate`: v57
tutarsız satırı **silinmez/düzeltilmez**, donar; yeni biçimsiz makbuz doğamaz; meşru
makbuz ve eski `running` satırın tamamlanması geçer) → `dim-storage --lib` **711 → 713**.
Mevcut sıra testi (`list_is_newest_first_and_survives_equal_timestamps`) `created_at`i
**UPDATE ile eşitliyordu** — v58 damgayı dondurduğu için sıra sözleşmesi artık eşit
damgayla **doğuşta** yazılarak ölçülüyor (kapı gevşetilmedi). fmt (iki manifest) / clippy
`-D warnings --all-targets` / masaüstü `check --all-targets` temiz.

**Kapı kanıtı:** taban iki uçta YEŞİL, **28/28 mutant KIRMIZI** (`gates_v58.py`, log
`gates_v58.log`): her mutant için (a) odak testi kırmızı, (b) hedef kusur **KABUL**
olmaya başlıyor, (c) 8 meşru yol KABUL kalıyor → **28/28 delik açıldı · meşru yollar
korundu**. Kaynak **byte-birebir** geri yazıldı (sha256 `b1cd5b118761b6a1…`). **Dürüst:**
ilk iki koşunun hükümleri kanıt **sayılmadı** — (i) kapı betiğinin sonda ayrıştırması
satır başındaki hükmü tanımıyordu (28/28 «HEDEF YOK»), (ii) düzeltmeden sonra üç mutant
YEŞİL kaldı: hedef testlerde eksik saldırı (`hedef boş`, `snapshot boş`,
`running→verified kanıtsız`, `running→succeeded` kanıt yalnız boşluk) ve M26'nın hedefi
yanlış satırdı (`u09`, oysa ayırt edici yol `u16`); testler ve hedef düzeltildi,
**28/28 yeniden ölçüldü**. `status` kapalı küme cümleleri (BI/BU) `CHECK` ile **eşdeğer**
olduğu için mutantlanmadı (CHECK varken aynı satırı o da reddeder) — kapı notunda yazılı.

**CI (ölçüldü).** Kod `412efd5` (4 dosya, **+923/−21**) → run `44425159253` **10/10 YEŞİL**
(`dim-storage` **713** iki OS'ta: macos-15 **472,84 s** / ubuntu-latest **443,67 s**;
`arayüz` 66 dosya / **1436** test, paket `index-BTXhYWbb.js` 694,61 kB — **değişmedi**;
`masaüstü kabuk`, `windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı, sürüm paketi
manifesti, kaynak arşivi yeşil). Yerelde **713/713 yeşil, 603,81 s** (son hâl; önceki koşu
592,77 s).

**Dürüst sınırlar.** (a) v58 öncesi tutarsız makbuzlar **silinmez ve düzeltilmez**
(donar); onarım yolu **yeni yedek almaktır**. (b) `manifest_hash`in gerçekten o
klasördeki `manifest.json`un SHA-256'sı olduğu **yeniden hesaplanmaz** (SQLite'ta
SHA-256 yok; çekirdek üretir) — biçim ve değişmezlik ölçülür. (c) Snapshot klasörünün
diskte **var olduğu** doğrulanmaz; satır hedefi göstermez, `destination_label` yalnız
etikettir. (d) Damga **gelecekte** olabilir (yalnız biçim ve dondurma). (e)
`verification_result`in içeriği denetlenmez (yalnız varlığı ve dondurulması); yedek
gerçekten geri yüklenebilir mi sorusu **geri dönüş tatbikatının** (F16-T03) işidir. (f)
Tetikleyicileri anahtarı olan saldırgan düşürebilir — çekirdek doğrulaması ayrıca durur.
(g) Arayüz davranışı **değişmedi** (aynı sütunlar); gerçek pencere + görsel QA **insan
bekliyor**.

## F08-T04ag — atıf kontrolü hükmü «bu alıntı bu karardan» iddiasıdır: hüküm ölçümden doğar, alıntı metni kayıtta (şema v59, 2026-11-19)

**Neden bu dilim.** `citation_checks` satırı hukukçunun dosyasındaki atıf tablosunun
hükmüdür: `identity_match` («künye tuttu») ve `quote_match` («alıntı metinde bulundu»)
rozetleri `Precedents.tsx` listesinde **doğrudan** basılır; `citation_context` aynı satırın
çevresini `verify_precedent_quote` ile **yeniden** doğrular. v46 satırın öznesini ve
hükmünü dondurdu (`identity_match`/`quote_match`/`extracted_citation`/
`candidate_precedent_id` yazıldıktan sonra değişmez) ama **doğuşta kanıt istemedi**;
dahası satırda **hangi pasajın** ölçüldüğü yoktu — `extracted_citation` atıf künyesidir
(`Yargıtay 1. HD 2024/123`), alıntı değil. Çekirdek `NewCitationCheck.quote_match`
alanını **çağırandan alıyordu**; arayüzdeki serbest açılır liste doğrulama düğmesinden
bağımsızdı: doğrulama yapılmadan «Eşleşti» seçilip kaydedilebiliyordu.

**Ölçülen kusur (gerçek SQLite v58; `dim-v59-probe/measure_v59.py`, log
`measure_before.log`).** 11 kusur yazımından **4'ü KABUL** — (b01/b02) aday yokken künye
hükmü, (b03) numarasız (eksik künyeli) emsale `match`, (b08) alıntı metni yokken alıntı
hükmü; kalan 7'si **temsil bile edilemiyordu** (`quote_text` sütunu yok → «hangi pasaj
ölçüldü» yazılamaz, kırpma/boşluk/4000 sınırı denetlenemez). 8 meşru yazma KABUL.

**Yazılan (`0059_citation_check_verdict_evidence_guard.sql`; yeni sütun + 1 BI + BU
genişletmesi).**
* `citation_checks.quote_text` — doğrulanan **pasajın kendisi**. NULL olabilir (yalnız
  künye kontrolü, v59 öncesi satırlar); doluysa kırpılmış, 1..4000 karakter.
* BI (`citation_check_quote_evidence_guard_bi`, 5 cümle): `quote_text` doluysa
  kırpılmış-dolu ve ≤ 4000; alıntı hükmü (`match`/`mismatch`) **alıntı metni** ister
  (hangi pasajın ölçüldüğü kayıtta yoksa hüküm kanıtsızdır); alıntı hükmü adayın **en az
  bir tam metinli sürümü** olmasını ister (ölçümün dayanağı); künye hükmü
  (`match`/`mismatch`) **aday** ister; künye `match` ayrıca adayın en az bir numarası
  (esas/karar) dolu olmasını ister. `not_checked`/`unknown` serbest — ölçmeyi reddetmek
  kanıt istemez.
* BU (`citation_check_subject_guard_bu` DROP+CREATE): `quote_text` de **donar** — hüküm
  sabitken pasaj değişirse rozet başka bir alıntının ölçümünü gösterirdi.
* Ret `DIM_INTEGRITY:citation_checks` → `E_DB_INTEGRITY`; tablo `case_id` taşır ve zaten
  taranıyor → `GUARDED_TABLES` **25 kalır**.

**Çekirdek hizası (davranış değişikliği).** `record_citation_check` hükmü **türetir**:
aday yoksa `not_checked`+`no_candidate`, `quote_text` yoksa `not_checked`+`no_quote`,
metin yoksa/kısa alıntıysa `not_checked` (doğrulamanın gerekçesi `no_full_text` /
`quote_too_short`), bulunduysa `match`, metin varken bulunamazsa `mismatch`+`not_found`.
`NewCitationCheck.quote_match` **kaldırıldı** (serde fazladan alanı yok sayar → geriye
uyumlu). `citation_context` pasajı `quote_text`ten okur; v59 öncesi satırlarda eski
davranışla `extracted_citation`a düşer. Alıntı metni 4000 karakterle sınırlı
(`MAX_QUOTE_TEXT_CHARS`; şema da aynı sınırı uygular). Tarama (`scope_violations`)
**üç** yeni sınıf sayar: adaysız künye hükmü, numarasız emsale `match`,
alıntısız/metinsiz emsale alıntı hükmü.

**Arayüz.** Serbest `Alıntı` açılır listesi (`c-quote`) ve «Atıf alanına taşı» düğmesi
**kaldırıldı**: hüküm artık seçilmez, kayıt anında ölçülür. Yerine «Atıf metni» yanında
**doğrulanacak pasaj** alanı (`c-quote-text`) geldi; pasaj kayda geçer, hükmü çekirdek
yazar. Liste rozeti aynı sütunu basmaya devam eder.

**Testler.** `quote_verdict_is_measured_from_the_text_not_claimed` (7 türetme yolu —
match/mismatch/no_full_text/no_candidate/no_quote/quote_too_short/4000 sınırı — ve 6
doğrudan SQL saldırısı; rozet ile bağlam panelinin **aynı** hükmü verdiği ölçülür),
`upgrade_tests` +1 (`upgrade_keeps_citation_verdicts_written_before_the_evidence_gate`:
v58'de kanıtsız yazılmış üç satır silinmez/düzeltilmez, taramada görünür; yeni kanıtsız
hüküm doğamaz) → **713 → 715**. Yerelde **715/715 yeşil, 542,80 s**. fmt (iki manifest) /
clippy `-D warnings --all-targets` / `tsc --noEmit` temiz; `Precedents.test.tsx` **57/57**.

**Kapı kanıtı.** Taban: kusur KABUL **0/11**, meşru satır KABUL, dondurma kapalı.
**8/8 mutant KIRMIZI** (`gates_v59.py`, log `gates_v59.log`): her mutantta (a) göç
yükleniyor, (b) hedef kusur KABUL olmaya başlıyor, (c) meşru satır KABUL kalıyor —
**8/8 delik açıldı · meşru yol korundu**. Kaynak **byte-birebir** (sha256
`1615eb406b6c415d…`; kapı betiği dosyayı yalnız okur). **Dürüst:** ilk koşu **7/8** —
M2'nin tanığı yalnız boşluktu ve o zaten `trim` cümlesine takılıyordu; `length(...) = 0`
cümlesi **boş metni** yakalar. Tanık `''` ile değiştirildi, **8/8 yeniden ölçüldü**; ilk
koşunun 7 hükmü kanıt sayılmadı.

**CI (ölçüldü).** Kod `80d0bca` (8 dosya, **+633/−87**) → run `44481594997` **9/10** —
rust işleri (macos-15 / ubuntu-latest / windows) **715** ile yeşilken **`arayüz`
KIRMIZI** düştü: yerelleştirme kapısı («sözlükte kullanılmayan anahtar kalmaz») elle
hüküm seçimi kaldırılınca sözlükte kalan `precedents.quoteToCitation` ve
`precedents.quoteNotChecked` anahtarlarını yakaladı (1 failed | 1435 passed). Düzeltme
`68b3dc4` (2 dosya, **+1/−14**): iki anahtar sözlükten düşürüldü, `Precedents.test.tsx`
kalan beklentileri temizlendi → run `44490584585` **10/10 YEŞİL** (`dim-storage` **715**
iki OS'ta: macos-15 **439,00 s** / ubuntu-latest **613,57 s**; `arayüz` 66 dosya /
**1436** test, paket `index-wBKnjNHs.js` 693,95 kB / gzip 185,22 kB; `masaüstü kabuk`,
`windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı, sürüm paketi manifesti,
kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) Hükmün **doğruluğu** SQL'de hesaplanmaz: katmanlı normalizasyon
(küçük harf, boşluk katlama, tire birleştirme) çekirdektedir; şema **kanıt tabanını**
(alıntı metni + aday + tam metin) doğrular, hükmü çekirdek türetir. Doğrudan SQL yazan
yol yine «metinde olmayan pasaja match» diyebilir (sondada `x01` — bilinçli kapı dışı).
(b) v59 öncesi satırlarda `quote_text` NULL'dur; bağlam paneli eski davranışla künye
metnine düşer — hangi pasajın ölçüldüğü **bilinmiyor**, düzeltilmez (donmuş satır).
(c) `quote_text`in **içeriği** denetlenmez (elle yazılmış/başka metinden kopyalanmış
olabilir); yalnız ölçümün girdisidir. (d) `identity_match` hâlâ **avukat hükmüdür**;
şema yalnız aday + numara ister — «gerçekten o karar mı» doğrulanmaz. (e) Emsal tam
metni sonradan silinirse eski `match` hükmü donmuş kalır (yeniden doğrulama
`citation_context`te «no_full_text» der; dürüst çelişki sınırda görünür). (f)
Tetikleyicileri anahtarı olan saldırgan düşürebilir — çekirdek türetmesi ayrıca durur.
(g) Arayüz değişikliği (pasaj alanı, kaldırılan liste/düğme) **gerçek pencerede** görsel
QA ile sınanmadı; **insan bekliyor**.

## F08-T04ah — dışa aktarma makbuzu «bu dosyadan ne çıktı, nereye» iddiasıdır: doğuşta kanıtlı, sonuçlandıktan sonra donar (şema v60, 2026-11-20)

**Neden bu dilim.** `exports` satırı «bu dosyadan **ne** çıktı, **nereye**, hangi
**özetle**, **ne zaman**, **başarılı mı**» iddiasıdır; `export_items` satırları da
«hangi **belge sürümlerinden** üretildi, o sürüm **maskeli** miydi» iddiasını taşır
(şartname §16.3, §433). Üç okuma yolu bu satıra yaslanır: `Exports.tsx` denetim
defteri (durum rozeti, hedef yol, özet öneki ve zaman **doğrudan** bu sütunlardan
basılır), `list_export_items` («kaynakları göster») ve `record_export`/`fail_export`
(«kayıt **önce** yazılır; denetlenemeyen aktarım yapılmaz»). Şemada bu iki tablonun
kapısı yalnız **kapsam**tı (`export_items`, 0037) ve `exports.status` için bir
`CHECK` vardı; `exports` üzerinde 0001'den beri **hiçbir tetikleyici yoktu**.
Komşuları kapılıydı (v55 aktarım makbuzu, v58 yedek makbuzu) ama dışa aktarma
makbuzu değildi.

**Ölçülen kusur (gerçek SQLite v59; `dim-v60-probe/measure_v60.py`, log
`measure_before.log`).** 75 kusur yazımının **73'ü KABUL** (2'si zaten RED —
yalnız `FOREIGN KEY`), 19 meşru yazma KABUL, 2 kapı dışı (bilinçli sınır) KABUL.
Doğum (41): boş/boşluk kimlik; boş, kırpılmamış, satır sonlu tür; doğuşta
`building`/`draft`/`blocked` (hiçbir yol tamamlamaz → sonsuza dek «sürüyor»
görünür); boş/kırpılmamış/satır sonlu hedef türü; `ready` + yol NULL/boş/boşluk/
kırpılmamış/satır sonlu; `ready` + özet NULL, `hash`, `'Z'*64`, **büyük harf**,
63 hane; boş/`'dün'` `created_at`; `ready` + `completed_at` NULL/`'dün'`/
`created_at`ten **önce**; `included_ids` JSON değil / nesne / sayı dizisi / boş
kimlik / **mükerrer** / **başka dosyanın sürümü** / `null` eleman / kırpılmamış
eleman; şablon boşluk / 65 karakter / kırpılmamış / boş metin; onay kimliği
kırpılmamış. UPDATE + silme (18): özet, hedef yol, `created_at`, `completed_at`,
tür, `included_ids`, şablon, hedef türü çevrildi; `ready`→`draft`/`building`
(geri alma); `failed`→`ready` (**sahte başarı**); `failed`→`blocked`; onay
sonradan eklendi; satır **başka dosyaya** taşındı; kimlik takası; makbuz ve kalem
satırı **silindi** (defter «silinemez» diyordu ama SQL siliyordu). Kalem (16):
boş kimlik; boş/kırpılmamış kaynak sürüm; bilinmeyen/boş maskeleme durumu; sayfa
aralığı 33 karakter / satır sonlu; atıf etiketi 65 karakter; **sonuçlanmış
(`failed`) makbuza kalem ekleme**; aynı sürüm **iki kez**; kaynak sürüm/
maskeleme/sayfa aralığı UPDATE ile çevrildi; kalem başka makbuza taşındı.

**Yazılan (`0060_export_receipt_evidence_guard.sql`, 5 tetikleyici + 0037
genişletmesi).** `exports` BI — `id` kırpılmış-dolu; `type` kırpılmış, dolu, ≤64
ve CR/LF/TAB taşımaz; doğuşta durum **yalnız `ready`**; `destination_kind`
kırpılmış-dolu ≤64; `destination_path` **dolu** (boşluk değil), kırpılmış, ≤4096,
kontrol karakteri yok; `output_hash` **64 küçük harf hex**; iki damga ISO-8601 ve
`completed_at ≥ created_at`; `included_ids` **metin dizisi** (json_valid + array +
her eleman kırpılmış metin), **mükerrer yok**, çözülebilen her kimlik **aynı
dosyanın** sürümü; `template_version` ve `externalization_consent_id` NULL ya da
kırpılmış-dolu ≤64. `exports` BU — on iki kanıt sütunu **donar**; durum yalnız
`ready`→`failed` ilerler; `failed` **terminal**; no-op takılmaz. `exports` BD —
makbuz **silinemez**. `export_items` BI — kimlik/sürüm kırpılmış-dolu, sürüm ≤128
ve kontrol karakteri yok, `redaction_state` **kapalı küme** (`none|masked`), sayfa
aralığı ≤32, atıf etiketi ≤64, üst makbuz **var ve `ready`**, aynı sürüm aynı
makbuza **iki kez** yazılamaz. `export_items` BU + BD — sayaç/etiket/maskeleme
**donar**, kalem **silinemez**; `export_id`/`source_version_id` 0037'nin kapsam
kapısında donar (`DROP`+`CREATE`, önek `DIM_SCOPE` korunur — v46 dersi).
**Kapsam dışına daraltma:** iki kalem tetikleyicisi `NOT EXISTS` (çapraz sürüm)
kapısıyla yalnız **kendi dosyasının** satırlarında çalışır; çapraz satırın hükmü
0037'nindir, iki kapı aynı satırda yarışmaz (v46 dersi). Ret
`DIM_INTEGRITY:exports` / `DIM_INTEGRITY:export_items` → `E_DB_INTEGRITY`.
Kapsam taraması: `exports` zaten taranıyor, `export_items` kapsamı 0037'de üst
kayıttan okunuyor → yeni sınıf yok, `GUARDED_TABLES` **25 kalır**.

**Çekirdek hizası (davranış sıkılaşması).** `record_export` tür, hedef türü,
hedef yol ve özeti **kendisi** doğrular (`receipt_text` + 64 küçük harf hex);
boş hedef yol artık `NULL`'a çevrilmez, `E_EXPORT_RECEIPT` ile reddedilir (dosya
yazıldıysa nereye yazıldığı bilinmelidir; `NULL` yalnız 24. şema öncesi eski
kayıtlarda okunur). Ürün yolunun beş çağrısı da geçerli değer geçiriyor —
arayüz davranışı değişmedi.

**Testler.** `exports` +1 (`export_receipt_is_born_evidenced_and_frozen`: 41
doğum + 13 dondurma/silme saldırısı + kalem saldırıları; 15 meşru/eski yol ve
no-op KABUL; eski `NULL` satırı kapı bilinçli düşürülerek okunur), `upgrade_tests`
+1 (`upgrade_keeps_exports_written_before_the_receipt_gate`), `scope_guard` ve iki
mevcut `exports` testi v60 kurallarına uyarlandı (hedef yol + 64 hex; sıra
sözleşmesi damga **doğuşta** sabitlenerek ölçülüyor — kapı gevşetilmedi) → **715
→ 717**. Yerelde **717/717 yeşil, 590,27 s**. fmt (iki manifest) / clippy
`-D warnings --all-targets` / masaüstü `check --all-targets` (1 dk 35 s) temiz.

**Dürüst olay (devralınan iş).** Bu dilim önceki koşuda (2026-11-19 15:20–16:30)
yazılmış ama **commit edilememişti**: o koşunun tam paketi 15:36'da **716/717**
düştü — `upgrade_reports_version_traces_left_by_the_previous_schema` çapraz kalemde
`DIM_SCOPE` yerine `DIM_INTEGRITY:export_items` aldı (yeni BI, çapraz daraltma
eklenmeden önce). 16:30'da yazılan `NOT EXISTS` daraltması bu testi düzeltiyor
ama **hiç yeşil ölçülmemişti**; bu tur ölçtü: 2/2 yeşil. Ayrıca mutasyon kapısı
16:30'da **EXIT=143** ile kesilmiş ve dosyayı **M20 mutantı** hâlinde bırakmıştı
(`included_ids` dizi kuralının yerinde `OR 0`). Bu tur sha256 farkından yakaladı
(`9fa5bd23…` ≠ kapının ölçtüğü `34cc9d65…`), hedeflenen metni **byte-birebir**
geri yazdı (46 desenin hepsi tam bir kez eşleşiyor, sha256 `34cc9d651f6bd310…`) ve
kapı betiğine **SIGTERM'de geri yazma** koruması ekledi (aynı hata tekrarlanmasın;
kaynak kopyası `0060_original.sql`).

**Kapı kanıtı.** Taban **YEŞİL — 31 test** (kusur KABUL 0/75, meşru 19/19, kapı
dışı 2/2; `measure_after.log`). **46/46 mutant KIRMIZI**, YEŞİL=0, ÖLÇÜLEMEDİ=0 (`gates_v60_final.log`); her
mutant tek cümleyi nötrler ve **hedef test** kırmızıya döner; kaynak her koşudan
sonra geri yazılır (sonda sha256 **birebir**, SIGTERM korumalı). Kapıdan **sonra**
yalnız **yorum** düzeltmesi yapıldı — ölçüm sayıları kaynak yorumunda ve test
başlığında 70/68 yazıyordu, gerçek log **75/73**; kurala dokunulmadı (kapının
ölçtüğü sürüm `34cc9d651f6bd310…`, yorum düzeltmesinden sonra
`a409be5258dc1d81…`). Düzeltmeden sonra ölçüm **tekrar** koşuldu: **0/75** kusur,
19/19 meşru, 2/2 kapı dışı; `export` testleri **31/31** yeşil.

**CI (ölçüldü).** Kod `ea93d44` (6 dosya, **+989/−60**) → run `44718606699`
**10/10 YEŞİL** (`dim-storage` **717** iki OS'ta: macos-15 **490,04 s** / ubuntu-latest **953,65 s**; `arayüz` 66 dosya / **1436** test, paket `index-wBKnjNHs.js` **693,95 kB** / gzip 185,22 kB; `masaüstü kabuk` `51 koştu · 0 düştü · 5 yoksayıldı` + yoksayılan koşu `5 passed`, `windows güvenli depo` 28, SBOM, sözleşmeler, belge worker'ı, sürüm paketi manifesti, kaynak arşivi yeşil).

**Dürüst sınırlar.** (a) `output_hash`in gerçekten yazılan dosyanın SHA-256'sı
olduğu **yeniden hesaplanmaz** (SQLite'ta dosya sistemi/SHA-256 yok); ölçülen
**biçim + değişmezlik**tir. (b) `destination_path`in diskte **var olduğu** ve
dosyanın oraya yazıldığı doğrulanmaz — yol bir **etiket**tir; yazma kanıtı
`write_atomic_verified`in geri okumasıdır. (c) `included_ids` ile kalem satırları
**çapraz denetlenmez** (makbuzun bildirmediği sürüm kalem olarak yazılabilir,
bildirdiği her sürüm için kalem doğmak zorunda değil; kalemsiz eski kayıtlar
meşru) — ürün yolu ikisini birlikte üretir. (d) `included_ids`teki bir kimlik
**hiç çözülemiyorsa** kapsam denetimi atlanır (sürüm silinmiş olabilir).
(e) `externalization_consent_id`nin var olan bir onaya işaret ettiği
doğrulanmaz (bugün hiçbir yol yazmıyor; sütun donar). (f) `cases` silme
`ON DELETE CASCADE`i **ölçüldü**: CASCADE eylemleri tetikleyicileri **çalıştırır**
— makbuzu olan dosya hard-delete edilemez; ürün yolu yalnız soft-delete
(`deleted_at`) kullanır. (g) Tetikleyicileri anahtarı olan saldırgan düşürebilir
(çekirdek doğrulaması ayrıca durur). (h) v60 öncesi tutarsız makbuzlar
**silinmez/düzeltilmez** (donar); onarım yolu yeni dışa aktarmadır. (i) Arayüz
davranışı değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04ai — kronoloji olayı «bu olay şu tarihte oldu» iddiasıdır: doğuşta tutarlı, kökeni ve dosyası donmuş (şema v61, 2026-11-20)

`events` satırı **kronolojinin hükmüdür**: «bu olay **şu tarihte** oldu, tarihin
kesinliği **şu**, rolü **şu**, kaynağı **şu**». Dört okuma yolu ona yaslanır:
`Timeline.tsx` tarih metnini (`eventDateText`), kesinlik rozetini
(`PRECISION_LABEL`), tarih rolü rozetini, durum rozetini («Onaylandı» ⇔
`status='confirmed'`, **değilse «Taslak»**) ve ham tarih notunu **doğrudan** bu
sütunlardan basar; `list_events` kronolojiyi **`sortable_date`** ile sıralar;
`csv_export` aynı üçlüden üretir; `review_commit::review_target_exists` inceleme
öğesinin hedefini bu tablodan çözer. Komşuları kapılıydı (`event_sources` 0032,
`claims` v32, `review_items` v40/v41) ama **olayın kendisi 0001'den beri
tetikleyicisizdi**; şemadaki tek kapı dört `CHECK` (kesinlik/rol/köken/durum
kapalı kümeleri) ve `unknown ⇒ sortable_date IS NULL` idi.

**Ölçülen kusur (gerçek SQLite v60; `scratch/dim-v61-probe/measure_v61.py`, log
`measure_before.log`):** 66 kusur yazımının **66'sı KABUL**, 16 meşru yazma
KABUL. Doğum (41): boş/boşluk/kırpılmamış/65 haneli kimlik; boşluk/boş/65 haneli
tür; boş/boşluk başlık; kırpılmamış açıklama; **gün** kesinliğinde ay biçimi /
dolgusuz gün / takvimde olmayan gün (`2025-02-30`); **ay** kesinliğinde tam gün
taşıyan değer, ay `13` ve ay `00`; **yıl** kesinliğinde ay taşıyan değer;
**aralık**ta tek tarih / ters aralık / yarım aralık / **ayırıcısız** değer
(`2025-03-01X2025-03-02`) / takvim dışı başlangıç ve bitiş; **yaklaşık**ta
`dün`; `sortable_date` tarihten **başka** (olay **yanlış sırada** görünür),
sıralama değeri yokken tarih var / tarih yokken sıralama değeri var (ekran tarih
uydurur), `unknown` + tarih taşır; `created_at` `dün` / kırpılmamış / serbest
metin (`'1'`), `updated_at` boş / kırpılmamış / `created_at`ten önce; doğuşta
`confirmed` (onay kullanıcı eylemidir), doğuşta silinmiş, **silinmiş dosyaya**
doğar; `date_raw` yalnız boşluk ve boş metin; `row_version` 0 (iyimser kilit ilk
yazmada çakışmayı göremez). UPDATE (25): satır **başka dosyaya** taşındı
(kaynak bağları eski dosyada kalır — INV-05), köken çevrildi (kullanıcı beyanı
«belge» oldu), kimlik yeniden adlandırıldı, `created_at` 2030'a çekildi, onay
geri alındı (`confirmed`→`draft`), onay arşive çevrildi, **taslak arşive
çevrildi**, sıralama değeri 1999'a çekildi, tarih çevrilip sıralama değeri
bırakıldı, silinmiş satır **geri getirildi**, silinmiş satırın içeriği değişti,
silme anında içerik değişti (sürüm artarak da), `row_version` sıfırlandı,
bilinmeyen tarihe gün eklendi, ay kesinliğine tam gün yazıldı, aralık ters
çevrildi / ayırıcısız yazıldı, başlık boşaltıldı, tür kırpılmamış yazıldı,
`updated_at` `dün`, tarih silinip sıralama değeri bırakıldı, içerik **sürüm
artmadan** değişti, yaklaşık kesinliğe `dün` yazıldı.

**Yazılan (`0061_timeline_event_evidence_guard.sql`, 2 tetikleyici):**

- `events` BI — `id` kırpılmış-dolu ≤64; `case_id` **var ve silinmemiş** dosyayı
  gösterir (fail-closed); `event_kind` kırpılmış-dolu ≤64; `title`
  kırpılmış-dolu; `description` kırpılmış; `date_raw` NULL ya da
  kırpılmış-dolu; doğuşta durum **yalnız `draft`**; `created_at`/`updated_at`
  ISO-8601 ve `updated_at ≥ created_at`; `row_version ≥ 1`; doğuşta
  **silinmemiş**; **tarih üçlüsü** tutarlı: `day` ⇒ `date(value) = value`
  (kanonik biçim **ve** takvim günü birlikte — `2025-02-30`, `2025-1-5`,
  `2025-03` düşer), `month` ⇒ `date(value || '-01') = value || '-01'`,
  `year` ⇒ dört hane, `approximate` ⇒ üç biçimden biri (gün/ay takvim
  doğrulamalı), `range` ⇒ `YYYY-AA-GG/YYYY-AA-GG` **ayırıcı dahil** ve
  başlangıç ≤ bitiş, `unknown` ⇒ tarih **yok**; `date_value` NULL ise
  `sortable_date` de NULL, değilse **sıralama değeri tarihin kendisi**
  (aralıkta başlangıç). Gün/ay için ayrıca `GLOB` yazılmadı: `date()` eşitliği
  kanonik biçimi zaten zorunlu kılar — **mutasyon kapısı** bu iki cümlenin
  etkisiz olduğunu gösterdi, cümleler çıkarıldı (kapı gevşetilmedi).
- `events` BU — `id`, `case_id`, `created_at` ve **`assertion_kind`** (kaydın
  kökeni) **donar**; durum yalnız `draft`→`confirmed` ilerler, `confirmed`
  terminal (`disputed`/`archived` eski satırlarda donmuş kalır — arayüz ikisini
  de «Taslak» bastığı için yeni geçiş **açılmadı**); `deleted_at` yalnız
  NULL→damga (**geri getirme yok**) ve silme anında içerik/durum değişmez;
  silinmiş satırda **hiçbir** alan değişmez; `row_version` **azalamaz** ve
  içerik değişiyorsa **artmalıdır** (eşit sürümle içerik yazan yol iyimser
  kilidi sessizce atlardı); içerik yazıldığında biçim ve tarih kuralları
  **yine** geçerli; yalnız durum/damga yazan geçişler eski satırı yeniden
  yargılamaz (onarım yolu: kullanıcı düzenlemesi).
- Ret `DIM_INTEGRITY:events` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
  `events` kapsam **referansı** taşımadığı için `GUARDED_TABLES` **25 kalır**
  (`case_id` burada donar).

**Çekirdek hizası (küçük sıkılaşma):** `create_event`/`update_event` artık
`date_raw`ı **kırpar**, yalnız boşluksa `NULL` yazar (`normalized_raw_date`) —
arayüz zaten kırpıyor, doğrudan IPC yolu da aynı sözleşmeye uysun. Diğer alanlar
zaten kırpılıyor; `normalize_event_date`/`sortable_event_date` kapının
kurallarıyla aynı sözleşmede. Ürün davranışı değişmedi.

**Testler:** `timeline` +1 (41 doğum kusuru + 25 UPDATE saldırısı RED; 9 meşru
doğum, 3 meşru düzenleme ve onay/silme geçişleri KABUL; çekirdek `date_raw` kırpması assert edilir),
`upgrade_tests` +1 (`upgrade_keeps_events_written_before_the_evidence_gate`: v60
tutarsız satırı **silinmez/düzeltilmez**, içeriği donar, silinebilir; yeni
biçimsiz olay doğamaz; kullanıcı düzenlemesi onarır) → **717 → 719**; yerelde
**719/719 yeşil** (süre aşağıda). fmt (iki manifest) / clippy `-D warnings
--all-targets` / masaüstü `check --all-targets` temiz.

**Kapı:** taban **YEŞİL**, **44/44 mutant KIRMIZI**, YEŞİL 0, ÖLÇÜLEMEDİ 0
(`gates_v61.py`, log `gates_v61_final.log`); kaynak geri yazıldı sha256
**birebir** (`d6e56662408f3eba…`). **Dürüst (üç koşu):** ilk koşu **34/44** —
10 mutant yaşadı; kök nedenler ölçüldü: (i) üç cümle **etkisiz** çıktı
(gün/ay `GLOB`ları `date()` eşitliğinin kapsadığı kanonik biçimi tekrar
ediyordu; `unknown THEN 1` çift kuralıyla aynı satırı yakalıyordu) → cümleler
**koddan çıkarıldı**, kapı gevşetilmedi; (ii) yedi mutantın senaryosu eksikti
(kırpılmamış kimlik, kırpılmamış/serbest `created_at`, kırpılmamış `updated_at`,
ayırıcısız ve takvim dışı **sıralı** aralık, sürüm artmadan içerik, silme
anında içerik+sürüm, taslak→arşiv, yaklaşık kesinliğe `dün`) → saldırılar
**eklendi**. İkinci koşu **43/44** (yalnız `aralık biçimi (BU)` yaşadı: aralık
saldırıları doğum yolundaydı) → güncelleme yoluna ayırıcısız aralık saldırısı
eklendi, **44/44** yeniden ölçüldü. İlk iki koşunun hükümleri kanıt sayılmadı.

**CI (ölçüldü).** Kod `4e95aa3` (4 dosya, **+892/−3**) → run `44736733863`
**10/10 YEŞİL** (`dim-storage` **719** iki OS'ta: macos-15 **467,30 s** /
ubuntu-latest **782,73 s**; `arayüz` 66 dosya / **1436** test, paket
`index-wBKnjNHs.js` **693,95 kB** / gzip 185,22 kB — değişmedi; `masaüstü kabuk`,
`windows güvenli depo`, SBOM, `sözleşmeler`, belge worker'ı, kaynak arşivi,
sürüm paketi manifesti yeşil).

**Dürüst sınırlar.** (a) Takvim doğrulaması `date()` eşitliğidir; 1582 öncesi
(Gregoryen proleptik) tarihlerde tarihçi beklentisi farklı olabilir — ölçülen
**biçim + takvimde varlık**tır. (b) Damga **gelecekte** olabilir; yalnız biçim
ve `updated_at ≥ created_at` denetlenir. (c) `date_raw`ın **içeriği**
doğrulanmaz (yalnız kırpılmış-dolu); ham ifadenin tarihle uyumu ürün kararıdır.
(d) `title`/`description` **içeriği** ve başlık uzunluğu denetlenmez. (e)
`event_kind` serbest metindir (kapalı küme değil) — yalnız kırpılmış, dolu,
≤64. (f) `assertion_kind` **doğruluğu** kanıtlanmaz: kullanıcı «belge» der,
kapı inanır; ölçülen **değişmezlik**tir. (g) `event_sources` bağının olayı
gerçekten desteklediği doğrulanmaz (v32 yalnız kapsamı ölçer). (h) Yanlış
silme **geri alınamaz** (silinmiş satır donar); onarım yolu yeni olaydır.
(i) v61 öncesi tutarsız satırlar **silinmez**; içerik yazılırsa kapıya takılır
(donar), kullanıcı düzenlemesi normalize ettiği için **onarır**. (j)
`row_version` monotonluğu eşzamanlı iki yazarı sıraya **sokmaz**; çakışmayı
çekirdek `expected_row_version` ile bildirir, şema yalnız geriye sarmayı
durdurur. (k) `cases` hard-delete CASCADE'i olayları da götürür (ürün yolu
soft-delete; 0060'ta ölçülen sınıf). (l) Tetikleyicileri anahtarı olan
saldırgan düşürebilir (çekirdek doğrulaması ayrıca durur). (m) Arayüz
davranışı değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04aj — inceleme hükmü (karar + çelişki değerlendirmesi) doğuşta kanıtlı, sonra değişmez (şema v62, 2026-11-20)

`review_decisions` satırı «bu öğe için **şu karar** verildi, gerekçesi **şu**,
veren **şu**, zaman **şu**» iddiasıdır; `contradiction_assessments` satırı «iki
kaynak **şu yönde** değerlendirildi» iddiasıdır. Üç okuma yolu bu satırlara
yaslanır: `review_history` → `Review.tsx` karar geçmişi paneli (karar, gerekçe,
inceleyen, `decided_at`, düzenlenen yük ve «okunamadı» uyarısı), 
`list_contradiction_assessments` → ekranın «son değerlendirme» satırı (en
yenisi gösterilir), `commit_review_decision` → hedef çözümü ve karar sayımı.
0007 göçünün kendi yorumu kayıtların **eklemeli** olduğunu söyler: düzeltme
yolu yeni satırdır. Komşuları kapılıydı (`review_items` 0033/0034/0040/0041
kapsam) ama **hükmün kendisi** 0001/0007'den beri tetikleyicisizdi; tek kapı
`decision` / `verdict` CHECK'leriydi.

**Ölçülen kusur (gerçek SQLite v61; `scratch/dim-v62-probe/measure_v62.py`, log
`measure_before.log`):** 61 kusur yazımının **57'si KABUL** (4'ü zaten RED —
iki CHECK, iki FK), 12 meşru yazma KABUL. Doğum (37 KABUL): boş / boşluk /
kırpılmamış / 65 haneli kimlik; boş / boşluk / kırpılmamış inceleyen;
kırpılmamış gerekçe; damga boş / `dün` / kırpılmamış / takvim dışı gün
(`2026-02-30`) / olmayan saat (`99:00:00`); düzenlenen yük **bozuk JSON**;
`edit` kararı **yüksüz**; **kapanmış öğeye** karar, **yerine geçmiş** öğeye
karar, **hedefi yazılmış** öğeye ikinci karar, **silinmiş dosyanın** öğesine
karar; değerlendirmede **başka dosya** kimliği, silinmiş dosya, **çelişki
adayı olmayan** öğe (ekran o paneli hiç çizmez — sessiz yetim satır),
boş/boşluk/kırpılmamış inceleyen ve gerekçe, boş/`dün`/kırpılmamış /
takvim dışı gün / olmayan saat / boşluk ayırıcılı damga.
UPDATE (15 KABUL): kararın `edit`→`reject` çevrilmesi, inceleyenin / gerekçenin
/ damganın değiştirilmesi, düzenlenen yükün değiştirilmesi/silinmesi, kimliğin
yeniden adlandırılması, satırın **başka öğeye** taşınması; değerlendirmede
hükmün çevrilmesi (`conflict`→`explained`), gerekçe / inceleyen / damga,
satırın **başka dosyaya** ve **başka öğeye** taşınması, kimlik. DELETE (5
KABUL): karar satırı, değerlendirme satırı, kararı olan öğe (CASCADE),
değerlendirmesi olan öğe (CASCADE), kararı olan dosya (CASCADE).

**Yazılan (`0062_review_verdict_evidence_guard.sql`, 3 + 3 tetikleyici):**

- `review_decisions` BI — `id` kırpılmış-dolu ≤64; `review_item_id` **var
  olan**, **açık** (`state IN ('pending','needs_more_evidence')`), yerine
  geçmemiş, hedefi yazılmamış ve **dosyası silinmemiş** öğeyi gösterir
  (çekirdeğin iki karar yolunun da ön koşulu; tek `EXISTS` cümlesinde);
  `reviewer_label` kırpılmış-dolu; `rationale` kırpılmış; `decided_at`
  kırpılmış ISO-8601 ve **gerçek gün + gerçek saat** (`date(substr(...))` /
  `time(substr(...))` eşitliği); `edited_payload` NULL ya da **geçerli JSON**;
  `decision='edit'` ise yük **dolu**.
- `review_decisions` BU — **yedi sütunun hepsi donar** (sicil eklemelidir).
- `review_decisions` BD — satır **silinemez** (silinen karar, öğe durumunu
  gerekçesiz bırakır; ekranın gösterdiği sicil bu tablodur).
- `contradiction_assessments` BI — `id` kırpılmış-dolu ≤64; satır **aynı
  dosyanın**, **çelişki adayı** ve **dosyası silinmemiş** öğesini gösterir
  (`case_id` öğenin dosyasıyla eşit — INV-05); `reviewer_label` kırpılmış-dolu;
  `rationale` kırpılmış; `assessed_at` kırpılmış ISO-8601 + gerçek gün/saat.
- `contradiction_assessments` BU — **yedi sütunun hepsi donar**.
- `contradiction_assessments` BD — satır **silinemez**.
- Ret `DIM_INTEGRITY:review_decisions` / `DIM_INTEGRITY:contradiction_assessments`
  → `StorageError::Integrity` → **`E_DB_INTEGRITY`**. İki tablo kapsam
  **referansı** taşımadığı için `GUARDED_TABLES` **25 kalır**
  (`contradiction_assessments.case_id` burada **öğenin dosyasına eşitlik**
  olarak donar).

**Çekirdek hizası (küçük sıkılaşma):** `decide_review_item` artık `edit`
kararını **düzenlenen yük olmadan** kabul etmez (`InvalidSetting` →
`E_DB_SETTING`). Arayüz düzenleme yolunda yükü her zaman gönderir;
`commit_review_decision` yükü kendisi üretir. Diğer yazma yolları zaten
kırpıyor ve `now_iso()` damgası kurallarla aynı sözleşmede — ürün davranışı
değişmedi.

**Testler:** `claims` +2 (`review_decision_is_born_on_an_open_item_and_frozen_after_verdict`:
21 doğum + 8 UPDATE + 3 silme saldırısı RED; 4 meşru doğum ve no-op KABUL;
çekirdeğin `edit` yüksüz reddi assert edilir — `contradiction_assessment_is_born_on_its_own_candidate_and_frozen`:
18 doğum + 7 UPDATE + 3 silme saldırısı RED; dört hüküm, aynı öğeye çok satır
ve başka dosyanın kendi adayı KABUL), `upgrade_tests` +1
(`upgrade_keeps_review_verdicts_written_before_the_evidence_gate`: v61 tutarsız
kararı ve değerlendirmesi **silinmez/düzeltilmez**, donar; yeni biçimsiz hüküm
doğamaz; meşru yeni karar ve değerlendirme yazılabilir) → **719 → 722**;
yerelde **722/722 yeşil, 669,33 s** (`libtest_v62.log`); fmt (iki manifest) /
clippy `-D warnings --all-targets` / masaüstü `check --all-targets` temiz.
**İki mevcut test uyarlandı (dürüst):** `review_history_marks_unreadable_edited_payload`
v62 öncesi bozuk satırı taklit ettiği için kapıyı **bilinçli** düşürür (okuyucu
hâlâ «okunamadı» demeli); `assessment_history_orders_by_instant_not_by_text`
değişken kesir genişliğini sabitliyordu — v62 damgayı dondurduğu için sonda
satırları kapı düşürülerek yazar; sınanan şey `list_…` sıralamasıdır.

**Kapı (`gates_v62.py`, log `gates_v62_final.log`):** taban YEŞİL, **40/40
mutant KANIT** (her mutantta: SQL yükleniyor, hedef delik **açılıyor**, meşru
yollar **korunuyor** ve Rust testi KIRMIZI); kaynak geri yazıldı sha256
**birebir** (`cccf9242faf01377…`). **Dürüst (iki koşu):** ilk koşu **36/40** —
dört mutant (karar damgasının `LIKE` biçimi; değerlendirme damgasının biçim /
takvim / saat cümleleri) YEŞİL kaldı çünkü saldırı listesi **ayırıcısı boşluk**
olan damgayı ve değerlendirmede **takvim dışı gün / olmayan saat**i hiç
denemiyordu; dört saldırı **hem sondoya hem Rust testlerine** eklendi, **40/40**
yeniden ölçüldü. İlk koşunun hükümleri kanıt sayılmadı; kapı gevşetilmedi.

**CI (ölçüldü).** Kod `40224d5` (4 dosya, **+949/−2**) → run `44758314186`
**10/10 YEŞİL** (`dim-storage` **722** iki OS'ta: macos-15 **491,98 s** /
ubuntu-latest **1294,48 s**; `arayüz` 66 dosya / **1436** test, paket
`index-wBKnjNHs.js` 693,95 kB / gzip 185,22 kB — **değişmedi**; `masaüstü kabuk`,
`windows güvenli depo`, SBOM, `sözleşmeler`, belge worker'ı, kaynak arşivi, sürüm
paketi manifesti yeşil).

**Dürüst sınırlar.** (a) Damga **gelecekte** olabilir; yalnız biçim + gerçek
gün/saat denetlenir. (b) `edited_payload`ın **içeriği** denetlenmez (yalnız
JSON geçerliliği); `reviewer_label` serbest metindir, kimlik doğrulanmaz.
(c) Hükmün **doğruluğu** değil, kaydın ön koşulları ve değişmezliği ölçülür.
(d) v62 öncesi tutarsız satırlar **silinmez/düzeltilmez** (BU dondurur);
onarım yolu yeni satırdır, ancak öğe kapalıysa yeni karar doğamaz. (e) BD,
CASCADE ile gelen silmeleri de durdurur: kararı/değerlendirmesi olan öğe ya da
dosya **hard-delete** edilemez (ürün yolu soft-delete; 0060/v61 ile aynı
sınıf). (f) Tetikleyicileri anahtarı olan saldırgan düşürebilir (çekirdek
doğrulaması ayrıca durur). (g) Arayüz davranışı değişmedi; gerçek pencere +
görsel QA **insan bekliyor**.

## F08-T04ak — sayfanın geometrisi, kökeni ve etiketi kanıtlanabilir olmalı (şema v63, 2026-11-22)

**Neden.** `pages` satırı «bu sayfa **şu** geometride, **şu** kökenle okundu»
iddiasıdır ve dört yerde kanıt olarak okunur:

- `text_origin` — arama `ocr`/`mixed` filtresi (`search_page_windowed`
  `p.text_origin IN ('ocr','mixed')`), dual reader rozeti ve kapsam raporunun
  «OCR kullanılan sayfa» sayımı (`sum(CASE WHEN p.text_origin IN ('ocr','mixed')
  …)`) **doğrudan** bu sütundan gelir. `ocr`→`native` çeviren satır, OCR'sız
  görünen bir belge üretir.
- `width`/`height`/`rotation`/`coordinate_space` — maskeleme bölgesi sayfa
  sınırı kapısı (v54) bölgeyi **bu** kutuya göre doğrular; ekran kutuları bu
  geometriyle çizer. Doğrulandıktan sonra kutu küçültülürse «sayfa içinde»
  hükmü **geriye dönük** bozulur: maske sayfa dışına taşar (hiçbir şey
  maskelenmez) ya da fazla maskeler.
- `quality_flags` — okuma uyarıları ekranda bu listeden basılır.
- `page_label` — basılı etiket; fiziksel numaradan türetilmez (v4 kuralı).

Sayfa metni `text_segments`tedir ve orada kapsam kapılıdır; bu tablo metin
taşımaz, **hangi sayfanın hangi koşuda hangi geometriyle okunduğunu** taşır.
Tablo 0001'den beri tetikleyicisizdi: tek kapı `page_index >= 0` CHECK'i ve
(`parse_run_id`,`page_index`) UNIQUE'iydi.

**Ölçülen kusur (düzeltme öncesi, gerçek SQLite v62; sonda
`~/.hermes/cache/scratch/dim-v63-probe/measure_v63.py`, log
`measure_before.log`):** 47 yazımın **42'si KABUL**, 5'i zaten RED (negatif
indeks CHECK, bilinmeyen köken CHECK, var olmayan blob/koşu FK, mükerrer sayfa
UNIQUE); 7 meşru/zararsız yol da KABUL. Yani **35 kusur** yazılabiliyordu.
23 doğum kusuru: **NULL** (SQLite'ta `TEXT PRIMARY KEY` NULL'a izin verir) /
boş / boşluk / kırpılmamış / 65 haneli kimlik; sayfa indeksi / genişlik /
yükseklik sütunlarına **metin** (SQLite karşılaştırmasında `TEXT` her zaman
sayıdan büyüktür → aralık cümleleri tek başına yetmez); boş ve boşluk etiket;
**sıfır ve negatif** genişlik/yükseklik; negatif ve 360 döndürme;
boş/boşluk/kırpılmamış koordinat uzayı; bozuk JSON, nesne ve metin olmayan
öğe taşıyan uyarı listesi. 12 UPDATE kusuru: sayfanın başka indekse ve
**başka koşuya** taşınması, `ocr`→`native` çevrilmesi, geometrinin
küçültülmesi/sıfırlanması, döndürme/uzay/uyarı/etiket değişimi, kimliğin
yeniden adlandırılması, türev blob bağlanması. Düzeltmeden sonra aynı sonda
**7 KABUL** (4 meşru doğum, no-op, 2 silme) / **40 RED** ölçtü.

**Yazılan (`0063_page_evidence_guard.sql`, 2 tetikleyici):**

- BI — `id` kırpılmış-dolu ≤64; `page_index` **tam sayı** ve ≥ 0; `rotation`
  0..359; `page_label` NULL ya da kırpılmış-dolu; `width`/`height` NULL ya da
  **sayısal ve pozitif** (sıfır/negatif/metin alanlı sayfa kutu olamaz);
  `coordinate_space` NULL ya da kırpılmış-dolu; `quality_flags` **metin
  dizisi** (v60'ın `included_ids` deseni: `json_valid` yoksa da reddet).
- BU — on bir kanıt sütunu **donar**; no-op takılmaz. Bugün
  `derivative_blob_id`i yazan ürün yolu yok (yalnız `ON DELETE SET NULL`);
  yine de donar, çünkü sayfanın gösterim görüntüsü bağlanırsa hangi görüntünün
  kanıt olduğu sonradan değişmemelidir.

Ret `DIM_INTEGRITY:pages` → `E_DB_INTEGRITY`; tablo `case_id` taşımaz →
`GUARDED_TABLES` **25 kalır**. Çekirdek davranışı değişmedi: sayfaları yazan
tek ürün yolu (`store_pages`) zaten kırpılmış etiket, pozitif geometri ve
`Vec<String>` uyarı yazıyor.

**Testler.** `parsing` +1 (`page_evidence_is_born_well_formed_and_frozen`:
23 doğum kusuru (NULL kimlik, metin sayfa indeksi/genişlik/yükseklik ve boş
etiket/uzay dahil) ve 12 UPDATE saldırısı RED; NULL geometri, döndürme 90,
uyarı listesi ve no-op KABUL; saldırılardan sonra satır aynı kanıtı taşır),
`upgrade_tests` +1 (`upgrade_keeps_pages_written_before_the_evidence_gate`:
v62 tutarsız satır silinmez/onarılmaz, donar; yeni biçimsiz doğamaz, meşru
sayfa yazılabilir).

**Dürüst sınırlar.** (a) Silme bu dilimin dışında: sayfa satırı **silinemez
değildir** — dosya/sürüm silme ürün yoludur ve `plaintext` kaskad testi
içeriğin temizlendiğini bu yolla ölçer; BD eklenirse o güvenlik testi düşerdi.
Sayfa silmenin izi `parse_runs.pages_processed` ile sayfa sayısı arasındaki
farkta kalır; bunu **sayfan** bir okuyucu yok (dürüst sınır). (b) Geometrinin
**doğru** olduğu (PDF kutusuyla birebir) doğrulanmaz; yalnız yazıldıktan sonra
değişmediği ölçülür. (c) `coordinate_space` kapalı küme değildir: işçi yeni bir
uzay adı üretebilir; boş/kırpılmamış değer reddedilir. (d) `quality_flags`
içeriği (uyarı metinleri) denetlenmez, yalnız metin dizisi olması. (e)
`page_label`in belgedeki gerçek etikete denkliği doğrulanmaz. (f) v63 öncesi
tutarsız sayfalar **silinmez/onarılmaz** (BU dondurur); onarım yolu **yeni
ayrıştırma koşusu**. (g) Tetikleyicileri anahtarı olan saldırgan düşürebilir.
(h) Arayüz değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04al — denetim defteri eklemelidir: doğuşta zincire bağlı, sonra değişmez (şema v64, 2026-11-23)

**Neden bu dilim:** `audit_events` ürünün **«bu işlem yapıldı»** defteridir ve üç
okuyucu ona yaslanır: (1) `audit::walk_chain` / `verify_audit_chain` /
`audit_chain_status` zinciri yürür — `prior_hash`in **önceki satırın**
`event_hash`ine eşitliğini ve özetin yeniden hesapla tuttuğunu ölçer; Settings
ekranı «zincir doğrulandı / kırıldı» satırını buradan basar; (2) `ai.rs`
`supersede` sayımını ve sağlayıcı yanıtının kayda geçtiğini
(`metadata_allowlist`) **doğrudan** bu satırlardan okur; (3) `blob_disk.rs`
temizlik kayıtlarını buraya yazar. Tablo 0001'den beri **tetikleyicisizdi**:
tek kapı `id` UNIQUE'iydi. `append_audit` zinciri kendisi kurar, ama doğrudan
SQL yazan yol bunu atlayabiliyor; doğmuş satır sonradan **her** sütunuyla
değiştirilebiliyor, silinebiliyordu — yani defterin «eklemeli» olduğu iddiası
şemada yazmıyordu.

**Ölçülen (düzeltme öncesi, gerçek SQLite v63; sonda
`scratch/dim-v64-probe/measure_v64.py`, log `measure_before.log`):** 41 yazımın
**41'i KABUL** (39 kusur + 1 meşru ekleme + 1 no-op güncelleme).
29 doğum kusuru: boş / kırpılmamış / 65 haneli kimlik; boş, yalnız boşluk ve
kırpılmamış kategori / işlem / aktör; boş ve kırpılmamış `entity_id`; bozuk
JSON, dizi, `null` ve metin `metadata_allowlist`; 63 haneli / büyük harf / hex
olmayan `event_hash`; `dün`, yalnız gün, olmayan gün (`2026-02-30`), olmayan
saat (`99:00:00`) ve saniyesiz damga; **zincir kopuk** `prior_hash`, zincirde
satır varken `prior_hash NULL`, 63 haneli ve büyük harf prior; açıkça **düşük
`sequence`** (satır defterin başına düşer). 9 UPDATE kusuru: işlem, kategori,
aktör, metadata, özet, prior, damga, kimlik ve `entity_id` çevrildi.
1 silme kusuru: kayıt silindi.

**Yazılan (`0064_audit_ledger_evidence_guard.sql`, 4 tetikleyici):**
`audit_events_evidence_birth` (BI) — `id`/`category`/`operation`/`actor_label`
kırpılmış-dolu ≤64, `entity_id` NULL ya da kırpılmış-dolu ≤64,
`metadata_allowlist` **geçerli JSON nesnesi** (dizi/`null`/metin/bozuk reddedilir),
damga **sabit genişlik** `YYYY-MM-DDTHH:MM:SS.uuuuuuZ` ve **gerçek** takvim
günü/saati (`date()`/`time()` eşitliği; `2026-02-30` normalleşir), `event_hash`
**64 küçük harf hex**, `prior_hash` NULL ya da 64 küçük harf hex ve
**defterin son satırının `event_hash`i** (boş defterde NULL) — zincir bağı
**doğuşta** kurulur. `audit_events_evidence_appended` (AFTER INSERT) — satır
defterin **sonuna** eklenir: açıkça verilen düşük `sequence` reddedilir
(AUTOINCREMENT değeri BEFORE tetikleyicide tanımsız olduğu için kural AFTER'da
ölçülür). `audit_events_evidence_frozen` (BU) — dokuz kanıt sütunu **donar**;
no-op yazma takılmaz. `audit_events_evidence_no_delete` (BD) — kayıt
**silinemez** (tabloda CASCADE bağı yok; dosya silme ürün yolu bu defteri
etkilemez). Ret `DIM_INTEGRITY:audit_events` → `E_DB_INTEGRITY`. Tablo
`case_id` taşımaz → `GUARDED_TABLES` **25 kalır**. **Çekirdek değişmedi**:
`append_audit` zaten aynı kuralları uyguluyordu; bu dilim şemayı ona hizaladı.

**Testler:** `audit` +1
(`audit_ledger_is_chained_at_birth_and_never_rewritten_or_erased`: 3 meşru
kayıt kapıdan geçer; **32 doğum saldırısı** (26 biçim + 4 zincir bağı +
1 düşük `sequence` + boş defterde dolu prior), 8 UPDATE saldırısı, 1 DELETE
saldırısı reddedilir; no-op güncelleme kabul; reddedilen satır defterde
kalmaz; her saldırıdan sonra zincir sağlam), `upgrade_tests` +1
(`upgrade_keeps_audit_ledger_written_before_the_evidence_gate`: v63'te
`append_audit`in yazdığı **geçerli** satır güncellemeden sonra da geçerli,
güncelleme kendi kaydını sona ekler, yeni satır eski satıra bağlanır,
biçimsiz yazma ve silme reddedilir). İki okuma testi (`broken_metadata_…`,
`tampered_…`, `removed_middle_…`) v64 kapısını **bilinçli** düşürür — sınadıkları
şey okuyucu/doğrulayıcı. `dim-storage --lib` **766 → 768**.

**Kapı kanıtı (`scratch/v64-gates.py`, log `v64_gates_run1.log`):**
**16/16 KIRMIZI · YEŞİL 0 · ÖLÇÜLEMEDİ 0** — her mutant tek cümleyi nötrler ve
hedef saldırı **kabul** olur (kimlik/kategori/işlem/aktör/entity kuralları,
metadata `json_valid`+nesne birlikte, metadata nesne, damga GLOB, damga gerçek
gün, özet biçimi, zincir bağı, AFTER sıra kuralı, tüm sütun donması, yalnız
işlem donması, silme yasağı). Her mutanttan sonra kaynak sha256 **birebir**
geri yazıldı. **Dürüst (iki koşu):** ilk koşu **15/17** — M9 (damga GLOB) ve
M11 (`time()`) YEŞİL kaldı: testte **boşluk ayırıcılı** damga ve **artık
saniye** (`10:00:60`) saldırıları yoktu; ikisi testlere eklendi, ayrıca
`time()` cümlesinin `date()` eşitliğine **eşdeğer** olduğu ölçüldü
(`date()` geçersiz saatte de NULL döner) → cümle koddan **çıkarıldı**; ikinci
koşu **16/16**. İlk koşunun hükümleri kanıt sayılmadı, kapı gevşetilmedi.

**CI (ölçüldü):** kod `ececa36` (4 dosya, **+605/−1**) → run `45079690771`
**10/10 YEŞİL** (`dim-storage` **768** iki OS'ta: `macos-15` **495,45 s** /
`ubuntu-latest` **529,88 s**; `arayüz` **67 dosya / 1482 test** — paket
`index-Ce2oEuEs.js` **731,38 kB** / gzip **194,04 kB**; `masaüstü kabuk`,
`windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı, sürüm paketi
manifesti, kaynak arşivi yeşil).

**Dürüst sınırlar:** (a) `event_hash`in **içeriği** SQLite'ta yeniden
hesaplanamaz (SHA-256 yok): kapı biçimi ve **zincir bağını** doğrular; özetin
doğruluğunu `walk_chain` çalışma anında ölçer. (b) Zincir kuralı **kuyruğa**
bağlar: geçmişte kopmuş bir defter onarılmaz, yeni satırlar son satıra
bağlanmaya devam eder (v64 öncesi tutarsız satırlar silinmez/donar). (c) Bir
saldırgan tetikleyicileri düşürüp sahte **tutarlı** zincir yazabilir; kapı iç
tutarlılığı ölçer, kimliği (actor_label) doğrulamaz. (d) `metadata_allowlist`
içeriği anahtar beyaz listesine göre denetlenmez (yalnız nesne + geçerli JSON).
(e) Eşzamanlı iki yazar aynı `prior_hash`i seçemez: SQLite tek yazar olduğu
için `append_audit` sıralıdır; çok süreçli yazma bu dilimde ölçülmedi.
(f) Tetikleyiciler `DROP TRIGGER` ile düşürülebilir — ürün yolu bunu yapmaz,
anahtarı olan saldırgan yapabilir. (g) Arayüz değişmedi; gerçek pencere +
görsel QA **insan bekliyor**.

## F08-T04am — dosya tarafı ve rolü kanıtlı doğar, sonra değişmez; birleştirme makbuzu silinemez (şema v65, 2026-11-23)

**Neden bu dilim:** `actors` + `case_participants` + `actor_merges` üçlüsü
birlikte **«bu kişi bu dosyada şu rolde taraf»** iddiasını taşır.
`Parties.tsx` listeyi `display_name` + `kind` + `role` + `role_note`
sütunlarından **doğrudan** basar; birleştirme geçmişini
`actor_merges.moved_participants`ten çözer (`case_ids` ile dosya süzülür).
`add_case_party` birleştirilmiş (`merged`) tarafa yeni rol yazmaz;
`merge_actors` kaynağın rollerini hedefe taşır ve makbuz bırakır;
`undo_actor_merge` rolleri kaynağa döndürür ve kaynağın **önceki inceleme
durumunu** birebir yazar. Üç tablonun da 0001/0025'ten beri **hiçbir
tetikleyicisi yoktu**: ad kırpılmadan, rol boş, `normalized_names` bozuk
JSON, `review_state` keyfî çevrilebilir, katılımcı **başka dosyaya**
taşınabilir, silinmiş satır geri getirilebilir, birleştirme makbuzu
silinebilirdi. Şemadaki tek kapılar `kind`/`review_state` CHECK'leri, iki FK
ve `UNIQUE (case_id, actor_id, role)` idi.

**Ölçülen (düzeltme öncesi, gerçek SQLite v64; sonda
`scratch/dim-v65-probe/measure_v65.py`, log `measure_before.log`):**
**92 yazımın 92'si KABUL** — 83 kusur (29 `actors` + 33 katılımcı +
21 makbuz) + 9 meşru yol. Doğum (57): kimlik boş/boşluk/kırpılmamış/65 hane;
ad boş/boşluk/kırpılmamış/201 hane/CR-LF-TAB; `normalized_names` bozuk
JSON/nesne/sayı dizisi; doğuşta `merged`, doğuşta `merged_into`, doğuşta
silinmiş; damga `dün`/geri; `row_version` 0; katılımcıda **silinmiş dosya**,
**silinmiş aktör**, **birleştirilmiş aktöre rol**, boş/kırpılmamış/81 hane
rol, rolde TAB, boş/kırpılmamış/501 hane not, `valid_from` `dün`/takvim dışı
gün, `valid_to` `dün`, **ters aralık**; makbuzda **aynı kaynak/hedef**,
silinmiş kaynak, `merged` kaynak, `source_previous_state='merged'`,
bozuk/nesne/metin `moved` listesi, eksik `id`, boş/kırpılmamış/201 hane
inceleyen, damga `dün`, doğuşta `undone_at`. UPDATE/DELETE (26): aktörde
ad/`kind`/`normalized_names`/`created_at` çevirme, `accepted→rejected`,
merged olmadan `merged_into`, merged hedefi çevirme, `row_version` 0, damga
`dün`, **silme**; katılımcıda **başka dosyaya taşıma**, rol/not/`valid_from`
çevirme, **keyfî `actor_id`**, silinmişi **geri getirme**, silinmiş satırda
içerik, `row_version` 0, damga `dün`, **silme**; makbuzda
hedef/taşınanlar/inceleyen/damga çevirme, geri alınmışı geri alma, **silme**.
Meşru yollar (9): aktör ve katılımcı doğumu (boş ve geçerli aralıkla),
aktör/katılımcı no-op, geri dönüşümlü silme, **birleştirme**, **geri alma**,
hazır birleştirmenin geri alınması.

**Yazılan (`0065_case_party_evidence_guard.sql`, 3 + 3 + 3 tetikleyici):**
`actor_identity_evidence_guard_bi` — `id` kırpılmış-dolu ≤64;
`display_name` kırpılmış-dolu ≤200 ve CR/LF/TAB taşımaz; `normalized_names`
**metin dizisi** (bozuk JSON/nesne/sayı dizisi reddedilir); doğuşta `merged`
değil ve `merged_into_actor_id` **NULL**; iki damga ISO-8601 + **gerçek
gün/saat** ve `updated_at ≥ created_at`; `row_version ≥ 1`; doğuşta
silinmemiş. `actor_identity_evidence_guard_bu` — kimlik sütunları **donar**
(`id`, `display_name`, `kind`, `normalized_names`, `created_at`,
`created_by_local_actor_id`); `deleted_at` yalnız NULL→damga, silinmiş satır
**hiç** değişmez; `review_state` yalnız **iki geçiş**: `merged`a giriş
(hedef var, açık, kendisi değil, hedef merged değil) ve `merged`dan çıkış
(yalnız kendisini kaynak gösteren bir **makbuz** varken, `merged_into`
NULL'a); başka durum çevrimi yok; `row_version` azalamaz, bu iki sütun
değişiyorsa **artmalı**. `actor_identity_evidence_guard_bd` — satır
**silinemez**. `case_participant_evidence_guard_bi` — `id` kırpılmış-dolu
≤64; `case_id` **var ve silinmemiş** dosya; `actor_id` **var, silinmemiş ve
`merged` değil**; `role` kırpılmış-dolu ≤80 ve CR/LF/TAB taşımaz;
`role_note` NULL ya da kırpılmış-dolu ≤500; `valid_from`/`valid_to` NULL ya
da gerçek günle başlayan ISO damga ve aralık **ters değil**; damga kuralları;
`row_version ≥ 1`; doğuşta silinmemiş.
`case_participant_evidence_guard_bu` — `id`, `case_id`, `role`, `role_note`,
`valid_from`, `valid_to`, `created_at`, `created_by_local_actor_id`
**donar**; `actor_id` yalnız bir **makbuzun** taşıdığı satır için ve yalnız
makbuzun hedefi (ileri) ya da kaynağı (geri alma, kaynak hâlâ `merged`)
yönünde değişir; `deleted_at` yalnız NULL→damga, silinmiş satır değişmez;
`row_version` azalamaz, `actor_id`/`deleted_at` değişiyorsa **artmalı**.
`case_participant_evidence_guard_bd` — satır **silinemez** (ürün yolu
`deleted_at`). `actor_merge_receipt_guard_bi` — `id` kırpılmış-dolu ≤64;
kaynak/hedef **var, silinmemiş, farklı** ve ikisi de `merged` değil;
`source_previous_state` kapalı küme (`merged` olamaz); `moved_participants`
**nesne dizisi** (`id` + `caseId` metin — sözleşme serde `camelCase`tir;
eski bozuk satır `json_valid` ile ayrılır); inceleyen kırpılmış-dolu ≤200;
damga ISO + gerçek gün/saat; doğuşta `undone_at` NULL.
`actor_merge_receipt_guard_bu` — makbuz sütunları **donar**; `undone_at`
yalnız NULL→damga, sonra **terminal**. `actor_merge_receipt_guard_bd` —
makbuz **silinemez**. Ret `DIM_INTEGRITY:actors` /
`DIM_INTEGRITY:case_participants` / `DIM_INTEGRITY:actor_merges` →
**`E_DB_INTEGRITY`**. Kapsam sütunu taşınmadığı için `GUARDED_TABLES`
**25 kalır**. **Çekirdek hizası:** yazma yolları zaten kırpıyor, boşu
reddediyor ve `now_iso()` kullanıyor — ürün davranışı değişmedi; kapı iki
çekirdek kuralını (birleştirilmiş tarafa rol eklenmez, birleştirme çakışmayı
önceden reddeder) şemada da tutar.

**Kapı (`scratch/v65-gates.py`, log `v65_gates_run3.log`):** taban YEŞİL,
**31/31 mutant KIRMIZI · YEŞİL 0 · ÖLÇÜLEMEDİ 0**, her mutantta hedef delik
açıldı; kaynak sha256 **birebir** geri yazıldı (`77c6c3cb20e253ce…`).
**Dürüst (üç koşu):** ilk koşu **21/31** — on mutant YEŞİL kaldı: testte
boş/kırpılmamış kimlik, takvim dışı `created_at`, boş rol, keyfî `actor_id`
için yalıtılmış saldırı yoktu ve güncelleme damgası doğumdan **önce**
alındığı için `updated_at < created_at` cümlesi hedef cümleyi maskeliyordu.
İkinci koşu **28/31** — kalan üç: geri getirme saldırısı `row_version`ı
artırmıyordu, makbuz saldırıları birleştirmeden **sonra** yazıldığı için
`merged` kaynak cümlesine takılıyordu. Saldırılar yalıtıldı (damga
güncellemeden sonra; `row_version` artırıldı; makbuz saldırıları
birleştirmeden önce) → **31/31**. İlk iki koşunun hükümleri kanıt sayılmadı,
kapı gevşetilmedi. Eşdeğer çıkan «merged değilken `merged_into` dolu
olamaz» cümlesi koddan **çıkarıldı** (durum değişmiyorsa kimlik cümlesi,
değişiyorsa geçiş cümlesi yakalar).

**CI (ölçüldü):** kod `8507a4d` (4 dosya, **+902/−1**: `0065`, `lib.rs`,
`actors.rs`, `upgrade_tests.rs`) → run `45137798693` **10/10 YEŞİL**
(`dim-storage` **770** iki OS'ta: macos-15 **589,67 s** / ubuntu-latest
**1.083,02 s**; `arayüz` 67 dosya / **1.482** test, paket
`index-Ce2oEuEs.js` **731,38 kB** / gzip 194,04 kB; `masaüstü kabuk`,
`windows güvenli depo`, SBOM, sözleşmeler, belge worker'ı, sürüm paketi
manifesti, kaynak arşivi yeşil).

**Dürüst sınırlar:** (a) damga **gelecekte** olabilir; (b) SQLite `trim()`
yalnız ASCII boşluk kırpar (U+00A0 artığı geçer); kontrol karakterlerinden
yalnız CR/LF/TAB denetlenir (gömülü NUL SQL metninde temsil edilemez, Rust
hepsini reddeder); (c) `display_name`/`role` **içeriği** (gerçek kişi mi) ve
rolün hukuki doğruluğu denetlenmez; (d) `normalized_names` eşleştirme
iddiası taşımaz; (e) birleştirme makbuzu **kendi içinde** tutarlıdır ama
kaynağın gerçekten o hedefe ait olduğu ve `reviewer_label` kimliği
doğrulanmaz; (f) v65 öncesi tutarsız satırlar silinmez/düzeltilmez (BU
dondurur, BD silmeyi durdurur) — onarım yolu **yeni taraf/rol** kaydıdır;
(g) BD CASCADE'i de durdurur: tarafı olan dosya **hard-delete** edilemez
(ürün yolu soft-delete); (h) `merged`dan çıkış, makbuz varken doğrudan SQL
ile de yazılabilir (makbuz `undone_at`ı ayrıca işaretlenmez) — çekirdek
`undo_actor_merge` her ikisini birlikte yapar; (i) `row_version` monotonluğu
eşzamanlı yazarları sıraya sokmaz; (j) tetikleyicileri anahtarı olan
saldırgan düşürebilir; (k) arayüz değişmedi; gerçek pencere + görsel QA
**insan bekliyor**.

## F08-T04an — şifreli bayt kimliği kanıtlı doğar, sonra değişmez; canlı blob silinemez (şema v66, 2026-11-23)

**Neden bu dilim:** `blobs` satırı «**şu baytlar** şu dosyada, şu boyutta, şu
anahtarla duruyor» iddiasıdır ve iki tüketici ona doğrudan yaslanır:
`gc.rs` dosya yolunu **`dir.join(format!("{}.blb", blob.storage_id))`** diye
kurar, `documents.add_version` sayacı bu satırdan artırır. Bugüne dek
`storage_id` **hiçbir yerde** biçim denetiminden geçmiyordu: `blob_disk::is_storage_id`
(32 küçük hex) yalnız **disk taramasında** çalışır, DB satırını korumaz.
Şemadaki tek kapılar `UNIQUE`, `cipher_version = 1`, `encrypted_size >= 0`,
`reference_count >= 0` CHECK'leriydi. Sonuç: `../../../../tmp/evil` gibi bir
ad doğabiliyor (GC kasanın **dışında** yol arar), uzantılı ad
(`aa11.blb` → `aa11.blb.blb`), 64 haneden uzun ad, CR/LF/TAB ve **büyük harf**
(macOS'ta varsayılan dosya sistemi büyük-küçük harf **duyarsız**: `AB` ile
`ab` aynı dosyadır) serbestti. Ayrıca `create_blob_record` doğuşta **kanıtsız
`integrity_status = 'verified'`** yazıyordu (sütunu bugün hiçbir okuyucu
okumuyor — yalan damga), `encrypted_size = 0`, boş `wrapped_key_ref` ve
doğuşta `reference_count = 5` kabul ediliyordu; satır sonradan her sütunuyla
çevrilebiliyor ve **canlı** blob makbuzsuz silinebiliyordu (`pages.derivative_blob_id`
`ON DELETE SET NULL` → sessiz NULL; `render_derivatives.blob_id` ve
`ai_cache_entries.output_blob_id` **FK'sız** → sessiz yetim).

**Ölçülen (düzeltme öncesi, gerçek SQLite v65; sonda
`scratch/dim-v66-probe/measure_v66.py`, log `measure_before.log`):**
**45 yazımın 40'ı KABUL** — **34 kusur** (24 doğum + 8 UPDATE + 2 DELETE) +
6 meşru yol; 5 RED yalnız mevcut CHECK/FK. Doğum: boş/boşluk/kırpılmamış
kimlik; boş/boşluk/kırpılmamış `storage_id`, **`../../../../tmp/evil`**,
uzantılı, 31 hane, **büyük harf**, TAB; `encrypted_size = 0`;
`wrapped_key_ref` boş/boşluk/CR-LF-TAB/kırpılmamış; doğuşta
`reference_count = 5`; doğuşta kanıtsız `verified` ve `corrupt`; damga
`dün`/boş/kırpılmamış. UPDATE: `storage_id` çevirme ve **yol kaçışı**,
`encrypted_size` 0/7, `wrapped_key_ref`, `integrity_status`, `created_at`
2030, `id`. DELETE: referanssız ve **sayaçlı** blob makbuzsuz silinebiliyor.

**Yazılan (`0066_blob_bytes_identity_guard.sql`, 1 sütun + 3 tetikleyici):**
`integrity_checked_at` sütunu eklenir — `verified`/`corrupt` hükmü ancak
**damgalı** olabilir.
- **BI** — `id` kırpılmış-dolu ≤64; `storage_id` **dosya adı belirteci**:
  1..=64 hane ve yalnız `[0-9a-z_-]` (yol ayırıcı, nokta/uzantı, boşluk,
  CR/LF/TAB ve büyük harf imkânsız); `encrypted_size ≥ 1` (AEAD'in etiketi
  her zaman vardır, sıfır bayt imkânsız); `wrapped_key_ref` kırpılmış-dolu
  ≤128, CR/LF/TAB yok; doğuşta `reference_count = 0`; doğuşta
  **`integrity_status = 'unverified'`** ve `integrity_checked_at` **NULL**;
  `created_at` ISO-8601 + gerçek gün/saat.
- **BU** — **altı sütun donar** (`id`, `storage_id`, `cipher_version`,
  `encrypted_size`, `wrapped_key_ref`, `created_at`); `reference_count` yalnız
  **±1** oynar (`add_version` +1, geri alma −1); `integrity_status` değişiyorsa
  yeni hüküm `verified`/`corrupt` olmalı ve **yeni** ISO damga taşımalı
  (`unverified`a geri dönüş yok), durum değişmeden damga yalnız
  `verified`/`corrupt` satırda tazelenebilir. No-op takılmaz, eski tutarsız
  satır kilitlenmez.
- **BD** — **canlı blob silinemez**: `document_versions`, `pages`,
  `render_derivatives` ya da `ai_cache_entries` hâlâ gösteriyorsa satır
  silinemez; referanssız blob GC tarafından silinebilir.

Ret `DIM_INTEGRITY:blobs` → `E_DB_INTEGRITY`; `blobs` `case_id` taşımaz →
`GUARDED_TABLES` **25 kalır** (kapsam `document_versions` üzerinden).
**Çekirdek hizası (davranış değişikliği):** `create_blob_record` doğuşta
`'unverified'` yazar ve aynı kuralı **kendi de doğrular**
(`blob_disk::is_safe_storage_token`, `E_DB_SETTING`) — güvenlik tek hatta
bırakılmaz.

**Düzeltmeden sonra ölçüm (57 yazım):** **47 RED · 10 KABUL · SAPMA 0**
(`measure_after.log`: doğum/UPDATE kusurları + dört canlı-silme yolunun
dördü de RED; meşru doğum, no-op, ±1, kanıtlı `verified`/`corrupt` geçişi ve
referanssız silme KABUL).

**Testler:** `blob_bytes_identity_is_evidence_backed_and_frozen` (22 doğum +
11 UPDATE kusuru + 4 canlı-silme yolu; meşru yollar ve çekirdek kapısı) ve
`upgrade_keeps_blobs_written_before_the_bytes_identity_gate` (v65 tutarsız
satırı silinmez/onarılmaz, kimliği donar, sayaç ±1 hâlâ çalışır, yeni
biçimsiz satır doğamaz, GC yolu açık) → **770 → 772**; tam paket
**772/772 yeşil, 767,60 s** (`scratch/dim-libtest-v66-final.log`). Dürüst olay: ilk tam
koşuda **5 kırmızı** — `redactions` test fikstürü `st-out-{64-hex}` (71 hane)
üretiyordu ve yeni sınır haklı olarak reddetti; **kapı gevşetilmedi**, fikstür
12 haneli benzersiz belirtece çevrildi, modül **33/33** yeşil, tam paket
yeniden koşuldu.

**Kapı:** taban YEŞİL (tam paket **772/772** + mutasyon öncesi/sonrası iki
hedefli koşu), **37/37 mutant KIRMIZI**, YEŞİL 0, ÖLÇÜLEMEDİ 0 (`scratch/dim-v66-probe/gates_v66_final.log`); kaynak byte-birebir geri yazıldı (sha256 `d92cdea36fae7192…`). **Dürüst (iki koşu):** ilk koşu **32/37** — beş mutant YEŞİL kaldı çünkü testlerde o cümleleri tek başına gerektiren saldırı yoktu (ayırıcı boşluklu damga, kanıtlı satırda damgasız hüküm, sondaki boşluklu damga, yeni damgayla `unverified`a dönüş, damga tazelemede sondaki boşluk); beş saldırı eklendi, **37/37** yeniden ölçüldü; ilk koşunun hükümleri kanıt sayılmadı, kapı gevşetilmedi.

**CI (ölçüldü):** kod `5865e16` (6 dosya, **+821/−3**: `0066`, `lib.rs`,
`documents.rs`, `blob_disk.rs`, `upgrade_tests.rs`, `redactions.rs` fikstürü) →
run `45167345756` **10/10 YEŞİL** (`dim-storage` **772** iki OS'ta: macos-15
**600,81 s** / ubuntu-latest **587,59 s**; `arayüz` 67 dosya / **1.482** test,
paket `index-Ce2oEuEs.js` **731,38 kB** / gzip 194,04 kB — değişmedi;
`masaüstü kabuk` 205 s, `windows güvenli depo` 30 test, SBOM, sözleşmeler,
belge worker'ı, sürüm paketi manifesti, kaynak arşivi yeşil).

**Dürüst sınırlar:** (a) `storage_id`in diskte gerçekten o adda dosya olduğu
doğrulanmaz; `encrypted_size`in dosya boyutu olduğu yeniden ölçülmez.
(b) `wrapped_key_ref` **hedefinin** var olduğu denetlenmez (etiket).
(c) `integrity_status` yalnız **iç tutarlılık** yönünden kapılıdır; bütünlük
taramasının gerçekten koştuğu kanıtlanmaz ve sütunun bugün okuyucusu yoktur.
(d) v66 öncesi tutarsız satırlar silinmez/düzeltilmez; kimlik sütunları
donduğu için **onarılamaz** — onarım yolu yeni içe aktarmadır.
(e) BD dört sütunu bilir; blob'a işaret eden yeni bir sütun eklenirse
`gc.rs`teki `every_blob_column_is_covered_by_the_reference_gate` testi düşer.
(f) Tetikleyicileri anahtarı olan saldırgan düşürebilir.
(g) **Kural daraltması (bilinçli):** `storage_id` «tam 32 hex» **değil**
«yol güvenli belirteç» olarak kapılandı; tam-32-hex 70+ meşru çağrı yerini
(testler/bench'ler, UUID'ler) kırardı ve dosya adı güvenliğine ek bir şey
katmazdı — ürün yolu (`random_storage_id()` → 32 hex) kuralın **alt kümesi**,
31 haneli bir belirteç de kabul edilir (yol güvenliği ölçülür, üretici biçimi
değil).
(h) **Sayaç silme kapısı değil:** `reference_count > 0` iken silmeyi
durdurmak, dosya CASCADE'i sonrası meşru olarak yetim kalan (sayaç hiç
azalmadığı için bayat) blob'ların GC ile **hiç** temizlenememesine yol açardı;
kapı «canlı referans»tır, sayaç ise doğuşta 0 + yazımda ±1 kuralıyla
dürüst tutulur.
(i) Arayüz değişmedi; gerçek pencere + görsel QA **insan bekliyor**.

## F08-T04ao — maske tanımı içerik adreslidir: kanıtlı doğar, değişmez, canlıyken silinemez (şema v67, 2026-11-23)

**Neden bu dilim:** `outbound_transforms` satırı «şu terim listesinin özeti
`settings_hash`, maskede **şu kadar** terim var, yer tutucu **şu**» iddiasıdır.
0027 tabloyu **içerik adresli** tanımladı (`settings_hash` UNIQUE) ve 0045
kendi kapanış notunda «`outbound_transforms` satırları içerik adreslidir ve
**hiçbir yol silmez**» diye yazdı; «var olmayan dönüşüm kimliği»ni *denetim
izi* değil **kanıtlanabilir yalan** sayıp fail-closed kapıya taktı. Ama bu
varsayımın **hiçbir zorlayıcısı yoktu**: tablonun 0027'den beri tek kapısı
`kind` CHECK'i ve UNIQUE indeksidir; `settings_hash` biçimi, `term_count`
işareti, `placeholder` biçimi ve `created_at` damgası denetlenmiyordu; satır
**her sütunuyla** çevrilebiliyor, **silinebiliyordu**.

Üç okuyucu bu satıra doğrudan yaslanır:
- `start_ai_run` onayın maskesini bu satırın `settings_hash`iyle karşılaştırır
  (0030/0045: «maske onayın parçasıdır»). `settings_hash` çevrilebilirse
  onaylanan maske **A** iken **B** (ya da maskesiz) gönderim kapıyı geçer.
- `list_transfer_segments`/`sent_segments` dönüşümü **sol birleştirir**; ekran
  türü (`identity`/`mask_terms`) ve terim sayısını buradan basar (Ai.tsx:
  «maskelendi · N terim» / «kimlik»). `kind` çevrilebilirse maskeli gönderim
  **maskesiz** görünür; `term_count` çevrilebilirse onayın terim sayısı yalan
  olur (0045'in eşitlik kuralı da bu sayıya bakar).
- Silinirse onayın maske bağı **çözülemez** hâle gelir (start_ai_run ham
  veritabanı hatası verir), sicil satırı «bilinmeyen tür»e düşer.

**Ölçülen (düzeltme öncesi, gerçek SQLite v66; sonda
`scratch/dim-v67-probe/measure_v67.py`, loglar `measure_before.log` /
`measure_after.log`):** **47 yazımın 44'ü KABUL** — **36 kusur** (27 doğum +
6 UPDATE + 3 DELETE) + 8 meşru yol; 3 RED yalnız mevcut kapılardan (`kind`
CHECK, iki UNIQUE çakışması). Doğum: boş/boşluk/kırpılmamış/65 hane kimlik;
boş/boşluk/kırpılmamış/63 hane/**büyük harf**/hex olmayan/LF/CR/TAB
`settings_hash`; **negatif `term_count`**; **`identity` + 3 terim** çelişkisi;
boş/boşluk/kırpılmamış/LF/CR/TAB/65 hane `placeholder`; `created_at`
`dün`/boş/kırpılmamış/**olmayan gün** (`2026-02-30`)/**olmayan saat**
(`99:00`)/TR biçimi/**boşluk ayırıcı** (`2026-01-01 00:00:00Z`). UPDATE: hash
çevirme, `kind` çevirme, terim sayısı 1→0 ve 1→9, `placeholder`, `created_at`
2030, `id`. DELETE: **onayla**, **sicille** ve **yalnız sicille** bağlı tanım
silinebiliyordu.

**Yazılan (`0067_outbound_transform_definition_guard.sql`, 3 tetikleyici):**
- **BI** — `id` kırpılmış-dolu ≤64; `settings_hash` **64 küçük harf hex**
  (içerik adresi); `term_count ≥ 0` ve **`identity` ⇒ `term_count = 0`**
  (kimlik satırı «terim maskeledim» diyemez); `placeholder` kırpılmış-dolu
  ≤64, CR/LF/TAB yok; `created_at` ISO-8601 + gerçek gün/saat (0066 deseni).
- **BU** — **altı sütun donar** (`id`, `kind`, `settings_hash`, `term_count`,
  `placeholder`, `created_at`): içerik adresli tanım yazıldıktan sonra
  değişmez. No-op takılmaz (değer aynıysa yazma geçer).
- **BD** — **canlı tanım silinemez**: `transfer_consents.outbound_transform_id`
  ya da `transfer_segments.transform_manifest_id` hâlâ gösteriyorsa satır
  silinemez (bağ başka sütunda donduğu için sessiz yetim kalamaz);
  referanssız tanım silinebilir.

Ret `DIM_INTEGRITY:outbound_transforms` → `E_DB_INTEGRITY`; tablo `case_id`
taşımaz → `GUARDED_TABLES` **25 kalır**. **Çekirdek hizası:** `ensure_outbound_transform`
özeti kendisi üretir (`sha256`, 64 küçük hex), `kind='mask_terms'`, yer tutucu
sabiti ve `now_iso()` yazar — kural ürünün yazdığı biçimi **kapsar**. Ek olarak
`start_ai_run`, onayın dönüşüm satırı **çözülemiyorsa** ham veritabanı hatası
yerine `E_AI_CONSENT_UNBOUND` döner (fail-closed, tek anlamlı kod).

**Düzeltmeden sonra ölçüm (46 yazım):** **39 RED · 8 KABUL · SAPMA 0**
(`measure_after.log`: 27 doğum + 6 UPDATE + 3 DELETE kusurunun hepsi RED;
sekiz kabulün hepsi meşru: iki no-op, referanssız silme, maskeli/boş/`identity`
tanım doğumu, onay bağı, içerik adresi sorgusu).

**Testler:** `outbound_transform_definition_is_content_addressed_and_frozen`
(28 doğum saldırısı + 6 güncelleme + 2 silme; meşru yollar; içerik adresinin
**tek satır** kaldığı; çekirdek yolun kapıdan geçtiği; tanımı silinmiş onayın
`E_AI_CONSENT_UNBOUND` verdiği) ve
`upgrade_keeps_outbound_transforms_written_before_the_definition_gate` (v66
biçimsiz satırı silinmez/düzeltilmez, sütunları donar, onaya bağlısı
silinemez, bağsızı silinebilir, yeni biçimsiz satır doğamaz, meşru tanım
doğar) → **772 → 774**; tam paket **774/774 yeşil, 822,98 s**
(`scratch/dim-libtest-v67.log`); fmt (iki manifest), `clippy -p dim-storage
--all-targets -D warnings`, masaüstü `check --all-targets` temiz.

**CI (ölçüldü):** kod `ae3eb27` → run `45191045812` **10/10 YEŞİL** — `dim-storage` **774** iki OS'ta (macos-15 **580,05 s** / ubuntu-latest **1.525,63 s**), `arayüz` 67 dosya / **1.482** test, paket `index-Ce2oEuEs.js` **731,38 kB** / gzip 194,04 kB (arayüz değişmedi). `HEAD == origin/main`.

**Kapı (`gates_v67.py`, log `gates_v67_final.log`):** taban YEŞİL, **25/25
mutant KIRMIZI**, YEŞİL 0, ÖLÇÜLEMEDİ 0; kaynak sha256 **birebir**
(`39f520339e5c99f9fb629e9856d4ad9badb950623de0cb9d6511f767f693b5ae`).

**Dürüst sınırlar:** (a) `settings_hash`in gerçekten o terim listesinin
SHA-256'sı olduğu **yeniden hesaplanmaz** (SQLite'ta SHA-256 yok) — biçim ve
değişmezlik ölçülür; (b) `placeholder`ın ürün sabitiyle (`[MASKED]`) aynı
olduğu denetlenmez, yalnız biçimi; (c) `term_count`in gerçek terim sayısı
olduğu doğrulanmaz (özetten türetilemez) — yalnız işaret + `identity`
tutarlılığı; (d) v67 öncesi tutarsız satırlar silinmez/düzeltilmez, kimlik
sütunları donduğu için **onarılamaz** — onarım yolu yeni maske + **yeni onay**;
(e) tetikleyicileri anahtarı olan saldırgan düşürebilir; (f) onay satırının
silinme yolu üründe yok — olsaydı transform BD kapısı yüzünden silinemezdi
(içerik adresli satırlar küçüktür, bilinçli); (g) arayüz değişmedi; gerçek
pencere + görsel QA **insan bekliyor**.
