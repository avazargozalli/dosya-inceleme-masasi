-- 0028: maskeleme planının **çıktı sürümü** bağı (şartname §15, F13-T12 izi).
--
-- Her maskelemede yeni bir belge + sürüm üretilir ve `parent_version_id` ile
-- kaynak sürüme bağlanır; ama plan satırı yalnız **kaynak** sürümü taşıyordu.
-- Sicilde "hangi maskeleme hangi çıktıyı üretti" sorusunun yanıtı yoktu; aynı
-- kaynak sürümden iki maskeleme yapıldığında çıktılar birbirinden ayırt
-- edilemiyordu (çıktıyı bulmak için belge adına bakmak gerekiyordu).
--
-- `output_version_id` yabancı anahtardır (`ON DELETE SET NULL`): çıktı sürümü
-- silinse bile denetim satırı **yaşar**, bağ çözülür — ad/çıktı uydurulmaz.
-- Bu sütundan önce yazılmış planlar NULL kalır; arayüz "çıktı kayıtlı değil"
-- der, çıktı varmış gibi göstermez.
ALTER TABLE redaction_plans ADD COLUMN output_version_id TEXT
  REFERENCES document_versions(id) ON DELETE SET NULL;

CREATE INDEX idx_redaction_plans_output ON redaction_plans(output_version_id);
