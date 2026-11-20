-- 0062: **inceleme hükmü** — `review_decisions` (öğe kararı) ve
-- `contradiction_assessments` (çelişki değerlendirmesi) doğuşta kanıtlı
-- olmalı ve sonrasında **değişmemeli**.
--
-- Neden: iki satır da bir hükmün sicilidir. `review_decisions` satırı «bu öğe
-- için **şu karar** verildi, gerekçesi **şu**, veren **şu**, zaman **şu**»
-- iddiasıdır; `Review.tsx` karar geçmişi paneli kararı, gerekçeyi, inceleyeni,
-- zamanı ve düzenlenen yükü **doğrudan** bu sütunlardan basar
-- (`edited_payload` çözülemezse «okunamadı» uyarısı çıkar — sessiz yutma yok).
-- `contradiction_assessments` satırı «iki kaynak **şu yönde** değerlendirildi»
-- iddiasıdır; ekran «son değerlendirme» olarak **en yenisini** basar
-- (`list_contradiction_assessments` `assessed_at` anına göre sıralar) ve
-- `commit_review_decision` hedefi çözerken karar sayımını bu sicilden okur.
-- 0007 göçünün kendi yorumu kayıtların **eklemeli** olduğunu söyler: aynı öğe
-- için birden çok değerlendirme tutulur, düzeltme yolu yeni satırdır.
--
-- Komşuları kapılıydı (`review_items` 0033/0034/0040/0041 kapsam, denetim
-- zinciri ayrı) ama **hükmün kendisi** 0001/0007'den beri tetikleyicisizdi.
-- Şemadaki tek kapı `decision` / `verdict` CHECK'leriydi. Kararın hangi öğeye
-- ait olduğu, öğenin **açık** olup olmadığı (kapanmış öğeye sonradan «kabul»
-- yazılabiliyordu), inceleyenin kimliği, gerekçenin kırpılmışlığı, damganın
-- biçimi, düzenlenen yükün JSON'luğu ve satırın sonradan **her sütunuyla**
-- değiştirilebilmesi (karar `accept`→`reject` çevrilebiliyordu) şemada yoktu.
-- 0007'nin «eklemeli» sözü de şemada zorlanmıyordu: satır silinebiliyor ve
-- UPDATE ile yeniden yazılabiliyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v61; sonda
-- `scratch/dim-v62-probe/measure_v62.py`, log `measure_before.log`):
-- **61 kusur yazımının 57'si KABUL** (4'ü zaten RED — iki CHECK, iki FK),
-- 12 meşru yazma KABUL. Doğum (37 KABUL): boş / boşluk / kırpılmamış /
-- 65 haneli kimlik; boş / boşluk / kırpılmamış inceleyen; kırpılmamış gerekçe;
-- damga boş / `dün` / kırpılmamış / takvim dışı gün (`2026-02-30`) / olmayan
-- saat (`99:00:00`) / boşluk ayırıcı (`2026-11-20 00:00:00Z`); düzenlenen yük
-- **bozuk JSON**; `edit` kararı **yüksüz**
-- (içeriksiz düzenleme: ekran «düzenlendi» der, neyin düzenlendiği yok);
-- **kapanmış öğeye** karar, **yerine geçmiş** öğeye karar, **hedefi yazılmış**
-- öğeye ikinci karar, **silinmiş dosyanın** öğesine karar; değerlendirmede
-- **başka dosya** kimliği, silinmiş dosya, **çelişki adayı olmayan** öğe
-- (ekran o paneli hiç çizmez — sessiz yetim satır), boş/boşluk/kırpılmamış
-- inceleyen ve gerekçe, boş/`dün`/kırpılmamış / takvim dışı gün / olmayan saat
-- / boşluk ayırıcılı damga. UPDATE (15 KABUL):
-- kararın `edit`→`reject` çevrilmesi, inceleyenin/gerekçenin/damganın
-- değiştirilmesi, düzenlenen yükün değiştirilmesi/silinmesi, kimliğin
-- yeniden adlandırılması, satırın **başka öğeye** taşınması; değerlendirmede
-- hükmün çevrilmesi (`conflict`→`explained`), gerekçe/inceleyen/damga,
-- satırın **başka dosyaya** ve **başka öğeye** taşınması, kimlik. DELETE
-- (5 KABUL): karar satırı, değerlendirme satırı, kararı olan öğe (CASCADE),
-- değerlendirmesi olan öğe (CASCADE), kararı olan dosya (CASCADE).
--
-- Yazılan kural (3 + 3 tetikleyici):
--   * `review_decisions` BI — `id` kırpılmış-dolu ≤64; `review_item_id`
--     **var olan**, **açık** (`state IN ('pending','needs_more_evidence')`),
--     yerine geçmemiş (`superseded_at IS NULL`), hedefi yazılmamış
--     (`target_entity_id IS NULL`) ve **dosyası silinmemiş** öğeyi gösterir
--     (çekirdeğin iki karar yolunun da ön koşulu budur; `EXISTS` tek cümlede
--     toplandı — ayrı `IS NOT NULL` cümlesi `EXISTS`le kapsanıp etkisiz
--     kalırdı); `reviewer_label` kırpılmış-dolu; `rationale` kırpılmış
--     (boş olabilir); `decided_at` kırpılmış ISO-8601 damgası ve **gerçek
--     gün + gerçek saat** (`date(substr(...,1,10))` ve
--     `time(substr(...,12,8))` eşitliği: `2026-02-30`, `99:00:00` düşer);
--     `edited_payload` NULL ya da **geçerli JSON**; `decision='edit'` ise
--     `edited_payload` **dolu** (bir düzenlemenin içeriği olmalı).
--   * `review_decisions` BU — **yedi sütunun hepsi donar**: karar sicili
--     eklemelidir, düzeltme yolu yeni satırdır. Değişmeyen yazma takılmaz.
--   * `review_decisions` BD — satır **silinemez**: silinen karar, öğe
--     durumunu (`accepted`/`rejected`) gerekçesiz bırakır; denetim zinciri
--     ayrı tutulur ama ekranın gösterdiği sicil bu tablodur.
--   * `contradiction_assessments` BI — `id` kırpılmış-dolu ≤64; satır
--     **aynı dosyanın**, **çelişki adayı** (`item_kind =
--     'contradiction_candidate'`) ve **dosyası silinmemiş** öğesini
--     gösterir (satırdaki `case_id` öğenin dosyasıyla **aynı** olmalı —
--     INV-05; ayrı `case_id` var-lık cümlesi yazılmadı, `EXISTS` kapsıyor);
--     `reviewer_label` kırpılmış-dolu; `rationale` kırpılmış; `assessed_at`
--     kırpılmış ISO-8601 damgası ve gerçek gün + gerçek saat.
--     `verdict` kapalı kümesi `CHECK` ile eşdeğer olduğu için ayrıca
--     yazılmadı (v61 notu).
--   * `contradiction_assessments` BU — **yedi sütunun hepsi donar**.
--   * `contradiction_assessments` BD — satır **silinemez**.
-- Ret `DIM_INTEGRITY:review_decisions` / `DIM_INTEGRITY:contradiction_assessments`
-- → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
--
-- Kapsam: iki tablo da `case_id` **referansı** (document_id /
-- document_version_id / parse_run_id / segment_id) taşımaz;
-- `contradiction_assessments.case_id` burada **öğenin dosyasına eşitlik**
-- olarak donar. Tarama sınıfı eklenmedi → `GUARDED_TABLES` **25 kalır**.
--
-- Çekirdek hizası (küçük sıkılaşma): `decide_review_item` artık `edit`
-- kararını **düzenlenen yük olmadan** kabul etmez (`InvalidSetting` →
-- `E_DB_SETTING`). Arayüz düzenleme yolunda yükü her zaman gönderir;
-- `commit_review_decision` düzenleme yükünü kendisi üretir. Diğer yazma
-- yolları zaten kırpıyor ve `now_iso()` damgası kurallarla aynı sözleşmede.
--
-- Dürüst sınırlar (ölçüm sonrası belgeye yazıldı): (a) damga **gelecekte**
-- olabilir; (b) `edited_payload`ın **içeriği** denetlenmez (yalnız JSON
-- geçerliliği); (c) `reviewer_label` serbest metindir — kimlik doğrulanmaz;
-- (d) kararın **doğruluğu** değil, kaydın ön koşulları ve değişmezliği
-- ölçülür; (e) v62 öncesi tutarsız satırlar silinmez/düzeltilmez (BU
-- dondurur) — onarım yolu yeni karar satırıdır, ancak öğe kapalıysa yeni
-- satır doğamaz; (f) BD, CASCADE ile gelen silmeleri de durdurur: kararı
-- olan öğe/dosya **hard-delete** edilemez (ürün yolu soft-delete); (g)
-- tetikleyicileri anahtarı olan saldırgan düşürebilir.

CREATE TRIGGER review_decision_evidence_guard_bi
BEFORE INSERT ON review_decisions
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR NOT EXISTS (
       SELECT 1 FROM review_items i
         JOIN cases c ON c.id = i.case_id
        WHERE i.id = NEW.review_item_id
          AND i.state IN ('pending', 'needs_more_evidence')
          AND i.superseded_at IS NULL
          AND i.target_entity_id IS NULL
          AND c.deleted_at IS NULL
     )
  OR NEW.reviewer_label IS NOT trim(NEW.reviewer_label)
  OR length(NEW.reviewer_label) = 0
  OR NEW.rationale IS NOT trim(NEW.rationale)
  OR NEW.decided_at IS NOT trim(NEW.decided_at)
  OR NEW.decided_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.decided_at, 1, 10)) IS NOT substr(NEW.decided_at, 1, 10)
  OR time(substr(NEW.decided_at, 12, 8)) IS NOT substr(NEW.decided_at, 12, 8)
  OR (NEW.edited_payload IS NOT NULL AND NOT json_valid(NEW.edited_payload))
  OR (NEW.decision = 'edit' AND NEW.edited_payload IS NULL)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:review_decisions');
END;

CREATE TRIGGER review_decision_evidence_guard_bu
BEFORE UPDATE ON review_decisions
WHEN NEW.id IS NOT OLD.id
  OR NEW.review_item_id IS NOT OLD.review_item_id
  OR NEW.decision IS NOT OLD.decision
  OR NEW.edited_payload IS NOT OLD.edited_payload
  OR NEW.reviewer_label IS NOT OLD.reviewer_label
  OR NEW.rationale IS NOT OLD.rationale
  OR NEW.decided_at IS NOT OLD.decided_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:review_decisions');
END;

CREATE TRIGGER review_decision_evidence_guard_bd
BEFORE DELETE ON review_decisions
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:review_decisions');
END;

CREATE TRIGGER contradiction_assessment_evidence_guard_bi
BEFORE INSERT ON contradiction_assessments
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR NOT EXISTS (
       SELECT 1 FROM review_items i
         JOIN cases c ON c.id = i.case_id
        WHERE i.id = NEW.review_item_id
          AND i.case_id = NEW.case_id
          AND i.item_kind = 'contradiction_candidate'
          AND c.deleted_at IS NULL
     )
  OR NEW.reviewer_label IS NOT trim(NEW.reviewer_label)
  OR length(NEW.reviewer_label) = 0
  OR NEW.rationale IS NOT trim(NEW.rationale)
  OR NEW.assessed_at IS NOT trim(NEW.assessed_at)
  OR NEW.assessed_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.assessed_at, 1, 10)) IS NOT substr(NEW.assessed_at, 1, 10)
  OR time(substr(NEW.assessed_at, 12, 8)) IS NOT substr(NEW.assessed_at, 12, 8)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:contradiction_assessments');
END;

CREATE TRIGGER contradiction_assessment_evidence_guard_bu
BEFORE UPDATE ON contradiction_assessments
WHEN NEW.id IS NOT OLD.id
  OR NEW.case_id IS NOT OLD.case_id
  OR NEW.review_item_id IS NOT OLD.review_item_id
  OR NEW.verdict IS NOT OLD.verdict
  OR NEW.rationale IS NOT OLD.rationale
  OR NEW.reviewer_label IS NOT OLD.reviewer_label
  OR NEW.assessed_at IS NOT OLD.assessed_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:contradiction_assessments');
END;

CREATE TRIGGER contradiction_assessment_evidence_guard_bd
BEFORE DELETE ON contradiction_assessments
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:contradiction_assessments');
END;
