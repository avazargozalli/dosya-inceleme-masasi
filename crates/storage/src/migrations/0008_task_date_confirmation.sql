-- 0008: görev tarihi kullanıcı onayı (F08-T10).
--
-- Tarih yazmak onay değildir: hatırlatıcı yalnız kullanıcının açıkça onayladığı
-- tarih için kurulur. Onay ayrı kolonda tutulur; böylece tarihin kaynağı
-- (`date_origin`) bozulmadan kalır ve onay geri alınabilir.
ALTER TABLE tasks ADD COLUMN date_confirmed_at TEXT;

CREATE INDEX idx_tasks_reminder
  ON tasks(case_id, status, date_confirmed_at, user_due_date);
