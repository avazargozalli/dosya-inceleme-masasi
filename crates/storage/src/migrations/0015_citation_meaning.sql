-- F12-T07: alıntının **anlam** incelemesi.
-- Künye doğruluğu, alıntının birebir bulunması ve anlamın kararı yansıtması
-- ayrı boyutlardır. Bu sütunlar üçüncüsünü taşır; boş bırakmak "incelenmedi"
-- demektir, "doğru" demek değildir.
ALTER TABLE citation_checks ADD COLUMN meaning_note TEXT NOT NULL DEFAULT '';
ALTER TABLE citation_checks ADD COLUMN meaning_reviewed_at TEXT;
