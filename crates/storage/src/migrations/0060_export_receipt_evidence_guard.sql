-- 0060: **dışa aktarma makbuzu** (`exports` + `export_items`) doğuşta kanıtlı
-- olmalı ve sonuçlandıktan sonra değişmemeli.
--
-- Neden: `exports` satırı «bu dosyadan **ne** çıktı, **nereye**, hangi
-- **özetle**, **ne zaman**, **başarılı mı**» iddiasıdır; kalem satırları da
-- «hangi **belge sürümlerinden** üretildi, o sürüm **maskeli** miydi»
-- iddiasını taşır (şartname §16.3, §433). Üç okuma yolu bu satıra yaslanır:
--   * `Exports.tsx` denetim defteri — durum rozeti (`ready` → «yazıldı»,
--     `failed` → «başarısız»), hedef yol, özet öneki ve zaman **doğrudan**
--     bu sütunlardan basılır; tanınmayan durum ham değeriyle yazılır;
--   * `list_export_items` — «kaynakları göster» hangi sürümlerin çıktığını ve
--     maskeli olup olmadığını bu tablodan okur;
--   * `record_export`/`fail_export` — kayıt **önce** yazılır (denetlenemeyen
--     aktarım yapılmaz), düşen adım kaydı `failed` işaretler.
-- Şemada bu iki tablonun kapısı yalnız **kapsam**tı (`export_items`, 0037) ve
-- `status` için bir `CHECK` vardı; `exports` üzerinde 0001'den beri **hiçbir**
-- tetikleyici yoktu. Komşuları kapılıydı (v55 aktarım makbuzu, v58 yedek
-- makbuzu) ama dışa aktarma makbuzu değildi.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v59; sonda
-- `scratch/dim-v60-probe/measure_v60.py`, log `measure_before.log`):
-- 75 kusur yazımının **73'ü KABUL** (2'si zaten RED — yalnız FK), 19 meşru
-- yazma KABUL, 2 kapı dışı (bilinçli sınır) KABUL. Doğum (41): boş/boşluk kimlik; boş, kırpılmamış, satır sonlu
-- tür; doğuşta `building`/`draft`/`blocked` (hiçbir yol tamamlamaz → sonsuza
-- dek «sürüyor» görünür); boş/kırpılmamış/satır sonlu hedef türü; `ready` +
-- yol NULL/boş/boşluk/kırpılmamış/satır sonlu; `ready` + özet NULL, `hash`,
-- `'Z'*64`, BÜYÜK harf hex, 63 hane; boş/`'dün'` `created_at`; `ready` +
-- `completed_at` NULL / `'dün'` / `created_at`ten **önce**; `included_ids`
-- JSON değil / nesne / sayı dizisi / boş kimlik / **mükerrer** / **başka
-- dosyanın sürümü**; şablon yalnız boşluk / 65 karakter / kırpılmamış;
-- onay kimliği kırpılmamış. UPDATE (17): özet, hedef yol, `created_at`,
-- `completed_at`, tür, `included_ids`, şablon, hedef türü çevrildi;
-- `ready`→`draft`/`building` (geri alma); `failed`→`ready` (**sahte
-- başarı**); `failed`→`blocked`; onay sonradan eklendi; satır **başka
-- dosyaya** taşındı; makbuz ve kalem satırı **silindi** (defter «silinemez»
-- diyordu ama SQL siliyordu). Kalem (17): boş kimlik; boş/kırpılmamış kaynak
-- sürüm; bilinmeyen/boş maskeleme durumu; sayfa aralığı 33 karakter / satır
-- sonlu; atıf etiketi 65 karakter; **sonuçlanmış (failed) makbuza kalem
-- ekleme**; aynı sürüm **iki kez**; kaynak sürüm/maskeleme/sayfa aralığı
-- UPDATE ile çevrildi; kalem başka makbuza taşındı; kimlik takası. (Makbuzun
-- `included_ids`inde bildirilmeyen **aynı dosya** sürümü kalem olarak
-- yazılabiliyor — bilinçli dışarıda, sınır (c): 0037 kapsam kapısının
-- sütunuyla aynı ifadeyi ikinci tetikleyiciye koymak hata kodunu
-- belirsizleştirirdi.)
--
-- Yazılan kural (5 tetikleyici):
--   * `exports` BI — `id` kırpılmış-dolu; `type` kırpılmış, dolu, ≤64 ve
--     CR/LF/TAB taşımaz; doğuşta durum **yalnız `ready`** (kayıt dosya
--     yazıldıktan **sonra** doğar; `failed` yalnız sonradan düşer — `draft`/
--     `building`/`verifying`/`blocked` doğuşu hiçbir yol tamamlamaz, «sürüyor»
--     yalanı olurdu); `destination_kind` kırpılmış-dolu ≤64; `destination_path`
--     **dolu** (boşluk değil), kırpılmış, ≤4096, CR/LF/TAB yok; `output_hash`
--     **64 küçük harf hex**; `created_at`/`completed_at` ISO-8601 ve
--     `completed_at ≥ created_at`; `included_ids` **metin dizisi** (json_valid
--     + array + her eleman kırpılmış metin), **mükerrer yok**, her kimlik
--     çözülebiliyorsa **aynı dosyanın** sürümü; `template_version` ve
--     `externalization_consent_id` NULL ya da kırpılmış-dolu ≤64.
--   * `exports` BU — on iki kanıt sütunu **donar**; durum yalnız
--     `ready`→`failed` ilerler; `failed` **terminal**; değişmeyen yazma takılmaz.
--   * `exports` BD — makbuz satırı **silinemez** (denetim defteri iddiası;
--     `case` silme CASCADE'i bilinçli dışarıda — aşağıdaki dürüst sınır (f)).
--     **Kapsam dışına daraltma:** bu iki kalem tetikleyicisi `NOT EXISTS`
--     (çapraz sürüm) kapısıyla yalnız **kendi dosyasının** satırlarında
--     çalışır: çapraz satırın hükmü 0037'nindir, iki kapı aynı satırda
--     yarışmaz (v46 dersi; `scope_guard.rs` ve v37 yükseltme testi
--     `E_DB_SCOPE` bekler).
--   * `export_items` BI — `id`/`export_id` kırpılmış-dolu; `source_version_id`
--     kırpılmış-dolu ≤128, CR/LF/TAB yok; `redaction_state` **kapalı küme**
--     (`none|masked`); sayfa aralığı ≤32, atıf etiketi ≤64 (kırpılmış, boşluk
--     değil, kontrol karakteri yok); üst makbuz **var ve `ready`**; aynı sürüm
--     aynı makbuza **iki kez** yazılamaz.
--   * `export_items` BU + BD — `id`, sayaç/etiket ve maskeleme durumu
--     **donar**; `export_id`/`source_version_id` aynı sütunlarda ikinci
--     tetikleyici açmamak için **0037'nin kapsam kapısında** donar
--     (`DROP`+`CREATE`, önek `DIM_SCOPE` korunur — v46 dersi); kalem satırı
--     **silinemez**.
-- Ret `DIM_INTEGRITY:exports` / `DIM_INTEGRITY:export_items` →
-- `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
--
-- Kapsam taraması: `exports` `case_id` taşıyor ve **zaten** taranıyor;
-- `export_items` kapsamı 0037'de üst kayıttan okunuyor → yeni sınıf eklenmedi,
-- `GUARDED_TABLES` **25 kalır**. Kural dosya kapsamı değil **makbuz kanıtı**
-- bütünlüğüdür.
--
-- Çekirdek hizası (davranış sıkılaşması): `record_export` artık tür, hedef
-- türü, hedef yol ve özeti **kendisi** doğrular (boş hedef yol artık `NULL`'a
-- çevrilmez, `E_EXPORT_RECEIPT` ile reddedilir: dosya yazıldıysa nereye
-- yazıldığı bilinmelidir; `NULL` yalnız 24. şema öncesi eski kayıtlarda
-- kalır). Ürün yolu değişmedi — beş çağrı da geçerli değerler geçiriyor.
--
-- Dürüst sınırlar: (a) `output_hash`in gerçekten yazılan dosyanın SHA-256'sı
-- olduğu **yeniden hesaplanmaz** (SQLite'ta dosya sistemi/SHA-256 yok);
-- ölçülen **biçim + değişmezlik**tir. (b) `destination_path`in diskte
-- **var olduğu** ve dosyanın gerçekten oraya yazıldığı doğrulanmaz — yol bir
-- **etiket**tir; yazma kanıtı `write_atomic_verified`in geri okumasıdır
-- (c) `included_ids` ile kalem satırları **çapraz denetlenmez**: makbuzun
-- bildirmediği bir sürüm kalem olarak yazılabilir ve bildirdiği her sürüm için
-- kalem doğmak zorunda değil (kalemsiz eski kayıtlar meşru). Ürün yolu ikisini
-- **birlikte** üretir; ayrıştırmayı şema yakalamaz. (d) `included_ids`teki bir
-- kimlik **hiç çözülemiyorsa** (sürüm silinmiş) kapsam denetimi atlanır —
-- silme meşru, kimlik uydurma değil. (e) `externalization_consent_id`nin var
-- olan bir onaya işaret ettiği doğrulanmaz (bugün hiçbir yol yazmıyor; sütun
-- donar). (f) `cases` silme `ON DELETE CASCADE`i **ölçüldü**: CASCADE
-- eylemleri bu bağlantıda tetikleyicileri **çalıştırır** — makbuzu olan dosya
-- hard-delete edilemez (`DIM_INTEGRITY:exports`). Ürün yolu yalnız soft-delete
-- (`deleted_at`) kullanır; hard delete gerekirse makbuzlar ürün kararıyla
-- ele alınmalıdır. (g)
-- Tetikleyicileri anahtarı olan saldırgan düşürebilir. (h) v60 öncesi
-- tutarsız makbuzlar **silinmez/düzeltilmez** (donar); onarım yolu yeni
-- dışa aktarmadır. (i) Arayüz davranışı değişmedi; gerçek pencere + görsel QA
-- **insan bekliyor**.

CREATE TRIGGER export_receipt_evidence_guard_bi
BEFORE INSERT ON exports
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR NEW.type IS NULL
  OR NEW.type IS NOT trim(NEW.type)
  OR length(NEW.type) = 0
  OR length(NEW.type) > 64
  OR instr(NEW.type, char(9)) > 0
  OR instr(NEW.type, char(10)) > 0
  OR instr(NEW.type, char(13)) > 0
  OR COALESCE(NEW.status, '') <> 'ready'
  OR NEW.destination_kind IS NULL
  OR NEW.destination_kind IS NOT trim(NEW.destination_kind)
  OR length(NEW.destination_kind) = 0
  OR length(NEW.destination_kind) > 64
  OR instr(NEW.destination_kind, char(9)) > 0
  OR instr(NEW.destination_kind, char(10)) > 0
  OR instr(NEW.destination_kind, char(13)) > 0
  OR NEW.destination_path IS NULL
  OR NEW.destination_path IS NOT trim(NEW.destination_path)
  OR length(NEW.destination_path) = 0
  OR length(NEW.destination_path) > 4096
  OR instr(NEW.destination_path, char(9)) > 0
  OR instr(NEW.destination_path, char(10)) > 0
  OR instr(NEW.destination_path, char(13)) > 0
  OR NEW.output_hash IS NULL
  OR length(NEW.output_hash) <> 64
  OR NEW.output_hash GLOB '*[^0-9a-f]*'
  OR NEW.created_at IS NULL
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR NEW.completed_at IS NULL
  OR NEW.completed_at NOT LIKE '____-__-__T__:__:__%'
  OR NEW.completed_at < NEW.created_at
  OR (CASE WHEN json_valid(NEW.included_ids)
           THEN json_type(NEW.included_ids) ELSE 'x' END) <> 'array'
  OR EXISTS (
       SELECT 1 FROM json_each(NEW.included_ids)
        WHERE length(trim(value)) = 0
           OR value IS NOT trim(value)
     )
  OR (SELECT count(*) FROM json_each(NEW.included_ids))
     <> (SELECT count(DISTINCT value) FROM json_each(NEW.included_ids))
  OR EXISTS (
       SELECT 1
         FROM json_each(NEW.included_ids) j
         JOIN document_versions v ON v.id = j.value
         JOIN documents d ON d.id = v.document_id
        WHERE d.case_id <> NEW.case_id
     )
  OR (NEW.template_version IS NOT NULL
      AND (NEW.template_version IS NOT trim(NEW.template_version)
           OR length(NEW.template_version) = 0
           OR length(NEW.template_version) > 64
           OR instr(NEW.template_version, char(9)) > 0
           OR instr(NEW.template_version, char(10)) > 0
           OR instr(NEW.template_version, char(13)) > 0))
  OR (NEW.externalization_consent_id IS NOT NULL
      AND (NEW.externalization_consent_id IS NOT trim(NEW.externalization_consent_id)
           OR length(NEW.externalization_consent_id) = 0
           OR length(NEW.externalization_consent_id) > 64
           OR instr(NEW.externalization_consent_id, char(9)) > 0
           OR instr(NEW.externalization_consent_id, char(10)) > 0
           OR instr(NEW.externalization_consent_id, char(13)) > 0))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:exports');
END;

CREATE TRIGGER export_receipt_evidence_guard_bu
BEFORE UPDATE ON exports
WHEN NEW.id IS NOT OLD.id
  OR NEW.case_id IS NOT OLD.case_id
  OR NEW.type IS NOT OLD.type
  OR NEW.included_ids IS NOT OLD.included_ids
  OR NEW.template_version IS NOT OLD.template_version
  OR NEW.destination_kind IS NOT OLD.destination_kind
  OR NEW.destination_path IS NOT OLD.destination_path
  OR NEW.output_hash IS NOT OLD.output_hash
  OR NEW.externalization_consent_id IS NOT OLD.externalization_consent_id
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.completed_at IS NOT OLD.completed_at
  OR (NEW.status IS NOT OLD.status
      AND NOT (OLD.status = 'ready' AND NEW.status = 'failed'))
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:exports');
END;

CREATE TRIGGER export_receipt_evidence_guard_bd
BEFORE DELETE ON exports
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:exports');
END;

-- 0037'nin kapsam kapısı **genişletilir** (DROP+CREATE, önek korunur):
-- `export_id` ve `source_version_id` aynı sütunlarda ikinci bir tetikleyici
-- açmak yerine burada **donar** — kapsam kapısının sütunları tek yerde kalır
-- (v46 dersi: aynı sütunda iki BEFORE tetikleyici hata kodunu belirsizleştirir;
-- `scope_guard.rs` testleri `E_DB_SCOPE` bekler).
DROP TRIGGER scope_guard_export_items_bu;

CREATE TRIGGER scope_guard_export_items_bu
BEFORE UPDATE ON export_items
WHEN NEW.export_id IS NOT OLD.export_id
  OR NEW.source_version_id IS NOT OLD.source_version_id
  OR EXISTS (
       SELECT 1
         FROM exports e
         JOIN document_versions v ON v.id = NEW.source_version_id
         JOIN documents d ON d.id = v.document_id
        WHERE e.id = NEW.export_id
          AND e.case_id <> d.case_id
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_SCOPE:export_items');
END;

CREATE TRIGGER export_item_evidence_guard_bi
BEFORE INSERT ON export_items
WHEN NOT EXISTS (
       SELECT 1
         FROM exports e
         JOIN document_versions v ON v.id = NEW.source_version_id
         JOIN documents d ON d.id = v.document_id
        WHERE e.id = NEW.export_id
          AND e.case_id <> d.case_id
     )
  AND (length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR NEW.source_version_id IS NULL
  OR NEW.source_version_id IS NOT trim(NEW.source_version_id)
  OR length(NEW.source_version_id) = 0
  OR length(NEW.source_version_id) > 128
  OR instr(NEW.source_version_id, char(9)) > 0
  OR instr(NEW.source_version_id, char(10)) > 0
  OR instr(NEW.source_version_id, char(13)) > 0
  OR COALESCE(NEW.redaction_state, '') NOT IN ('none', 'masked')
  OR (NEW.output_page_range IS NOT NULL
      AND (NEW.output_page_range IS NOT trim(NEW.output_page_range)
           OR length(NEW.output_page_range) = 0
           OR length(NEW.output_page_range) > 32
           OR instr(NEW.output_page_range, char(9)) > 0
           OR instr(NEW.output_page_range, char(10)) > 0
           OR instr(NEW.output_page_range, char(13)) > 0))
  OR (NEW.citation_label IS NOT NULL
      AND (NEW.citation_label IS NOT trim(NEW.citation_label)
           OR length(NEW.citation_label) = 0
           OR length(NEW.citation_label) > 64
           OR instr(NEW.citation_label, char(9)) > 0
           OR instr(NEW.citation_label, char(10)) > 0
           OR instr(NEW.citation_label, char(13)) > 0))
  OR NOT EXISTS (
       SELECT 1 FROM exports e
        WHERE e.id = NEW.export_id AND e.status = 'ready'
     )
  OR EXISTS (
       SELECT 1 FROM export_items x
        WHERE x.export_id = NEW.export_id
          AND x.source_version_id = NEW.source_version_id
     )
      )

BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:export_items');
END;

CREATE TRIGGER export_item_evidence_guard_bu
BEFORE UPDATE ON export_items
WHEN NOT EXISTS (
       SELECT 1
         FROM exports e
         JOIN document_versions v ON v.id = NEW.source_version_id
         JOIN documents d ON d.id = v.document_id
        WHERE e.id = NEW.export_id
          AND e.case_id <> d.case_id
     )
  AND (NEW.id IS NOT OLD.id
  OR NEW.output_page_range IS NOT OLD.output_page_range
  OR NEW.citation_label IS NOT OLD.citation_label
  OR NEW.redaction_state IS NOT OLD.redaction_state
      )

BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:export_items');
END;

CREATE TRIGGER export_item_evidence_guard_bd
BEFORE DELETE ON export_items
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:export_items');
END;
