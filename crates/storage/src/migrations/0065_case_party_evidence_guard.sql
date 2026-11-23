-- 0065: **dosya tarafı** — `actors` (kişi/kurum kimliği), `case_participants`
-- (dosyadaki rol) ve `actor_merges` (birleştirme makbuzu) doğuşta kanıtlı
-- olmalı ve sonrasında **değişmemeli**.
--
-- Neden: üç satır birlikte «**bu kişi** bu dosyada **şu rolde** taraf»
-- iddiasını taşır. `Parties.tsx` listeyi `display_name` + `kind` + `role` +
-- `role_note` sütunlarından **doğrudan** basar, birleştirme geçmişini
-- `actor_merges.moved_participants`ten çözer (`case_ids` ile dosya süzülür);
-- `merge_actors` kaynağın rollerini hedefe taşır, `undo_actor_merge` rolleri
-- kaynağa döndürür ve kaynağın **önceki inceleme durumunu** birebir yazar;
-- `add_case_party` birleştirilmiş (`merged`) tarafa yeni rol yazmaz.
-- 0001/0025'ten beri üç tablonun da **hiçbir tetikleyicisi yoktu**: ad
-- kırpılmadan, rol boş, `normalized_names` bozuk JSON, `review_state`
-- keyfî çevrilebilir, katılımcı **başka dosyaya** taşınabilir, silinmiş satır
-- geri getirilebilir, birleştirme makbuzu silinebilirdi. Şemadaki tek
-- kapılar `kind`/`review_state` CHECK'leri, iki FK ve
-- `UNIQUE (case_id, actor_id, role)` idi.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v64; sonda
-- `scratch/dim-v65-probe/measure_v65.py`, log `measure_before.log`):
-- **92 yazımın 92'si KABUL** — 83 kusur (29 `actors` + 33 katılımcı +
-- 21 makbuz) + 9 meşru yol. Doğum (57): kimlik boş/boşluk/kırpılmamış/
-- 65 hane; ad boş/boşluk/kırpılmamış/201 hane/CR-LF-TAB; `normalized_names`
-- bozuk JSON/nesne/sayı dizisi; doğuşta `merged`, doğuşta `merged_into`,
-- doğuşta silinmiş; `created_at`/`updated_at` `dün` ya da geri;
-- `row_version` 0; katılımcıda **silinmiş dosya**, **silinmiş aktör**,
-- boş/boşluk/kırpılmamış/81 hane rol, rolde TAB, boş/kırpılmamış/501 hane
-- rol notu, `valid_from` `dün`/takvim dışı gün, `valid_to` `dün`, **ters
-- aralık**; makbuzda **aynı kaynak/hedef**, silinmiş kaynak, `merged`
-- kaynak, `source_previous_state='merged'`, bozuk/nesne/metin `moved`
-- listesi, eksik `id`, boş/kırpılmamış/201 hane inceleyen, damga `dün`,
-- doğuşta `undone_at`. UPDATE/DELETE (26): aktörde ad/kind/
-- `normalized_names`/`created_at` çevirme, `accepted→rejected`, merged
-- olmadan `merged_into`, merged hedefi çevirme, `row_version` 0, damga
-- `dün`, **silme**; katılımcıda **başka dosyaya taşıma**, rol/rol notu/
-- `valid_from` çevirme, **keyfî `actor_id`**, silinmişi **geri getirme**,
-- silinmiş satırda içerik, `row_version` 0, damga `dün`, **silme**;
-- makbuzda hedef/taşınanlar/inceleyen/damga çevirme, geri alınmışı geri
-- alma, **silme**. Meşru yollar (9): aktör doğumu, katılımcı doğumu (boş ve
-- geçerli aralıkla), aktör/katılımcı no-op, geri dönüşümlü silme,
-- **birleştirme**, **geri alma**, hazır birleştirmenin geri alınması.
-- Düzeltmeden sonra: **83/83 kusur RED**, **9/9 meşru KABUL**
-- (`measure_after.log`; eski tutarsız satırlar v64'te yazılıp 0065
-- uygulandıktan sonra ölçüldü — güncelleme yolunun kanıtı).
--
-- Yazılan kural (3 + 3 + 3 tetikleyici):
--   * `actors` BI — `id` kırpılmış-dolu ≤64; `display_name` kırpılmış-dolu
--     ≤200 ve CR/LF/TAB taşımaz; `normalized_names` **metin dizisi** (bozuk
--     JSON sessizce boşaltılmasın); doğuşta `merged` **değil** ve
--     `merged_into_actor_id` **NULL** (birleştirme yalnız kayıtlı makbuzla
--     olur); iki damga ISO-8601 + gerçek gün/saat ve `updated_at ≥
--     created_at`; `row_version ≥ 1`; doğuşta silinmemiş.
--   * `actors` BU — kimlik sütunları **donar** (`id`, `display_name`, `kind`,
--     `normalized_names`, `created_at`, `created_by_local_actor_id`);
--     `deleted_at` yalnız NULL→damga, silinmiş satır **hiç** değişmez;
--     `merged_into` yalnız `review_state` değişimiyle birlikte (ayrı
--     «merged değilken merged_into dolu olamaz» cümlesi **eşdeğer** çıktı:
--     durum değişmiyorsa birinci cümle, değişiyorsa geçiş cümlesi yakalar —
--     yazılmadı); `review_state` yalnız **iki geçiş**: `merged`a giriş (hedef var, açık,
--     kendisi değil, hedef merged değil) ve `merged`dan çıkış (yalnız
--     kendisini kaynak gösteren bir **makbuz** varken, `merged_into` NULL'a);
--     başka durum çevrimi yok; `row_version` azalamaz, bu iki sütun
--     değişiyorsa **artmalı**; damga kuralları yine geçerli.
--   * `actors` BD — satır **silinemez**: kimlik sicili silinirse denetim
--     defterindeki `actorId` referansları sahipsiz kalır.
--   * `case_participants` BI — `id` kırpılmış-dolu ≤64; `case_id` **var ve
--     silinmemiş** dosya; `actor_id` **var, silinmemiş ve `merged` değil**
--     (çekirdeğin `add_case_party` ön koşulu); `role` kırpılmış-dolu ≤80 ve
--     CR/LF/TAB taşımaz; `role_note` NULL ya da kırpılmış-dolu ≤500;
--     `valid_from`/`valid_to` NULL ya da gerçek günle başlayan ISO damga ve
--     aralık **ters değil**; iki damga ISO + gerçek gün/saat ve
--     `updated_at ≥ created_at`; `row_version ≥ 1`; doğuşta silinmemiş.
--   * `case_participants` BU — `id`, `case_id`, `role`, `role_note`,
--     `valid_from`, `valid_to`, `created_at`, `created_by_local_actor_id`
--     **donar**; `actor_id` yalnız bir **birleştirme makbuzunun** taşıdığı
--     satır için ve yalnız makbuzun hedefi (ileri) ya da kaynağı (geri alma,
--     kaynak hâlâ `merged`) yönünde değişir — keyfî taşıma yok;
--     `deleted_at` yalnız NULL→damga, silinmiş satır **hiç** değişmez;
--     `row_version` azalamaz, `actor_id`/`deleted_at` değişiyorsa **artmalı**.
--   * `case_participants` BD — satır **silinemez**: rol kaydı silinirse
--     taraf listesi sessizce değişir; ürün yolu geri dönüşümlü silmedir
--     (`deleted_at`).
--   * `actor_merges` BI — `id` kırpılmış-dolu ≤64; kaynak/hedef **var,
--     silinmemiş, farklı** ve ikisi de `merged` **değil** (doğuş anı);
--     `source_previous_state` kapalı küme (`merged` olamaz);
--     `moved_participants` **nesne dizisi** (`id`, `case_id` metin —
--     okuyucu `unwrap_or_default` ile sessizce boşaltmasın); inceleyen
--     kırpılmış-dolu ≤200; `created_at` ISO + gerçek gün/saat; doğuşta
--     `undone_at` NULL.
--   * `actor_merges` BU — makbuz sütunları **donar** (`source_actor_id`,
--     `target_actor_id`, `source_previous_state`, `moved_participants`,
--     `reviewer_label`, `created_at`); `undone_at` yalnız NULL→damga ve
--     sonrasında **terminal**.
--   * `actor_merges` BD — makbuz **silinemez**: silinen makbuz, geri alma
--     yolunu ve `moved_participants` kanıtını yok eder.
-- Ret `DIM_INTEGRITY:actors` / `DIM_INTEGRITY:case_participants` /
-- `DIM_INTEGRITY:actor_merges` → `StorageError::Integrity` → **`E_DB_INTEGRITY`**.
--
-- Kapsam: `case_participants.case_id` bir **dosya bağıdır** ama kapsam
-- sütunu (document_id / document_version_id / parse_run_id / segment_id /
-- actor_id ...) taşımaz; üç tablo da `GUARDED_TABLES` sınıfına girmez →
-- liste **25 kalır**. `actors`/`actor_merges` `case_id` taşımaz.
--
-- Çekirdek hizası: yazma yolları zaten kırpıyor, boşu reddediyor ve
-- `now_iso()` biçimini kullanıyor; davranış değişmedi. `add_case_party`
-- birleştirilmiş tarafa rol eklemez, `merge_actors` çakışmayı önceden
-- reddeder — kapı bu iki kuralı **şemada** da tutar (savunma derinliği).
--
-- Dürüst sınırlar (ölçüm sonrası belgeye yazıldı): (a) damga **gelecekte**
-- olabilir; (b) SQLite `trim()` yalnız ASCII boşluk kırpar (U+00A0 artığı
-- geçer); kontrol karakterlerinden yalnız CR/LF/TAB denetlenir (gömülü NUL
-- SQL metninde temsil edilemez, Rust hepsini reddeder); (c) `display_name`/
-- `role` **içeriği** (gerçek kişi mi) ve `role`ün hukuki doğruluğu
-- denetlenmez; (d) `normalized_names`in eşleştirme iddiası yoktur (adın
-- küçük harfli kopyasıdır); (e) `merged_into_actor_id`nin hedefinin o
-- kaynağın **doğru** birleşme hedefi olduğu makbuzla çapraz denetlenir ama
-- kimlik doğrulanmaz (`reviewer_label` serbest metin); (f) v65 öncesi
-- tutarsız satırlar silinmez/düzeltilmez (BU dondurur; BD silmeyi de
-- durdurur) — onarım yolu **yeni taraf/rol** kaydıdır, eski satır denetim
-- izi olarak kalır; (g) BD, CASCADE ile gelen silmeleri de durdurur: tarafı
-- olan dosya **hard-delete** edilemez (ürün yolu soft-delete); (h)
-- tetikleyicileri anahtarı olan saldırgan düşürebilir.

CREATE TRIGGER actor_identity_evidence_guard_bi
BEFORE INSERT ON actors
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR NEW.display_name IS NOT trim(NEW.display_name)
  OR length(NEW.display_name) = 0
  OR length(NEW.display_name) > 200
  OR instr(NEW.display_name, char(10)) > 0
  OR instr(NEW.display_name, char(13)) > 0
  OR instr(NEW.display_name, char(9)) > 0
  OR CASE
       WHEN json_valid(NEW.normalized_names) THEN (
         json_type(NEW.normalized_names) <> 'array'
         OR EXISTS (
              SELECT 1 FROM json_each(NEW.normalized_names) AS n
               WHERE n.type <> 'text'
            )
       )
       ELSE 1
     END
  OR NEW.review_state = 'merged'
  OR NEW.merged_into_actor_id IS NOT NULL
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.created_at, 1, 10)) IS NOT substr(NEW.created_at, 1, 10)
  OR time(substr(NEW.created_at, 12, 8)) IS NOT substr(NEW.created_at, 12, 8)
  OR NEW.updated_at IS NOT trim(NEW.updated_at)
  OR NEW.updated_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.updated_at, 1, 10)) IS NOT substr(NEW.updated_at, 1, 10)
  OR time(substr(NEW.updated_at, 12, 8)) IS NOT substr(NEW.updated_at, 12, 8)
  OR NEW.updated_at < NEW.created_at
  OR NEW.row_version < 1
  OR NEW.created_by_local_actor_id IS NOT NULL
     AND (
       NEW.created_by_local_actor_id IS NOT trim(NEW.created_by_local_actor_id)
       OR length(NEW.created_by_local_actor_id) = 0
       OR length(NEW.created_by_local_actor_id) > 64
     )
  OR NEW.deleted_at IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:actors');
END;

CREATE TRIGGER actor_identity_evidence_guard_bu
BEFORE UPDATE ON actors
WHEN NEW.id IS NOT OLD.id
  OR NEW.display_name IS NOT OLD.display_name
  OR NEW.kind IS NOT OLD.kind
  OR NEW.normalized_names IS NOT OLD.normalized_names
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.created_by_local_actor_id IS NOT OLD.created_by_local_actor_id
  OR OLD.deleted_at IS NOT NULL
  OR (
       NEW.deleted_at IS NOT NULL
       AND (
         NEW.deleted_at IS NOT trim(NEW.deleted_at)
         OR NEW.deleted_at NOT LIKE '____-__-__T__:__:__%'
         OR date(substr(NEW.deleted_at, 1, 10)) IS NOT substr(NEW.deleted_at, 1, 10)
         OR time(substr(NEW.deleted_at, 12, 8)) IS NOT substr(NEW.deleted_at, 12, 8)
       )
     )
  OR NEW.updated_at IS NOT trim(NEW.updated_at)
  OR NEW.updated_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.updated_at, 1, 10)) IS NOT substr(NEW.updated_at, 1, 10)
  OR time(substr(NEW.updated_at, 12, 8)) IS NOT substr(NEW.updated_at, 12, 8)
  OR (
       OLD.created_at LIKE '____-__-__T__:__:__%'
       AND NEW.updated_at < NEW.created_at
     )
  OR NEW.row_version < 1
  OR NEW.row_version < OLD.row_version
  OR (
       (NEW.review_state IS NOT OLD.review_state
        OR NEW.merged_into_actor_id IS NOT OLD.merged_into_actor_id)
       AND NEW.row_version <= OLD.row_version
     )
  OR (
       NEW.review_state IS NOT OLD.review_state
       AND NOT (
         (
           NEW.review_state = 'merged'
           AND OLD.review_state <> 'merged'
           AND NEW.merged_into_actor_id IS NOT NULL
           AND NEW.merged_into_actor_id <> NEW.id
           AND EXISTS (
                SELECT 1 FROM actors t
                 WHERE t.id = NEW.merged_into_actor_id
                   AND t.deleted_at IS NULL
                   AND t.review_state <> 'merged'
               )
         )
         OR (
           OLD.review_state = 'merged'
           AND NEW.review_state <> 'merged'
           AND NEW.merged_into_actor_id IS NULL
           AND EXISTS (
                SELECT 1 FROM actor_merges m
                 WHERE m.source_actor_id = NEW.id
                   AND m.target_actor_id = OLD.merged_into_actor_id
               )
         )
       )
     )
  OR (
       NEW.merged_into_actor_id IS NOT OLD.merged_into_actor_id
       AND NEW.review_state IS OLD.review_state
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:actors');
END;

CREATE TRIGGER actor_identity_evidence_guard_bd
BEFORE DELETE ON actors
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:actors');
END;

CREATE TRIGGER case_participant_evidence_guard_bi
BEFORE INSERT ON case_participants
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR NOT EXISTS (
       SELECT 1 FROM cases c
        WHERE c.id = NEW.case_id
          AND c.deleted_at IS NULL
     )
  OR NOT EXISTS (
       SELECT 1 FROM actors a
        WHERE a.id = NEW.actor_id
          AND a.deleted_at IS NULL
          AND a.review_state <> 'merged'
     )
  OR NEW.role IS NOT trim(NEW.role)
  OR length(NEW.role) = 0
  OR length(NEW.role) > 80
  OR instr(NEW.role, char(10)) > 0
  OR instr(NEW.role, char(13)) > 0
  OR instr(NEW.role, char(9)) > 0
  OR (
       NEW.role_note IS NOT NULL
       AND (
         NEW.role_note IS NOT trim(NEW.role_note)
         OR length(NEW.role_note) = 0
         OR length(NEW.role_note) > 500
         OR instr(NEW.role_note, char(10)) > 0
         OR instr(NEW.role_note, char(13)) > 0
         OR instr(NEW.role_note, char(9)) > 0
       )
     )
  OR (
       NEW.valid_from IS NOT NULL
       AND (
         NEW.valid_from IS NOT trim(NEW.valid_from)
         OR length(NEW.valid_from) < 10
         OR date(substr(NEW.valid_from, 1, 10)) IS NOT substr(NEW.valid_from, 1, 10)
       )
     )
  OR (
       NEW.valid_to IS NOT NULL
       AND (
         NEW.valid_to IS NOT trim(NEW.valid_to)
         OR length(NEW.valid_to) < 10
         OR date(substr(NEW.valid_to, 1, 10)) IS NOT substr(NEW.valid_to, 1, 10)
       )
     )
  OR (
       NEW.valid_from IS NOT NULL
       AND NEW.valid_to IS NOT NULL
       AND NEW.valid_from > NEW.valid_to
     )
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.created_at, 1, 10)) IS NOT substr(NEW.created_at, 1, 10)
  OR time(substr(NEW.created_at, 12, 8)) IS NOT substr(NEW.created_at, 12, 8)
  OR NEW.updated_at IS NOT trim(NEW.updated_at)
  OR NEW.updated_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.updated_at, 1, 10)) IS NOT substr(NEW.updated_at, 1, 10)
  OR time(substr(NEW.updated_at, 12, 8)) IS NOT substr(NEW.updated_at, 12, 8)
  OR NEW.updated_at < NEW.created_at
  OR NEW.row_version < 1
  OR NEW.created_by_local_actor_id IS NOT NULL
     AND (
       NEW.created_by_local_actor_id IS NOT trim(NEW.created_by_local_actor_id)
       OR length(NEW.created_by_local_actor_id) = 0
       OR length(NEW.created_by_local_actor_id) > 64
     )
  OR NEW.deleted_at IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:case_participants');
END;

CREATE TRIGGER case_participant_evidence_guard_bu
BEFORE UPDATE ON case_participants
WHEN NEW.id IS NOT OLD.id
  OR NEW.case_id IS NOT OLD.case_id
  OR NEW.role IS NOT OLD.role
  OR NEW.role_note IS NOT OLD.role_note
  OR NEW.valid_from IS NOT OLD.valid_from
  OR NEW.valid_to IS NOT OLD.valid_to
  OR NEW.created_at IS NOT OLD.created_at
  OR NEW.created_by_local_actor_id IS NOT OLD.created_by_local_actor_id
  OR OLD.deleted_at IS NOT NULL
  OR (
       NEW.deleted_at IS NOT NULL
       AND (
         NEW.deleted_at IS NOT trim(NEW.deleted_at)
         OR NEW.deleted_at NOT LIKE '____-__-__T__:__:__%'
         OR date(substr(NEW.deleted_at, 1, 10)) IS NOT substr(NEW.deleted_at, 1, 10)
         OR time(substr(NEW.deleted_at, 12, 8)) IS NOT substr(NEW.deleted_at, 12, 8)
       )
     )
  OR (
       NEW.actor_id IS NOT OLD.actor_id
       AND NOT EXISTS (
         SELECT 1 FROM actor_merges m,
           json_each(
             CASE
               WHEN json_valid(m.moved_participants) THEN m.moved_participants
               ELSE '[]'
             END
           ) AS j
          WHERE json_extract(j.value, '$.id') = NEW.id
            AND json_extract(j.value, '$.caseId') = NEW.case_id
            AND (
              NEW.actor_id = m.target_actor_id
              OR (
                NEW.actor_id = m.source_actor_id
                AND EXISTS (
                     SELECT 1 FROM actors s
                      WHERE s.id = NEW.actor_id
                        AND s.deleted_at IS NULL
                        AND s.review_state = 'merged'
                    )
              )
            )
            AND EXISTS (
                 SELECT 1 FROM actors a
                  WHERE a.id = NEW.actor_id
                    AND a.deleted_at IS NULL
                )
       )
     )
  OR NEW.updated_at IS NOT trim(NEW.updated_at)
  OR NEW.updated_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.updated_at, 1, 10)) IS NOT substr(NEW.updated_at, 1, 10)
  OR time(substr(NEW.updated_at, 12, 8)) IS NOT substr(NEW.updated_at, 12, 8)
  OR (
       OLD.created_at LIKE '____-__-__T__:__:__%'
       AND NEW.updated_at < NEW.created_at
     )
  OR NEW.row_version < 1
  OR NEW.row_version < OLD.row_version
  OR (
       (NEW.actor_id IS NOT OLD.actor_id OR NEW.deleted_at IS NOT OLD.deleted_at)
       AND NEW.row_version <= OLD.row_version
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:case_participants');
END;

CREATE TRIGGER case_participant_evidence_guard_bd
BEFORE DELETE ON case_participants
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:case_participants');
END;

CREATE TRIGGER actor_merge_receipt_guard_bi
BEFORE INSERT ON actor_merges
WHEN length(trim(COALESCE(NEW.id, ''))) = 0
  OR NEW.id IS NOT trim(NEW.id)
  OR length(NEW.id) > 64
  OR NEW.source_actor_id IS NEW.target_actor_id
  OR NOT EXISTS (
       SELECT 1 FROM actors s
        WHERE s.id = NEW.source_actor_id
          AND s.deleted_at IS NULL
          AND s.review_state <> 'merged'
     )
  OR NOT EXISTS (
       SELECT 1 FROM actors t
        WHERE t.id = NEW.target_actor_id
          AND t.deleted_at IS NULL
          AND t.review_state <> 'merged'
     )
  OR NEW.source_previous_state NOT IN ('pending', 'accepted', 'rejected')
  OR CASE
       WHEN json_valid(NEW.moved_participants) THEN (
         json_type(NEW.moved_participants) <> 'array'
         OR EXISTS (
              SELECT 1 FROM json_each(NEW.moved_participants) AS j
               WHERE j.type <> 'object'
                 OR typeof(json_extract(j.value, '$.id')) <> 'text'
                 OR typeof(json_extract(j.value, '$.caseId')) <> 'text'
            )
       )
       ELSE 1
     END
  OR NEW.reviewer_label IS NOT trim(NEW.reviewer_label)
  OR length(NEW.reviewer_label) = 0
  OR length(NEW.reviewer_label) > 200
  OR instr(NEW.reviewer_label, char(10)) > 0
  OR instr(NEW.reviewer_label, char(13)) > 0
  OR instr(NEW.reviewer_label, char(9)) > 0
  OR NEW.created_at IS NOT trim(NEW.created_at)
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR date(substr(NEW.created_at, 1, 10)) IS NOT substr(NEW.created_at, 1, 10)
  OR time(substr(NEW.created_at, 12, 8)) IS NOT substr(NEW.created_at, 12, 8)
  OR NEW.undone_at IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:actor_merges');
END;

CREATE TRIGGER actor_merge_receipt_guard_bu
BEFORE UPDATE ON actor_merges
WHEN NEW.id IS NOT OLD.id
  OR NEW.source_actor_id IS NOT OLD.source_actor_id
  OR NEW.target_actor_id IS NOT OLD.target_actor_id
  OR NEW.source_previous_state IS NOT OLD.source_previous_state
  OR NEW.moved_participants IS NOT OLD.moved_participants
  OR NEW.reviewer_label IS NOT OLD.reviewer_label
  OR NEW.created_at IS NOT OLD.created_at
  OR OLD.undone_at IS NOT NULL
  OR (
       NEW.undone_at IS NOT NULL
       AND NEW.undone_at IS NOT trim(NEW.undone_at)
     )
  OR (
       NEW.undone_at IS NOT NULL
       AND NEW.undone_at NOT LIKE '____-__-__T__:__:__%'
     )
  OR (
       NEW.undone_at IS NOT NULL
       AND date(substr(NEW.undone_at, 1, 10)) IS NOT substr(NEW.undone_at, 1, 10)
     )
  OR (
       NEW.undone_at IS NOT NULL
       AND time(substr(NEW.undone_at, 12, 8)) IS NOT substr(NEW.undone_at, 12, 8)
     )
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:actor_merges');
END;

CREATE TRIGGER actor_merge_receipt_guard_bd
BEFORE DELETE ON actor_merges
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:actor_merges');
END;
