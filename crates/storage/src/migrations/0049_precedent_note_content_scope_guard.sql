-- 0049: **emsal notunun kapsamı, konusu ve metni kanıtlanabilir olmalı**.
--
-- Neden: `precedent_notes` hukukçunun **iş ürünüdür** ve arayüz her notun
-- yanında kapsam rozetini basar — `caseIdOptional` doluysa «dosyaya özel»,
-- boşsa «kütüphane» notu (`screens/Precedents.tsx`; sözleşme
-- `create_precedent_note` yorumunda: «verilirse o dosyaya özeldir; verilmezse
-- kütüphane notudur»). Rozetin yanında `createdAt` gösterilir ve üç metin alanı
-- (benzerlik / ayırt edici / sonuç) okunur. Çekirdek yalnız INSERT eder
-- (`create_precedent_note`): not doğduktan sonra düzeltme/silme yolu **yoktur**.
-- Şemada ise tablo **hiçbir tetikleyici taşımıyordu** (0001'den beri): kapsam,
-- konu, metin ve zaman doğrudan SQL ile serbestçe çevrilebiliyordu.
--
-- Ölçüldü (düzeltme öncesi, gerçek SQLite v48; sonda
-- `scratch/dim-v49-probe/measure_v49.py`, log `measure_before.log`): 18 kusur
-- yazımının **15'i KABUL** (3 RED yalnız FK/NOT NULL'ın kendisiydi):
--   (1) not **silinmiş** dosyaya bağlandı (`deleted_at` dolu; çekirdek
--       reddeder, şema kabul etti) → rozet «dosyaya özel» der, dosya yoktur;
--   (2) `id=''` ile not yazıldı;
--   (3) `created_at=''` ile not yazıldı — ekranda tarih boş basılır;
--   (4) kırpılmamış metin (`'  boşluklu  '`) yazıldı (çekirdek `trim` eder);
--   (5) yalnız boşluktan oluşan `useful_passage_anchor` yazıldı;
--   (6) **dosyaya özel not `case_id_optional=NULL` yapılarak kütüphaneye
--       çevrildi** — gizlilik iddiası sessizce genişledi;
--   (7) not **başka dosyaya** taşındı (`case_id_optional='c2'`);
--   (8) not **başka emsale** taşındı (`precedent_id` UPDATE) — metin artık
--       başka kararın altında okunur;
--   (9) benzerlik metni sonradan değiştirildi;
--   (10) sonuç metni sonradan değiştirildi;
--   (11) ayırt edici metin sonradan değiştirildi (kütüphane notunda);
--   (12) `created_at` geriye alındı (not sonradan «eski» gösterildi);
--   (13) sonradan çapa eklendi (`useful_passage_anchor` UPDATE);
--   (14) **kütüphane notu dosyaya** çevrildi (kapsam daraltma da serbestti);
--   (15) `id` değiştirildi (satır kimliği taşındı).
-- FK iki yazımı zaten reddediyordu (olmayan emsal, olmayan dosya) ve
-- `similarity_note=NULL` NOT NULL'a takılıyordu; kapı bunları **yedekler**
-- (FK'siz açılan bağlantıda da doğru hüküm verir).
--
-- Yazılan kural (2 tetikleyici):
--   * INSERT — `id`/`precedent_id` dolu; `precedent_id` **var olan** emsali
--     gösterir; `case_id_optional` NULL ya da **silinmemiş** bir dosya;
--     `created_at` dolu ve ISO-8601 damgası (`YYYY-MM-DDTHH:MM:SS…`);
--     üç metin alanı **kırpılmış** (çekirdek `trim` yazar); dolu çapa
--     boşluktan ibaret olamaz.
--   * UPDATE — sekiz sütunun **hepsi** değişmez: not yazıldıktan sonra
--     kapsamı, konusu, metni ve zamanı donar. Düzeltme yolu **yeni not**
--     yazmaktır (çekirdekte de tek yol budur; ekleme append-only sicildir).
-- Ret `DIM_INTEGRITY:precedent_notes` → `StorageError::Integrity` →
-- **`E_DB_INTEGRITY`**.
--
-- Kapsam taraması: tablo `case_id` değil **`case_id_optional`** taşır, bu
-- yüzden `scope_violations()` alanı **eklenmez** (`GUARDED_TABLES` **25
-- kalır**; 0044/0045/0046/0047/0048 ile aynı sınıf — kural dosya kapsamı
-- değil **not bütünlüğü**dür).
--
-- Dürüst sınırlar: (a) v49 öncesi kapsamı/metni bozuk satırlar **silinmez**
-- ve artık **onarılamaz** (donmuş) — temizlik ürün kararı. (b) Kapı notun
-- **doğruluğunu** değil **değişmezliğini** kanıtlar: yazan taraf yalan bir
-- notu bir kez yazabilir. (c) `useful_passage_anchor`ın hedef metinde
-- **karşılığı olduğu** doğrulanmaz (bugün arayüz daima NULL gönderir; dolu
-- çapa yalnız biçimce denetlenir). (d) Soft-delete sonradan olabilir: kapı
-- yalnız **doğuşta** dosyanın yaşadığını ister (tarihsel gerçek). (e) Tetikleyicileri
-- anahtarı olan saldırgan düşürebilir. (f) Arayüz davranışı değişmedi;
-- gerçek pencere + görsel QA **insan bekliyor**.

CREATE TRIGGER precedent_notes_content_guard_bi
BEFORE INSERT ON precedent_notes
WHEN length(trim(NEW.id)) = 0
  OR length(trim(NEW.precedent_id)) = 0
  OR NOT EXISTS (SELECT 1 FROM precedents p WHERE p.id = NEW.precedent_id)
  OR (NEW.case_id_optional IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM cases c
                       WHERE c.id = NEW.case_id_optional
                         AND c.deleted_at IS NULL))
  OR NEW.created_at NOT LIKE '____-__-__T__:__:__%'
  OR NEW.similarity_note <> trim(NEW.similarity_note)
  OR NEW.distinguishing_note <> trim(NEW.distinguishing_note)
  OR NEW.user_conclusion <> trim(NEW.user_conclusion)
  OR (NEW.useful_passage_anchor IS NOT NULL
      AND length(trim(NEW.useful_passage_anchor)) = 0)
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedent_notes');
END;

CREATE TRIGGER precedent_notes_content_guard_bu
BEFORE UPDATE OF id, precedent_id, case_id_optional, useful_passage_anchor,
                 similarity_note, distinguishing_note, user_conclusion, created_at
ON precedent_notes
WHEN NEW.id IS NOT OLD.id
  OR NEW.precedent_id IS NOT OLD.precedent_id
  OR NEW.case_id_optional IS NOT OLD.case_id_optional
  OR NEW.useful_passage_anchor IS NOT OLD.useful_passage_anchor
  OR NEW.similarity_note IS NOT OLD.similarity_note
  OR NEW.distinguishing_note IS NOT OLD.distinguishing_note
  OR NEW.user_conclusion IS NOT OLD.user_conclusion
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'DIM_INTEGRITY:precedent_notes');
END;
