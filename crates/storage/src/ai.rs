//! AI çalıştırma ve aktarım sicili.
//!
//! Değişmez kural: geçerli bir aktarım onayı (`transfer_consents`) olmadan
//! `ai_runs` başlatılamaz. Onay sağlayıcı, dosya ve belge listesine bağlıdır;
//! iptal edilirse yeni çalıştırma açılamaz. Gönderilen içerik `ai_context_items`
//! ve `transfer_receipts` ile denetlenebilir.

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::outbound_mask::{
    mask_terms, parse_runs, validate_terms, MaskError, MaskRun, MASK_PLACEHOLDER,
};
use crate::{now_iso, Database, NewAnchor, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRunRow {
    pub id: String,
    pub case_id: String,
    pub task_type: String,
    pub provider_id: Option<String>,
    pub model_snapshot: String,
    pub input_manifest_hash: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub consent_id: Option<String>,
    pub status: String,
    pub usage_report: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAiRun {
    pub task_type: String,
    pub provider_id: String,
    pub model_snapshot: String,
    pub input_manifest_hash: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub consent_id: String,
    /// Gönderilecek **maske terimleri** (plandan türetilenler dâhil; boş liste
    /// = maskesiz gönderim).
    ///
    /// Onay maskeye bağlıdır: alan **tip düzeyinde zorunludur** ki çağıran
    /// "maskeyi iletmeyi unutarak" onaylanan maskeyi sessizce düşüremesin
    /// (eksik iletiş = maskesiz istek = onay maskeli ise `E_AI_MASK_CHANGED`).
    #[serde(default)]
    pub mask_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferConsentRow {
    pub id: String,
    pub provider_snapshot: String,
    pub case_scope: String,
    pub purpose: String,
    pub selected_document_ids: Vec<String>,
    pub expires_at: Option<String>,
    pub consent_text_version: String,
    pub approved_at: String,
    pub revoked_at: Option<String>,
    /// Onayın bağlandığı **maske tanımı** (`outbound_transforms` kimliği).
    ///
    /// `None` = onay bu sütundan önce alınmış: onaylanan maske **bilinmiyor**,
    /// bu yüzden çalıştırma reddedilir (`E_AI_CONSENT_UNBOUND`). Yeni onaylar
    /// maskesizken de bir tanıma (terim sayısı 0) bağlanır.
    pub outbound_transform_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTransferConsent {
    /// Onayın bağlanacağı sağlayıcı **profili**. Kimlik dizesini (`adapter@sürüm`)
    /// çağıran yazmaz; çekirdek profilden türetir (uç nokta + model parmak izi
    /// dahil), böylece arayüz ile çekirdek ayrışamaz.
    pub provider_id: String,
    pub case_scope: String,
    pub purpose: String,
    #[serde(default)]
    pub selected_document_ids: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    pub consent_text_version: String,
    /// Onaylanan **maske terimleri** (serbest metin). Terimler kasaya
    /// yazılmaz: onay yalnız tanımın özetine (`outbound_transforms`) bağlanır.
    #[serde(default)]
    pub mask_terms: Vec<String>,
    /// Onaylanan **karartma planı** (varsa). Plan bölgelerinden türetilen
    /// terimler onayın parçasıdır; plan onayın kapsamındaki bir belgeye ait
    /// olmalıdır (`E_MASK_PLAN_SCOPE`, gönderim yoluyla aynı kapı).
    #[serde(default)]
    pub mask_plan_id: Option<String>,
}

/// Onay envanteri satırı: kayıt + **çekirdeğin** hesapladığı etkinlik.
///
/// `active` arayüzdeki saatten değil, `consent_is_active` ile **aynı** kuraldan
/// gelir (iptal yok + süre dolmamış). Böylece liste ile çalıştırma kapısı
/// ayrışamaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferConsentListRow {
    #[serde(flatten)]
    pub consent: TransferConsentRow,
    pub active: bool,
    /// Onayın bağlandığı varış yeri hâlâ **bir** yürürlükteki sağlayıcı
    /// profilinden türetilebiliyor mu?
    ///
    /// `start_ai_run` onay özetini çalıştırılan profilin özetiyle karşılaştırır;
    /// hiçbir profil aynı özeti vermiyorsa onay **hiçbir** profille
    /// yürütülemez. `active` yalnız iptal/süreye bakar — "kayıt yürürlükte" ile
    /// "yürütülebilir" aynı şey değildir; ekran ikisini ayrı yazar.
    pub provider_current: bool,
    /// Onayın bağlandığı maske tanımındaki **terim sayısı** (0 = maskesiz).
    /// Terimlerin kendisi saklanmaz; envanter yalnız "maskeli onay mı, kaç
    /// terimle" sorusunu yanıtlar. Bağ yoksa (eski onay) 0'dır ve
    /// `consent.outbound_transform_id` `None` kalır — ekran ikisini ayrı yazar.
    pub mask_term_count: i64,
}

/// Bir aktarım makbuzunun **okunabilir** satırı.
///
/// Makbuz yazılıyordu ama okunamıyordu: hangi hedefe, kaç öğe, kaç bayt,
/// hangi içerik özetiyle çıktığı yalnız SQL'de duruyordu. Avukatın denetim
/// kanıtı budur; ekran bu satırları çekirdekten okur.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferReceiptRow {
    pub id: String,
    pub ai_run_id: String,
    pub case_id: String,
    pub task_type: String,
    /// Hedef etiketi: ana makine + model. Yol/sorgu/anahtar **yazılmaz**.
    pub destination_label: String,
    /// Gönderilen segment kimlikleri + metinlerinin sha256 özeti (tam).
    pub outbound_hash: String,
    pub item_count: i64,
    pub byte_count: i64,
    /// `attempted` | `dispatched` | `failed` | `interrupted`.
    ///
    /// `attempted`, "gönderim denendi, sonuç yazılmadı" demektir (süreç
    /// çöktü ya da yanıt gelmedi) — sessizce başarı sayılmaz. `interrupted`
    /// ise açılış uzlaştırmasının yazdığı **kesin** durumdur: süreç, sonucu
    /// yazamadan kapandı. İçerik çıkmış **olabilir**; satır ne "gönderildi"
    /// ne "gönderilemedi" der.
    pub status: String,
    /// Makbuzun açıldığı an (gönderim **denemesi**). Sütun adı tarihseldir
    /// (`dispatched_at`); `attempted` satırda da doludur.
    pub dispatched_at: String,
}

/// Sağlayıcı profilinin **varış yerine bağlı** kimliği: `adapter@politika`.
/// API profilinde kayıtlı uç nokta ve modelin kısa sha256 parmak izi de girer —
/// aynı adapter sürümüyle **başka** bir uç noktaya/modelle gönderim, önceki
/// onayla yapılamaz (şartname: izin kendiliğinden genişlemez).
fn provider_snapshot(adapter_id: &str, policy_version: &str, capabilities: &str) -> String {
    let value: serde_json::Value =
        serde_json::from_str(capabilities).unwrap_or(serde_json::Value::Null);
    let endpoint = value
        .get("endpoint")
        .and_then(|item| item.as_str())
        .unwrap_or_default();
    let model = value
        .get("model")
        .and_then(|item| item.as_str())
        .unwrap_or_default();
    if endpoint.is_empty() && model.is_empty() {
        return format!("{adapter_id}@{policy_version}");
    }
    let mut hasher = Sha256::new();
    hasher.update(endpoint.as_bytes());
    hasher.update(b"\n");
    hasher.update(model.as_bytes());
    let digest = hasher.finalize();
    let fingerprint: String = digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{adapter_id}@{policy_version}#{fingerprint}")
}

/// SHA-256 özetinin küçük harfli onaltılığı (gönderim sicili).
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Profil satırının kimlik için gereken alanları (ham `capabilities` dahil).
struct ProfileRecord {
    provider_type: String,
    adapter_id: String,
    policy_version: String,
    capabilities: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextItemRow {
    pub id: String,
    pub ai_run_id: String,
    pub document_version_id: String,
    /// Belgenin okunabilir adı (`documents.display_name`). Sürüm ya da belge
    /// bulunamazsa `None`: kimlik dururken **ad uydurulmaz**. Kayıt, belge
    /// silinse de denetim için okunur kalır (`ai_context_items` sürüme yabancı
    /// anahtarla bağlı değil).
    pub document_title: Option<String>,
    /// Sürüm numarası (`document_versions.version_no`); bulunamazsa `None`.
    pub version_no: Option<i64>,
    pub source_anchor_id: Option<String>,
    pub inclusion_reason: String,
    pub bytes_sent: i64,
    /// Gönderilen segment sayısı (`segment_ids` sütunundan). `None`: sütun
    /// okunamadı — "0 segment" denmez. Eski satırlarda `[]` yazılıydı; bu
    /// sürümden sonra gerçek segment kimlikleri yazılır.
    pub segment_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAiContextItem {
    pub document_version_id: String,
    #[serde(default)]
    pub source_anchor_id: Option<String>,
    pub inclusion_reason: String,
    pub bytes_sent: i64,
    /// Gönderilen segmentlerin kimlikleri. Boş liste "segment bilinmiyor"
    /// demektir; çekirdek yazmadan önce her kimliği **kasadaki** segmentle
    /// doğrular (uydurma kimlik yazılamaz).
    #[serde(default)]
    pub segment_ids: Vec<String>,
}

/// Gönderilen tek bir segmentin sicil satırı (şartname `transfer_segments`).
///
/// Metin **çağırandan** gelir ama beyan edilemez: çekirdek kasadaki
/// `text_segments.raw_text` ile birebir karşılaştırır; farklıysa yazmaz.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTransferSegment {
    pub segment_id: String,
    pub document_version_id: String,
    pub text: String,
}

/// `transfer_segments` satırının **okunabilir** biçimi. Tam metin döndürülmez;
/// `text_head` ilk 120 karakterdir (denetimde gözle tanıma için).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSegmentRow {
    pub id: String,
    pub ai_run_id: String,
    pub document_version_id: Option<String>,
    pub segment_id: String,
    pub outbound_hash: String,
    pub char_count: i64,
    pub byte_count: i64,
    /// `identity` — gönderilen metin özgün segmentin kendisi;
    /// `mask_terms` — maske uygulandı (`outbound_transforms`); `unknown` —
    /// dönüşüm kimliği çözülemedi (kayıt bozuk, gizlenmez).
    pub transform_kind: String,
    /// Maskede kullanılan terim sayısı (kimlikte 0). Terimlerin **kendisi**
    /// saklanmaz.
    #[serde(default)]
    pub mask_term_count: i64,
    pub text_head: String,
}

/// Doğrulamanın «gönderilen metin» aşaması için satır (tam metinle).
#[derive(Debug, Clone)]
pub struct SentSegment {
    pub document_version_id: Option<String>,
    pub segment_id: String,
    /// Modelin **gördüğü** metin (maskeli olabilir).
    pub text: String,
    /// Gönderilen metnin özgün karakter aralıklarına eşlemesi. `None` —
    /// harita yok ya da okunamadı; bu durumda alıntı geri eşlenemez
    /// (şartname §737: «geri eşlenemeyen alıntı doğrulanmış sayılmaz»).
    pub runs: Option<Vec<MaskRun>>,
    /// `identity` · `mask_terms` · `unknown` (dönüşüm kimliği çözülemedi).
    pub transform_kind: String,
}

/// Gönderim maskesi isteği. Yalnız **terim listesi** taşır; yer tutucu sürüme
/// bağlıdır (`v1` → `[MASKED]`). Terimlerin kendisi kasaya yazılmaz; sicile
/// yalnız terim sayısı ve listenin özeti düşer.
#[derive(Debug, Clone)]
pub struct OutboundMask {
    pub terms: Vec<String>,
}

/// Önizlemede maskenin **tüm kapsama** uygulanacağı karakter bütçesi.
///
/// `mask_terms` maliyeti metin × terim olduğundan büyük kapsamda (ör. 1.000
/// sayfa) önizleme yavaşlardı. Bütçe aşılırsa yalnız **örnek** segment
/// maskelenir, eşleşme sayısı örneğe daralır ve bayt sayısı maskesiz üst sınır
/// olarak işaretlenir (`bytesExact=false`). Sessizce "maskeli" denmez.
pub const PREVIEW_MASK_CHAR_BUDGET: usize = 1_000_000;

/// Gönderilecek segmentin kasadaki kimliği. Metin **çağırandan alınmaz**:
/// çekirdek kasadaki `text_segments.raw_text`i okur, maskeyi kendisi uygular.
#[derive(Debug, Clone)]
pub struct OutboundSegmentRef {
    pub segment_id: String,
    pub document_version_id: String,
}

/// Kasadan türetilen gönderim satırı. `text` kasa dışına çıkacak **tam**
/// metindir (maskeli olabilir); sicile yazılan da budur.
#[derive(Debug, Clone)]
pub struct OutboundSegment {
    pub segment_id: String,
    pub document_version_id: String,
    pub text: String,
    pub masked_runs: usize,
    pub masked_chars: i64,
}

/// Gönderim hazırlığının sonucu: dönüşüm kimliği + gönderilecek metinler.
#[derive(Debug, Clone)]
pub struct OutboundRecord {
    pub transform_id: Option<String>,
    /// `identity` ya da `mask_terms`.
    pub transform_kind: String,
    pub term_count: usize,
    pub segments: Vec<OutboundSegment>,
    pub masked_runs: usize,
    pub masked_chars: i64,
}

impl OutboundRecord {
    fn empty() -> Self {
        Self {
            transform_id: None,
            transform_kind: "identity".into(),
            term_count: 0,
            segments: Vec::new(),
            masked_runs: 0,
            masked_chars: 0,
        }
    }
}

fn mask_storage_error(error: MaskError) -> StorageError {
    StorageError::MaskRejected(error.code())
}

/// Şartname §06.3 `ai_messages` satırı: **görünen** yanıt metni.
///
/// Özel akıl yürütme (chain-of-thought) alanı **yoktur**; yalnız sağlayıcının
/// kullanıcıya görünen yanıtı saklanır.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMessageRow {
    pub id: String,
    pub ai_run_id: String,
    pub role: String,
    pub visible_content: String,
    pub source_refs: Vec<String>,
    pub state: String,
}

impl Database {
    /// Profil satırının kimlik için gereken alanlarını okur; profil yoksa `None`.
    fn profile_record(&self, provider_id: &str) -> Result<Option<ProfileRecord>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT provider_type,adapter_id,policy_version,capabilities
                 FROM provider_profiles WHERE id=?1",
                [provider_id],
                |row| {
                    Ok(ProfileRecord {
                        provider_type: row.get(0)?,
                        adapter_id: row.get(1)?,
                        policy_version: row.get(2)?,
                        capabilities: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    /// Onay **sağlayıcı profiline** bağlanır. Kimlik dizesi çağırandan gelmez;
    /// profilden türetilir (`provider_snapshot`), böylece uç nokta/model
    /// değişince eski onay kendiliğinden geçersizleşir.
    pub fn create_transfer_consent(
        &self,
        new: &NewTransferConsent,
    ) -> Result<TransferConsentRow, StorageError> {
        if new.case_scope.trim().is_empty() || new.purpose.trim().is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        let profile = self
            .profile_record(&new.provider_id)?
            .ok_or(StorageError::AiProvider)?;
        // AI kapalıyken aktarım onayı alınamaz: "hiçbir yere gönderme" onayı
        // bir yetki değildir.
        if profile.provider_type == "off" {
            return Err(StorageError::AiOff);
        }
        let snapshot = provider_snapshot(
            &profile.adapter_id,
            &profile.policy_version,
            &profile.capabilities,
        );
        let exists: i64 = self.conn().query_row(
            "SELECT count(*) FROM cases WHERE id=?1 AND deleted_at IS NULL",
            [&new.case_scope],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(StorageError::NotFound);
        }
        // Onay **belge kümesine** bağlıdır: boş seçim hiçbir şeyi yetkilendirmez,
        // başka dosyanın ya da silinmiş bir belgenin kimliği onaya yazılamaz.
        let mut selected: Vec<String> = Vec::new();
        for document_id in &new.selected_document_ids {
            if selected.contains(document_id) {
                continue;
            }
            let belongs: i64 = self.conn().query_row(
                "SELECT count(*) FROM documents WHERE id=?1 AND case_id=?2 AND deleted_at IS NULL",
                rusqlite::params![document_id, new.case_scope],
                |r| r.get(0),
            )?;
            if belongs == 0 {
                return Err(StorageError::AiConsent);
            }
            selected.push(document_id.clone());
        }
        if selected.is_empty() {
            return Err(StorageError::AiConsent);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        // **Maske onayın parçasıdır** (şartname §763): onaylanan maske tanımı
        // kayda bağlanır. Terimler saklanmaz — `outbound_transforms` yalnız
        // kanonik terim listesinin özetini ve sayısını taşır (0027 sözleşmesi).
        // Plan seçildiyse bölgelerden türetilen terimler de onayın parçasıdır;
        // plan onayın kapsamındaki bir belgeye ait olmalıdır (gönderim yoluyla
        // **aynı** kapı ve aynı hata kodu).
        let mut approved_terms = canonical_mask_terms(&new.mask_terms)?;
        if let Some(plan_id) = new.mask_plan_id.as_deref() {
            // Kapsam kapısı **türetmeden önce**: plan onayın kapsamındaki bir
            // belgenin *güncel* sürümüne ait değilse hangi terimleri
            // bağlayacağımızı sormak anlamsız (ve gönderim yoluyla aynı kod).
            let plan = self
                .get_redaction_plan(plan_id)?
                .ok_or(StorageError::NotFound)?;
            if plan.case_id != new.case_scope {
                return Err(StorageError::NotFound);
            }
            let mut plan_in_scope = false;
            for document_id in &selected {
                let current: Option<Option<String>> = self
                    .conn()
                    .query_row(
                        "SELECT current_version_id FROM documents
                         WHERE id=?1 AND case_id=?2 AND deleted_at IS NULL",
                        rusqlite::params![document_id, new.case_scope],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(Some(version_id)) = current {
                    if version_id == plan.document_version_id {
                        plan_in_scope = true;
                    }
                }
            }
            if !plan_in_scope {
                return Err(StorageError::MaskRejected("E_MASK_PLAN_SCOPE"));
            }
            let derived = self.outbound_terms_from_plan(&new.case_scope, plan_id)?;
            approved_terms.extend(derived.terms.iter().cloned());
            approved_terms = canonical_mask_terms(&approved_terms)?;
        }
        // Onay **aynı amaç** için yenilendiyse önceki etkin onay kapanır: o
        // yetkinin güncel kapsamı en son onaydır (varış yeri değişse bile).
        // Farklı amaç ayrı yetkidir; ona dokunulmaz.
        let tx = self.conn().unchecked_transaction()?;
        let transform_id = ensure_outbound_transform(&tx, &approved_terms)?;
        tx.execute(
            "INSERT INTO transfer_consents (id,provider_snapshot,case_scope,purpose,selected_document_ids,expires_at,consent_text_version,approved_at,outbound_transform_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            rusqlite::params![
                id,
                snapshot,
                new.case_scope,
                new.purpose.trim(),
                serde_json::to_string(&selected).unwrap_or_else(|_| "[]".into()),
                new.expires_at,
                new.consent_text_version,
                now,
                transform_id
            ],
        )?;
        let superseded: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM transfer_consents
                 WHERE case_scope=?1 AND purpose=?2
                   AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?3)
                   AND id<>?4",
            )?;
            let rows = stmt.query_map(
                rusqlite::params![new.case_scope, new.purpose.trim(), now, id],
                |row| row.get(0),
            )?;
            rows.collect::<rusqlite::Result<Vec<String>>>()?
        };
        for previous in &superseded {
            tx.execute(
                "UPDATE transfer_consents SET revoked_at=?1 WHERE id=?2 AND revoked_at IS NULL",
                rusqlite::params![now, previous],
            )?;
            self.append_audit(
                "transfer_consent",
                Some(previous),
                "supersede",
                "local-user",
                &serde_json::json!({"superseded_by": id}),
            )?;
        }
        self.append_audit(
            "transfer_consent",
            Some(&id),
            "approve",
            "local-user",
            &serde_json::json!({"provider": snapshot, "documents": selected.len()}),
        )?;
        tx.commit()?;
        self.get_transfer_consent(&id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn get_transfer_consent(
        &self,
        id: &str,
    ) -> Result<Option<TransferConsentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,provider_snapshot,case_scope,purpose,selected_document_ids,expires_at,consent_text_version,approved_at,revoked_at,outbound_transform_id
             FROM transfer_consents WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_consent).optional()?)
    }

    pub fn revoke_transfer_consent(&self, id: &str) -> Result<(), StorageError> {
        let changed = self.conn().execute(
            "UPDATE transfer_consents SET revoked_at=?1 WHERE id=?2 AND revoked_at IS NULL",
            rusqlite::params![now_iso(), id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "transfer_consent",
            Some(id),
            "revoke",
            "local-user",
            &serde_json::json!({}),
        )?;
        Ok(())
    }

    /// Bir dosyanın **bütün** aktarım onayları — en yeni önce.
    ///
    /// Envanter olmadan etkin bir onay görünmez ve iptal edilemezdi: arayüz
    /// yalnız kendi açtığı son onayı tutuyordu. Liste dosyaya kapsanmıştır;
    /// başka dosyanın onayı (ve belge kimlikleri) sızmaz.
    pub fn list_transfer_consents(
        &self,
        case_id: &str,
    ) -> Result<Vec<TransferConsentListRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT c.id,c.provider_snapshot,c.case_scope,c.purpose,c.selected_document_ids,c.expires_at,c.consent_text_version,c.approved_at,c.revoked_at,c.outbound_transform_id,
                    COALESCE(t.term_count, 0)
             FROM transfer_consents c
             LEFT JOIN outbound_transforms t ON t.id = c.outbound_transform_id
             WHERE c.case_scope=?1
             ORDER BY c.approved_at DESC, c.id DESC",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok((map_consent(row)?, row.get::<_, i64>(10)?))
        })?;
        let now = now_iso();
        // Yürürlükteki varış yerleri **bir kez** okunur: satır başına ayrı
        // sorgu, aynı liste içinde farklı anların karışmasına yol açardı.
        let current = self.current_provider_snapshots()?;
        let mut out = Vec::new();
        for row in rows {
            let (consent, mask_term_count) = row?;
            let active = consent_active_at(&consent, &now);
            let provider_current = current
                .iter()
                .any(|snapshot| snapshot == consent.provider_snapshot.trim());
            out.push(TransferConsentListRow {
                consent,
                active,
                provider_current,
                mask_term_count,
            });
        }
        Ok(out)
    }

    /// Yürürlükteki (`off` dışı) profillerin türetilmiş varış yeri özetleri.
    ///
    /// Onay yalnız `provider_snapshot` dizesini saklar (profil kimliği değil);
    /// bu yüzden "hâlâ yürütülebilir mi" sorusu **özet eşitliğiyle** yanıtlanır —
    /// `start_ai_run`ın kullandığı kuralın aynısı.
    fn current_provider_snapshots(&self) -> Result<Vec<String>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT adapter_id,policy_version,capabilities FROM provider_profiles
             WHERE provider_type<>'off'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (adapter_id, policy_version, capabilities) = row?;
            out.push(provider_snapshot(
                &adapter_id,
                &policy_version,
                capabilities.as_deref().unwrap_or_default(),
            ));
        }
        Ok(out)
    }

    /// Onay geçerli mi? İptal edilmiş veya süresi dolmuş onay yeni çalıştırma açamaz.
    pub fn consent_is_active(&self, id: &str) -> Result<bool, StorageError> {
        let consent = self
            .get_transfer_consent(id)?
            .ok_or(StorageError::NotFound)?;
        Ok(consent_active_at(&consent, &now_iso()))
    }

    pub fn start_ai_run(&self, case_id: &str, new: &NewAiRun) -> Result<AiRunRow, StorageError> {
        if !self.consent_is_active(&new.consent_id)? {
            return Err(StorageError::AiConsent);
        }
        let consent = self
            .get_transfer_consent(&new.consent_id)?
            .ok_or(StorageError::NotFound)?;
        if consent.case_scope != case_id {
            return Err(StorageError::AiConsent);
        }
        if consent.selected_document_ids.is_empty() {
            return Err(StorageError::AiConsent);
        }
        // Onaydan sonra belge silinmiş/taşınmışsa kapsam daralmıştır: yeni
        // çalıştırma açılmaz (izin kendiliğinden genişlemez, daralmış izinle
        // de gönderim yapılmaz).
        for document_id in &consent.selected_document_ids {
            let live: i64 = self.conn().query_row(
                "SELECT count(*) FROM documents WHERE id=?1 AND case_id=?2 AND deleted_at IS NULL",
                rusqlite::params![document_id, case_id],
                |r| r.get(0),
            )?;
            if live == 0 {
                return Err(StorageError::AiConsent);
            }
        }
        // Onay **sağlayıcıya ve varış yerine** bağlıdır: başka bir profille ya
        // da aynı profilin uç nokta/modeli değiştikten sonra aynı onay
        // kullanılamaz — şartname "mevcut izin kendiliğinden genişlemez".
        let profile = self
            .profile_record(&new.provider_id)?
            .ok_or(StorageError::AiProvider)?;
        let current = provider_snapshot(
            &profile.adapter_id,
            &profile.policy_version,
            &profile.capabilities,
        );
        if consent.provider_snapshot.trim() != current {
            // "Onay yok" demek yanlış olurdu: onay **var**, varış yeri değişti.
            return Err(StorageError::AiProviderChanged);
        }
        // **Maske onayın parçasıdır** (şartname §763): onaylanan maske tanımı
        // ile gönderilecek maske aynı olmalı. Maskenin kaldırılması izni
        // **genişletir**; onaylanmayan bir maske de onaylanmayan metin
        // gönderir — ikisi de yeni onay ister. Onay maskesiz alınmışsa
        // gönderilecek maske de boş olmalıdır (eşitlik, tek yön değil).
        let Some(approved_transform) = consent.outbound_transform_id.as_deref() else {
            return Err(StorageError::AiConsentUnbound);
        };
        let approved_hash: String = self.conn().query_row(
            "SELECT settings_hash FROM outbound_transforms WHERE id=?1",
            [approved_transform],
            |row| row.get(0),
        )?;
        let wanted_hash = outbound_settings_hash(&canonical_mask_terms(&new.mask_terms)?);
        if approved_hash != wanted_hash {
            return Err(StorageError::AiMaskChanged);
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO ai_runs (id,case_id,task_type,provider_id,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,consent_id,status,usage_report,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'running','{\"billing\":\"unknown\"}',?10)",
            rusqlite::params![
                id,
                case_id,
                new.task_type,
                new.provider_id,
                new.model_snapshot,
                new.input_manifest_hash,
                new.prompt_version,
                new.output_schema_version,
                new.consent_id,
                now_iso()
            ],
        )?;
        self.append_audit(
            "ai_run",
            Some(&id),
            "start",
            "local-user",
            &serde_json::json!({"caseId": case_id}),
        )?;
        self.get_ai_run(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn get_ai_run(&self, id: &str) -> Result<Option<AiRunRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,task_type,provider_id,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,consent_id,status,usage_report,created_at,completed_at
             FROM ai_runs WHERE id=?1",
        )?;
        Ok(stmt.query_row([id], map_run).optional()?)
    }

    pub fn list_ai_runs(&self, case_id: &str) -> Result<Vec<AiRunRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,case_id,task_type,provider_id,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,consent_id,status,usage_report,created_at,completed_at
             FROM ai_runs WHERE case_id=?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([case_id], map_run)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Gönderilen içeriğin denetlenebilir sicili. `bytes_sent` gerçek dışa giden
    /// bayt sayısıdır; tahmini değer yazılmaz.
    pub fn record_context_items(
        &self,
        run_id: &str,
        items: &[NewAiContextItem],
    ) -> Result<usize, StorageError> {
        if self.get_ai_run(run_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let tx = self.conn().unchecked_transaction()?;
        for item in items {
            if item.bytes_sent < 0 {
                return Err(StorageError::InvalidSetting);
            }
            // Segment kimlikleri **kasadan** doğrulanır: var olmayan ya da
            // başka sürüme ait bir kimlik yazılamaz. Boş liste «bilinmiyor»
            // demektir (eski davranış), uydurma kimlik değil.
            let mut seen: Vec<&str> = Vec::new();
            for segment_id in &item.segment_ids {
                if segment_id.trim().is_empty() || segment_id.trim() != segment_id {
                    return Err(StorageError::InvalidSetting);
                }
                if seen.contains(&segment_id.as_str()) {
                    return Err(StorageError::InvalidSetting);
                }
                seen.push(segment_id);
                let exists: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM text_segments s
                     JOIN parse_runs pr ON pr.id = s.parse_run_id
                     WHERE s.id=?1 AND pr.document_version_id=?2",
                    rusqlite::params![segment_id, item.document_version_id],
                    |row| row.get(0),
                )?;
                if exists == 0 {
                    return Err(StorageError::NotFound);
                }
            }
            let segment_ids =
                serde_json::to_string(&item.segment_ids).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "INSERT INTO ai_context_items (id,ai_run_id,document_version_id,source_anchor_id,segment_ids,inclusion_reason,bytes_sent)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    run_id,
                    item.document_version_id,
                    item.source_anchor_id,
                    segment_ids,
                    item.inclusion_reason,
                    item.bytes_sent
                ],
            )?;
        }
        tx.commit()?;
        Ok(items.len())
    }

    /// Gönderilen içeriğin **okunabilir** sicili.
    ///
    /// Sıra kayıt sırasıdır (`rowid`): aynı çalıştırmada aynı damgaya düşen iki
    /// satır bile gönderim sırasını korur. Belge adı ve sürüm numarası **sol
    /// birleşimle** gelir — belge/sürüm silinmişse satır yine döner, ad alanı
    /// `None` kalır (uydurma ad yok, sessizce düşen denetim satırı yok).
    pub fn list_context_items(&self, run_id: &str) -> Result<Vec<AiContextItemRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT c.id,c.ai_run_id,c.document_version_id,d.display_name,v.version_no,
                    c.source_anchor_id,c.inclusion_reason,c.bytes_sent,c.segment_ids
             FROM ai_context_items c
             LEFT JOIN document_versions v ON v.id = c.document_version_id
             LEFT JOIN documents d ON d.id = v.document_id
             WHERE c.ai_run_id=?1
             ORDER BY c.rowid ASC",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            let raw_segments: String = row.get(8)?;
            let segment_count = serde_json::from_str::<Vec<String>>(&raw_segments)
                .ok()
                .map(|ids| i64::try_from(ids.len()).unwrap_or(i64::MAX));
            Ok(AiContextItemRow {
                id: row.get(0)?,
                ai_run_id: row.get(1)?,
                document_version_id: row.get(2)?,
                document_title: row.get(3)?,
                version_no: row.get(4)?,
                source_anchor_id: row.get(5)?,
                inclusion_reason: row.get(6)?,
                bytes_sent: row.get(7)?,
                segment_count,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Beyan kapısı: satırların metni kasadaki `text_segments.raw_text` ile
    /// **birebir** eşleşmeli. Kimlik yolunda metin çağırandan gelir; uydurma
    /// metin yazılamaz. Maskeleme için `record_outbound_segments` kullanılır.
    pub fn record_transfer_segments(
        &self,
        run_id: &str,
        segments: &[NewTransferSegment],
    ) -> Result<usize, StorageError> {
        for segment in segments {
            self.verify_declared_segment_text(segment)?;
        }
        let refs: Vec<OutboundSegmentRef> = segments
            .iter()
            .map(|segment| OutboundSegmentRef {
                segment_id: segment.segment_id.clone(),
                document_version_id: segment.document_version_id.clone(),
            })
            .collect();
        let record = self.record_outbound_segments(run_id, &refs, None)?;
        Ok(record.segments.len())
    }

    fn verify_declared_segment_text(
        &self,
        segment: &NewTransferSegment,
    ) -> Result<(), StorageError> {
        if segment.segment_id.trim().is_empty()
            || segment.segment_id.trim() != segment.segment_id
            || segment.document_version_id.trim().is_empty()
            || segment.text.is_empty()
            || segment.text.contains('\0')
        {
            return Err(StorageError::InvalidSetting);
        }
        let stored: Option<String> = self
            .conn()
            .query_row(
                "SELECT s.raw_text FROM text_segments s
                 JOIN parse_runs pr ON pr.id = s.parse_run_id
                 WHERE s.id=?1 AND pr.document_version_id=?2",
                rusqlite::params![segment.segment_id, segment.document_version_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(stored) = stored else {
            return Err(StorageError::NotFound);
        };
        if stored != segment.text {
            return Err(StorageError::InvalidSetting);
        }
        Ok(())
    }

    /// Gönderilecek segmentleri kasadan okur, istenirse **maskeler** ve sicile
    /// yazar; gönderilecek metinleri geri döndürür.
    ///
    /// Sözleşme (şartname §737):
    /// - metin çağırandan **alınmaz**; kasadaki segmentten okunur,
    /// - maske çekirdekte uygulanır ve `source_range_map` özgün Unicode
    ///   aralıklarını taşır,
    /// - dönüşüm sürümü `outbound_transforms`ta içerik adresli saklanır,
    /// - herhangi bir satır yazılamazsa **hiçbir satır** yazılmaz (tek işlem).
    ///
    /// Dönen `segments[].text` sicile yazılan metnin aynısıdır; çağıran
    /// gönderimi bundan kurmalıdır (kayıt ile gönderim ayrışamaz).
    pub fn record_outbound_segments(
        &self,
        run_id: &str,
        segments: &[OutboundSegmentRef],
        mask: Option<&OutboundMask>,
    ) -> Result<OutboundRecord, StorageError> {
        if self.get_ai_run(run_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let terms = match mask {
            Some(mask) => validate_terms(&mask.terms).map_err(mask_storage_error)?,
            None => Vec::new(),
        };
        if segments.is_empty() {
            return Ok(OutboundRecord::empty());
        }
        let tx = self.conn().unchecked_transaction()?;
        let transform_id = if terms.is_empty() {
            None
        } else {
            Some(ensure_outbound_transform(&tx, &terms)?)
        };
        let mut seen: Vec<&str> = Vec::new();
        let mut chars_in: i64 = 0;
        let mut chars_out: i64 = 0;
        let mut masked_runs_total = 0usize;
        let mut masked_chars_total = 0i64;
        let mut out = Vec::with_capacity(segments.len());
        for segment in segments {
            if segment.segment_id.trim().is_empty()
                || segment.segment_id.trim() != segment.segment_id
                || segment.document_version_id.trim().is_empty()
            {
                return Err(StorageError::InvalidSetting);
            }
            if seen.contains(&segment.segment_id.as_str()) {
                return Err(StorageError::InvalidSetting);
            }
            seen.push(segment.segment_id.as_str());
            let already: i64 = tx.query_row(
                "SELECT COUNT(*) FROM transfer_segments
                 WHERE ai_run_id=?1 AND outbound_segment_id=?2",
                rusqlite::params![run_id, segment.segment_id],
                |row| row.get(0),
            )?;
            if already > 0 {
                return Err(StorageError::InvalidSetting);
            }
            let stored: Option<String> = tx
                .query_row(
                    "SELECT s.raw_text FROM text_segments s
                     JOIN parse_runs pr ON pr.id = s.parse_run_id
                     WHERE s.id=?1 AND pr.document_version_id=?2",
                    rusqlite::params![segment.segment_id, segment.document_version_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(raw) = stored else {
                return Err(StorageError::NotFound);
            };
            if raw.trim().is_empty() {
                // Boş segment gönderilmez: maskeyi kanıtlayacak metin yok.
                return Err(StorageError::InvalidSetting);
            }
            let masked = mask_terms(&raw, &terms).map_err(mask_storage_error)?;
            let source_chars = i64::try_from(raw.chars().count()).unwrap_or(i64::MAX);
            let out_chars = i64::try_from(masked.text.chars().count()).unwrap_or(i64::MAX);
            let hash = sha256_hex(masked.text.as_bytes());
            let map = serde_json::to_string(&masked.runs)
                .map_err(|_| StorageError::MaskRejected(MaskError::InvalidMap.code()))?;
            tx.execute(
                "INSERT INTO transfer_segments
                   (id,ai_run_id,outbound_segment_id,source_segment_id,outbound_text,outbound_hash,
                    transform_manifest_id,source_range_map,document_version_id,source_char_count)
                 VALUES (?1,?2,?3,?3,?4,?5,?6,?7,?8,?9)",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    run_id,
                    segment.segment_id,
                    masked.text,
                    hash,
                    transform_id,
                    map,
                    segment.document_version_id,
                    source_chars
                ],
            )?;
            chars_in = chars_in.saturating_add(source_chars);
            chars_out = chars_out.saturating_add(out_chars);
            masked_runs_total += masked.masked_runs;
            masked_chars_total = masked_chars_total
                .saturating_add(i64::try_from(masked.masked_chars).unwrap_or(i64::MAX));
            out.push(OutboundSegment {
                segment_id: segment.segment_id.clone(),
                document_version_id: segment.document_version_id.clone(),
                text: masked.text,
                masked_runs: masked.masked_runs,
                masked_chars: i64::try_from(masked.masked_chars).unwrap_or(i64::MAX),
            });
        }
        tx.commit()?;
        self.append_audit(
            "transfer_segment",
            Some(run_id),
            "record",
            "local-system",
            &serde_json::json!({
                "segments": segments.len(),
                "chars": chars_in,
                "outboundChars": chars_out,
                "transform": if transform_id.is_some() { "mask_terms" } else { "identity" },
                "maskedRuns": masked_runs_total,
                "maskedChars": masked_chars_total,
            }),
        )?;
        Ok(OutboundRecord {
            transform_id,
            transform_kind: if terms.is_empty() {
                "identity".into()
            } else {
                "mask_terms".into()
            },
            term_count: terms.len(),
            segments: out,
            masked_runs: masked_runs_total,
            masked_chars: masked_chars_total,
        })
    }

    /// Gönderim sicilinin **okunabilir** satırları; sıra kayıt sırasıdır
    /// (`rowid`). Tam metin döndürülmez (ilk 120 karakter + özet yeter).
    /// Dönüşüm türü `outbound_transforms`tan okunur: ekran "maskelendi" ile
    /// "kimlik" arasındaki farkı uydurmadan gösterir.
    pub fn list_transfer_segments(
        &self,
        run_id: &str,
    ) -> Result<Vec<TransferSegmentRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT t.id,t.ai_run_id,t.document_version_id,t.outbound_segment_id,t.outbound_hash,
                    length(t.outbound_text),length(CAST(t.outbound_text AS BLOB)),
                    t.transform_manifest_id,substr(t.outbound_text,1,120),
                    x.kind,x.term_count
             FROM transfer_segments t
             LEFT JOIN outbound_transforms x ON x.id = t.transform_manifest_id
             WHERE t.ai_run_id=?1 ORDER BY t.rowid ASC",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            let transform_manifest_id: Option<String> = row.get(7)?;
            let joined_kind: Option<String> = row.get(9)?;
            let transform_kind = if transform_manifest_id.is_none() {
                "identity".to_string()
            } else {
                joined_kind.unwrap_or_else(|| "unknown".to_string())
            };
            Ok(TransferSegmentRow {
                id: row.get(0)?,
                ai_run_id: row.get(1)?,
                document_version_id: row.get(2)?,
                segment_id: row.get(3)?,
                outbound_hash: row.get(4)?,
                char_count: row.get(5)?,
                byte_count: row.get(6)?,
                transform_kind,
                mask_term_count: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                text_head: row.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Doğrulamanın **gönderilen metin** aşaması için satırlar (tam metinle).
    /// Aday doğrulaması alıntıyı önce burada arar; kasa dışına çıkan metin
    /// budur. `runs`, alıntının özgün karakter aralığına geri eşlenmesini
    /// sağlar; okunamayan harita `None` döner (alıntı doğrulanmış sayılmaz).
    pub(crate) fn sent_segments(&self, run_id: &str) -> Result<Vec<SentSegment>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT t.document_version_id,t.outbound_segment_id,t.outbound_text,
                    t.source_range_map,t.transform_manifest_id,
                    COALESCE(x.kind,'identity'),t.source_char_count
             FROM transfer_segments t
             LEFT JOIN outbound_transforms x ON x.id = t.transform_manifest_id
             WHERE t.ai_run_id=?1 ORDER BY t.rowid ASC",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            let text: String = row.get(2)?;
            let map_json: String = row.get(3)?;
            let transform: Option<String> = row.get(4)?;
            let kind: String = row.get(5)?;
            let source_chars: Option<i64> = row.get(6)?;
            let out_len = text.chars().count();
            let runs = if transform.is_none() {
                // Kimlik satırı: eşleme özdeşliktir (şartname §737).
                Some(identity_runs(out_len))
            } else {
                match source_chars.and_then(|n| usize::try_from(n).ok()) {
                    Some(source_len) => parse_runs(&map_json, out_len, source_len).ok(),
                    None => None,
                }
            };
            Ok(SentSegment {
                document_version_id: row.get(0)?,
                segment_id: row.get(1)?,
                text,
                runs,
                transform_kind: if transform.is_none() {
                    "identity".to_string()
                } else {
                    kind
                },
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn finish_ai_run(
        &self,
        run_id: &str,
        status: &str,
        usage_report: &str,
    ) -> Result<(), StorageError> {
        if !matches!(status, "succeeded" | "failed" | "cancelled" | "partial") {
            return Err(StorageError::InvalidSetting);
        }
        let changed = self.conn().execute(
            "UPDATE ai_runs SET status=?1, usage_report=?2, completed_at=?3 WHERE id=?4",
            rusqlite::params![status, usage_report, now_iso(), run_id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "ai_run",
            Some(run_id),
            status,
            "local-system",
            &serde_json::json!({}),
        )?;
        Ok(())
    }

    /// Görünen yanıt metninin üst sınırı (karakter). Üstünde metin **kırpılır**
    /// ve satır `truncated` durumuyla yazılır — sessiz kırpma yok.
    pub const AI_MESSAGE_CHAR_LIMIT: usize = 262_144;
    /// Bir yanıtın dayanabileceği en çok segment kimliği.
    pub const AI_MESSAGE_REF_LIMIT: usize = 200;

    /// Şartname §06.3 `ai_messages`: **görünen** yanıt metni kayda alınır.
    ///
    /// - Özel akıl yürütme (chain-of-thought) alanları bu sözleşmede yoktur;
    ///   çağıran yalnız sağlayıcının görünen içeriğini verir.
    /// - `state`: `visible` (yanıt yapısal şemaya çözüldü) ya da `unparsed`
    ///   (yanıt geldi ama şemaya çözülemedi). Metin sınırı aşarsa satır
    ///   `truncated` yazılır.
    /// - Kusurlu satır (boş içerik, kontrol karakteri, biçimsiz/yinelenen
    ///   kaynak başvurusu) **hiç yazılmaz**.
    pub fn record_ai_message(
        &self,
        run_id: &str,
        role: &str,
        content: &str,
        source_refs: &[String],
        state: &str,
    ) -> Result<AiMessageRow, StorageError> {
        if !matches!(role, "assistant" | "user" | "system" | "tool") {
            return Err(StorageError::AiMessageInvalid("bilinmeyen rol"));
        }
        if !matches!(state, "visible" | "unparsed") {
            return Err(StorageError::AiMessageInvalid("bilinmeyen durum"));
        }
        if self.get_ai_run(run_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        if content.trim().is_empty() {
            return Err(StorageError::AiMessageInvalid("içerik boş"));
        }
        if content
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(StorageError::AiMessageInvalid(
                "içerik kontrol karakteri taşıyor",
            ));
        }
        if source_refs.len() > Self::AI_MESSAGE_REF_LIMIT {
            return Err(StorageError::AiMessageInvalid(
                "kaynak başvurusu sınırı aşıldı",
            ));
        }
        let mut seen: Vec<&str> = Vec::new();
        for reference in source_refs {
            if reference.trim().is_empty()
                || reference.trim() != reference
                || reference.chars().count() > 128
                || reference.chars().any(char::is_control)
            {
                return Err(StorageError::AiMessageInvalid("kaynak başvurusu biçimsiz"));
            }
            if seen.contains(&reference.as_str()) {
                return Err(StorageError::AiMessageInvalid(
                    "kaynak başvurusu yineleniyor",
                ));
            }
            seen.push(reference);
        }
        let mut stored = content.to_string();
        let mut stored_state = state.to_string();
        if stored.chars().count() > Self::AI_MESSAGE_CHAR_LIMIT {
            stored = stored.chars().take(Self::AI_MESSAGE_CHAR_LIMIT).collect();
            stored_state = "truncated".to_string();
        }
        let refs_json = serde_json::to_string(source_refs)
            .map_err(|_| StorageError::AiMessageInvalid("kaynak başvurusu yazılamadı"))?;
        let id = uuid::Uuid::now_v7().to_string();
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT INTO ai_messages (id,ai_run_id,role,visible_content,source_refs,state)
             VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![id, run_id, role, stored, refs_json, stored_state],
        )?;
        tx.commit()?;
        self.append_audit(
            "ai_message",
            Some(&id),
            "record",
            "local-system",
            &serde_json::json!({
                "role": role,
                "state": stored_state,
                "chars": stored.chars().count(),
            }),
        )?;
        Ok(AiMessageRow {
            id,
            ai_run_id: run_id.to_string(),
            role: role.to_string(),
            visible_content: stored,
            source_refs: source_refs.to_vec(),
            state: stored_state,
        })
    }

    /// Bir çalıştırmanın görünen yanıtları (kayıt sırası). Bozuk kaynak
    /// başvurusu sessizce yutulmaz: satır **okunamadı** diye hata verir.
    pub fn list_ai_messages(&self, run_id: &str) -> Result<Vec<AiMessageRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,ai_run_id,role,visible_content,source_refs,state
             FROM ai_messages WHERE ai_run_id=?1 ORDER BY rowid",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, ai_run_id, role, visible_content, refs_json, state) = row?;
            let source_refs: Vec<String> = serde_json::from_str(&refs_json)
                .map_err(|_| StorageError::AiMessageInvalid("kaynak başvurusu okunamadı"))?;
            out.push(AiMessageRow {
                id,
                ai_run_id,
                role,
                visible_content,
                source_refs,
                state,
            });
        }
        Ok(out)
    }

    /// Aktarım makbuzunu **deneme anında** açar (`attempted`).
    ///
    /// Neden gönderimden önce: `provider.propose` zaman aşımına düşerse içerik
    /// çıkmış **olabilir**. Yalnız başarıda makbuz yazmak, "denendi" ile
    /// "gönderildi" arasındaki farkı sicilden siler; düşen deneme hiç iz
    /// bırakmaz. Makbuz yazılamazsa aktarım **hiç başlamaz** (denetlenemeyen
    /// aktarım yapılmaz) — hata çağırana taşınır.
    pub fn begin_transfer_receipt(
        &self,
        run_id: &str,
        destination_label: &str,
        outbound_hash: &str,
        item_count: i64,
        byte_count: i64,
    ) -> Result<String, StorageError> {
        if self.get_ai_run(run_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.conn().execute(
            "INSERT INTO transfer_receipts (id,ai_run_id,destination_label,outbound_hash,item_count,byte_count,status,dispatched_at)
             VALUES (?1,?2,?3,?4,?5,?6,'attempted',?7)",
            rusqlite::params![id, run_id, destination_label, outbound_hash, item_count, byte_count, now_iso()],
        )?;
        self.append_audit(
            "transfer_receipt",
            Some(&id),
            "attempt",
            "local-system",
            &serde_json::json!({"destination": destination_label}),
        )?;
        Ok(id)
    }

    /// Deneme makbuzunu sonuçlandırır: `attempted` → `dispatched` | `failed`.
    ///
    /// Geçiş **tek** koşullu `UPDATE`dir: önce okuyup sonra yazmak, arada
    /// başka bir sonuçlandırmanın yazdığını ezme penceresi bırakırdı. Satır
    /// değişmediyse ya makbuz yoktur ya da zaten sonuçlanmıştır; ikisi ayrı
    /// kodla döner. Sonuçlanmış makbuz **yeniden yazılmaz** (sicil geriye
    /// dönük değiştirilemez). `attempted` kalan makbuz "denendi, sonuç
    /// yazılmadı" demektir — sessizce başarıya çevrilmez.
    pub fn finish_transfer_receipt(
        &self,
        receipt_id: &str,
        status: &str,
    ) -> Result<(), StorageError> {
        if status != "dispatched" && status != "failed" {
            return Err(StorageError::AiReceipt(format!(
                "bilinmeyen makbuz durumu: {status}"
            )));
        }
        let changed = self.conn().execute(
            "UPDATE transfer_receipts SET status=?2 WHERE id=?1 AND status='attempted'",
            rusqlite::params![receipt_id, status],
        )?;
        if changed == 0 {
            let current: Option<String> = self
                .conn()
                .query_row(
                    "SELECT status FROM transfer_receipts WHERE id=?1",
                    [receipt_id],
                    |row| row.get(0),
                )
                .optional()?;
            return Err(match current {
                None => StorageError::NotFound,
                Some(current) => {
                    StorageError::AiReceipt(format!("makbuz zaten sonuçlanmış: {current}"))
                }
            });
        }
        self.append_audit(
            "transfer_receipt",
            Some(receipt_id),
            status,
            "local-system",
            &serde_json::json!({}),
        )?;
        Ok(())
    }

    /// Açılış uzlaştırması: **sonucu yazılmadan kapanmış** gönderim makbuzları.
    ///
    /// `attempted` satırı "gönderim denendi, sonuç yazılmadı" demektir; onu
    /// sonuçlandıran çağrı yalnız gönderimi yapan süreçte bulunur
    /// (`finish_transfer_receipt`). Süreç öldüyse satır sonsuza kadar
    /// `attempted` kalıyordu ve arayüzde hâlâ uçuşta olabilecek bir deneme gibi
    /// görünüyordu — oysa onu yazacak süreç artık yok.
    ///
    /// Kasa açılışında hiçbir gönderim uçuşta olamaz: `ai_run_execute` kasa
    /// kilidini gönderim boyunca tutar ve komut dönerken makbuz sonuçlandırılır.
    /// Sonuçlandırma yazılamamış olsa bile o çağrı geri gelmez (`partial`
    /// raporlanır, satır `attempted` kalır) — yani açılış anında `attempted`
    /// olan satırın sonucunu yazacak **canlı bir çağrı yoktur**: süreç çökmüş
    /// ya da final yazımı düşmüştür; ikisi de "sonuç yok" demektir.
    ///
    /// `interrupted` yazar: satır ne "gönderildi" ne "gönderilemedi" der;
    /// içerik çıkmış **olabilir**. Geçiş tek `UPDATE`dir, sonuçlanmış satırlara
    /// dokunmaz ve kesin olduğu için aynı satır sonradan sonuçlandırılamaz.
    /// Kaç satırın kapandığını döndürür; sayı denetime yazılır (makbuz kimliği,
    /// hedef ve özet **yazılmaz**).
    pub fn reconcile_attempted_receipts(&self) -> Result<usize, StorageError> {
        let changed = self.conn().execute(
            "UPDATE transfer_receipts SET status='interrupted' WHERE status='attempted'",
            [],
        )?;
        if changed > 0 {
            // Denetim yazılamazsa kasa açılışı düşmez: satırlar zaten kesin
            // durumda; eksik denetim kaydı açılışı engellemez (yükseltme
            // göçünün `let _ =` deseniyle aynı gerekçe).
            let _ = self.append_audit(
                "transfer_receipt",
                None,
                "interrupt",
                "local-system",
                &serde_json::json!({ "interrupted": changed }),
            );
        }
        Ok(changed)
    }

    /// Kasa açılışında **yarım kalmış çalıştırmalar** kesin duruma bağlanır.
    ///
    /// `start_ai_run` satırı `running` olarak açar; durumu yalnız
    /// `finish_ai_run` yazar. Süreç çalıştırma sırasında ölürse satır **sonsuza
    /// kadar `running`** kalıyordu: aylar sonra açılan ekranda "sürüyor" gibi
    /// görünüyordu — oysa onu bitirecek çağrı artık yok.
    ///
    /// Kasa açılışında hiçbir çalıştırma uçuşta olamaz: `ai_run_execute` kasa
    /// kilidini çalıştırmanın tamamı boyunca tutar (`commands.rs`), yani açılış
    /// anında `running` satırın sonucunu yazacak **canlı bir çağrı yoktur**;
    /// ya süreç çökmüştür ya da final yazımı düşmüştür.
    ///
    /// `ai_runs.status` CHECK kümesi `interrupted` taşımaz (şemayı değiştirmek
    /// tabloyu yeniden kurmayı gerektirir) → durum `failed` yazılır ve ayrım
    /// **`usage_report`** içinde taşınır (`error=interrupted`); ekran bu
    /// işareti görünce "oturum kesildi · sonuç yazılmadı" der, "başarısız" ya
    /// da "sürüyor" demez. `completed_at` **yazılmaz**: bitiş zamanı bilinmiyor,
    /// açılış saatini yazmak uydurma olurdu.
    ///
    /// Geçiş tek koşullu `UPDATE`dir; sonuçlanmış satıra dokunmaz ve ikinci
    /// açılış 0 döner (idempotent). Kaç satırın kapandığı denetime yazılır;
    /// satır kimlikleri ve çalıştırma içeriği **yazılmaz**.
    pub fn reconcile_running_ai_runs(&self) -> Result<usize, StorageError> {
        let report = serde_json::json!({
            "billing": "unknown",
            "error": "interrupted",
            "reason": "process_exit",
        })
        .to_string();
        let changed = self.conn().execute(
            "UPDATE ai_runs SET status='failed', usage_report=?1 WHERE status='running'",
            [report],
        )?;
        if changed > 0 {
            // Denetim yazılamazsa kasa açılışı düşmez: satırlar zaten kesin
            // durumda (makbuz uzlaştırmasıyla aynı gerekçe).
            let _ = self.append_audit(
                "ai_run",
                None,
                "interrupt",
                "local-system",
                &serde_json::json!({ "interrupted": changed }),
            );
        }
        Ok(changed)
    }

    /// Dosyanın aktarım makbuzları — **en yeni önce**.
    ///
    /// Makbuz `ai_runs`a bağlıdır; dosya kapsamı `ai_runs.case_id` üzerinden
    /// kurulur (başka dosyanın makbuzu bu listeye girmez). Bu, "ne çıktı,
    /// nereye, kaç bayt, hangi içerik özetiyle" sorusunun tek okuma yoludur.
    pub fn list_transfer_receipts(
        &self,
        case_id: &str,
    ) -> Result<Vec<TransferReceiptRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT r.id,r.ai_run_id,a.case_id,a.task_type,r.destination_label,r.outbound_hash,
                    r.item_count,r.byte_count,r.status,r.dispatched_at
             FROM transfer_receipts r JOIN ai_runs a ON a.id=r.ai_run_id
             WHERE a.case_id=?1
             ORDER BY r.dispatched_at DESC, r.id DESC",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok(TransferReceiptRow {
                id: row.get(0)?,
                ai_run_id: row.get(1)?,
                case_id: row.get(2)?,
                task_type: row.get(3)?,
                destination_label: row.get(4)?,
                outbound_hash: row.get(5)?,
                item_count: row.get(6)?,
                byte_count: row.get(7)?,
                status: row.get(8)?,
                dispatched_at: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileRow {
    pub id: String,
    pub provider_type: String,
    pub adapter_id: String,
    pub execution_environment: String,
    pub policy_version: String,
    /// Profilin **varış yeri** kimliği (uç nokta + modelin kısa özeti dahil).
    /// Onayın `provider_snapshot`ı ile aynı kuralla türetilir; ekran hangi
    /// profilin hangi varış yerine gittiğini ve onayın eşleşip eşleşmediğini
    /// tahmin etmeden yazar.
    pub provider_snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCandidateRow {
    pub id: String,
    pub ai_run_id: String,
    pub candidate_type: String,
    pub structured_payload: String,
    pub validation_state: String,
    pub validation_errors: String,
    /// Şema doğrulamasının sonucu: `valid` | `partial` | `invalid` | `legacy`.
    pub schema_state: String,
    /// Alıntı eşleştirmesinin sonucu: `verified` | `partial` | `failed` |
    /// `no_quotes` | `not_checked` | `legacy`.
    pub quote_state: String,
    pub quote_report: Vec<crate::ai_candidate::QuoteMatchReport>,
    pub checked_at: Option<String>,
}

/// Kapsamlılık raporu: hangi belge gerçekten ayrıştırıldı, hangi sayfa boş,
/// nerede OCR kullanıldı. "İncelenmedi" ile "temiz" karıştırılmaz.
///
/// Sayımlar **gönderilebilir korpusu** anlatır: her belgenin güncel sürümü ve o
/// sürümün sayfa üreten en yeni ayrıştırma koşusu (gönderim kapsamıyla aynı
/// kural). `versions`/`parse_runs`/`failed_parse_runs` **tarihçedir**.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReport {
    /// Silinmemiş belge sayısı.
    pub documents: i64,
    /// Bütün belge sürümleri (tarihçe).
    pub versions: i64,
    /// Bütün ayrıştırma koşuları (tarihçe).
    pub parse_runs: i64,
    /// **Gönderilebilir** korpustaki sayfa sayısı (güncel sürümlerin en yeni
    /// sayfa üreten koşuları); eski sürümler ve önceki koşular sayılmaz.
    pub pages: i64,
    pub ocr_pages: i64,
    pub segments: i64,
    /// Bu korpusta kullanılabilir metni olmayan sayfa (metin katmanı yok).
    pub empty_pages: i64,
    /// Güncel sürümü olmayan ya da güncel sürümü hiç ayrıştırılmamış belge.
    /// Eski sürümün metni gönderilmediği için bu belge "ayrıştırıldı" sayılmaz.
    pub unparsed_documents: i64,
    /// Ayrıştırma koşusu olup **hiçbir** sayfasında kullanılabilir metin
    /// olmayan belge (OCR'sız tarama). Gönderim kapsamı bu belgeyi atlar.
    pub documents_without_text: i64,
    /// Bütün başarısız koşular (tarihçe; bugünkü korpusu anlatmaz).
    pub failed_parse_runs: i64,
}

impl Database {
    pub fn list_ai_candidates(&self, run_id: &str) -> Result<Vec<AiCandidateRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,ai_run_id,type,structured_payload,validation_state,validation_errors,
                    schema_state,quote_state,quote_report,checked_at
             FROM ai_candidates WHERE ai_run_id=?1 ORDER BY rowid",
        )?;
        let rows = stmt.query_map([run_id], map_candidate)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Adayı inceleme kuyruğuna taşır. Aday doğrudan hukuki kayda dönüşmez;
    /// avukat kabul edene kadar `review_items` içinde `pending` kalır.
    ///
    /// Kapı: şeması geçersiz aday (`E_AI_SCHEMA`) ve metinde bulunmayan alıntı
    /// taşıyan aday (`E_AI_QUOTE`) kuyruğa alınmaz — uydurma alıntı kaynak
    /// sayılmaz (INV-02). Birebir doğrulanmış alıntılar için `source_anchors`
    /// kaydı oluşturulur ve `source_anchor_ids` bu kimliklerle yazılır.
    pub fn promote_candidate(
        &self,
        candidate_id: &str,
        case_id: &str,
    ) -> Result<String, StorageError> {
        let row = self
            .conn()
            .query_row(
                "SELECT c.type,c.structured_payload,c.ai_run_id,r.case_id,c.validation_state,c.quote_state,c.quote_report,c.schema_state
                 FROM ai_candidates c JOIN ai_runs r ON r.id=c.ai_run_id WHERE c.id=?1",
                [candidate_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        let (
            kind,
            payload,
            run_id,
            run_case,
            validation_state,
            quote_state,
            quote_report,
            schema_state,
        ) = row.ok_or(StorageError::NotFound)?;
        if run_case != case_id {
            return Err(StorageError::NotFound);
        }
        // Bu sürümden önce yazılmış aday doğrulanmamıştır: önce yeniden doğrula.
        if quote_state == "legacy" || schema_state == "legacy" {
            return Err(StorageError::AiSchema(
                "Aday bu sürümde doğrulanmamış; önce yeniden doğrulayın.".into(),
            ));
        }
        if quote_state == "failed" {
            return Err(StorageError::AiQuote(
                "Adaydaki alıntı belge metninde bulunamadı; kaynak olarak kullanılamaz.".into(),
            ));
        }
        if validation_state == "invalid" {
            return Err(StorageError::AiSchema(
                "Aday şeması geçersiz; incelemeye gönderilmez.".into(),
            ));
        }

        let quotes: Vec<crate::ai_candidate::QuoteMatchReport> =
            serde_json::from_str(&quote_report).unwrap_or_default();
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        let mut anchor_ids: Vec<String> = Vec::new();
        self.write_atomically(|| {
            for quote in &quotes {
                if let Some(anchor_id) = self.ensure_anchor(case_id, quote)? {
                    anchor_ids.push(anchor_id);
                }
            }
            let anchors = serde_json::to_string(&anchor_ids).unwrap_or_else(|_| "[]".into());
            self.conn().execute(
                "INSERT INTO review_items (id,case_id,item_kind,payload_schema_version,payload,source_anchor_ids,ai_run_id,state,created_at,updated_at,row_version)
                 VALUES (?1,?2,?3,1,?4,?5,?6,'pending',?7,?7,1)",
                rusqlite::params![id, case_id, kind, payload, anchors, run_id, now],
            )?;
            Ok(())
        })?;
        self.append_audit(
            "review_item",
            Some(&id),
            "promote",
            "local-user",
            &serde_json::json!({
                "candidateId": candidate_id,
                "anchors": anchor_ids.len(),
            }),
        )?;
        Ok(id)
    }

    /// Aynı alıntı için zaten aktif bir anchor varsa onu kullanır; böylece aynı
    /// adayın yeniden terfisi kaynak kaydını çoğaltmaz. Rapor `exact` değilse ya
    /// da konum alanları eksikse anchor oluşturulmaz (`None`).
    fn ensure_anchor(
        &self,
        case_id: &str,
        quote: &crate::ai_candidate::QuoteMatchReport,
    ) -> Result<Option<String>, StorageError> {
        if quote.state != "exact" {
            return Ok(None);
        }
        let (Some(parse_run_id), Some(segment_id), Some(char_start)) = (
            quote.parse_run_id.as_deref(),
            quote.segment_id.as_deref(),
            quote.char_start,
        ) else {
            return Ok(None);
        };
        let length = i64::try_from(quote.quote.chars().count()).unwrap_or(0);
        let end_offset = char_start.saturating_add(length);
        // Kimlik ve tekilleştirme **çekirdekte** (`create_anchor`) tek yerde:
        // aynı kanıt konumu ikinci kez yazılmaz, var olan anchor döner.
        let anchor = self.create_anchor(&NewAnchor {
            case_id: case_id.to_string(),
            document_version_id: quote.document_version_id.clone(),
            parse_run_id: parse_run_id.to_string(),
            segment_id: segment_id.to_string(),
            start_offset: char_start,
            end_offset,
            exact_quote: quote.quote.clone(),
        })?;
        Ok(Some(anchor.id))
    }

    pub fn list_provider_profiles(&self) -> Result<Vec<ProviderProfileRow>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id,provider_type,adapter_id,execution_environment,policy_version,capabilities
             FROM provider_profiles ORDER BY provider_type, adapter_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let adapter_id: String = row.get(2)?;
            let policy_version: String = row.get(4)?;
            let capabilities: Option<String> = row.get(5)?;
            Ok(ProviderProfileRow {
                id: row.get(0)?,
                provider_type: row.get(1)?,
                provider_snapshot: provider_snapshot(
                    &adapter_id,
                    &policy_version,
                    capabilities.as_deref().unwrap_or_default(),
                ),
                adapter_id,
                execution_environment: row.get(3)?,
                policy_version,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// `off` profili yoksa oluşturur; AI varsayılan olarak kapalıdır.
    pub fn ensure_off_provider(&self) -> Result<ProviderProfileRow, StorageError> {
        if let Some(row) = self
            .list_provider_profiles()?
            .into_iter()
            .find(|p| p.provider_type == "off")
        {
            return Ok(row);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        self.conn().execute(
            "INSERT INTO provider_profiles (id,provider_type,adapter_id,execution_environment,policy_version,created_at,updated_at)
             VALUES (?1,'off','off','local','1',?2,?2)",
            rusqlite::params![id, now],
        )?;
        Ok(ProviderProfileRow {
            id,
            provider_type: "off".into(),
            adapter_id: "off".into(),
            execution_environment: "local".into(),
            policy_version: "1".into(),
            provider_snapshot: provider_snapshot("off", "1", ""),
        })
    }

    /// Kapsam raporu **gönderim kapsamıyla aynı kuralı** kullanır: her belgenin
    /// güncel sürümü ve o sürümün **sayfa üreten en yeni** ayrıştırma koşusu
    /// (`resolve_transfer_scope` ile birebir aynı seçim). Böylece rapor,
    /// gönderilemeyecek bir belgeyi "ayrıştırıldı" saymaz ve `complete`
    /// ("Kapsam tam") yalnız gerçekten gönderilebilir korpusta doğrudur.
    ///
    /// "Sayfada kullanılabilir metin" ölçütü alıntı denetimiyle **tek**
    /// tanımdır ([`crate::intext_quotes::PAGE_TEXT_EXPR`]): boşluktan ibaret
    /// sayfa metinli sayılmaz, OCR'sız taranmış sayfa sayıya "metinli" girmez.
    pub fn coverage_report(&self, case_id: &str) -> Result<CoverageReport, StorageError> {
        // Gönderilebilir korpus: belge başına güncel sürümün en yeni **sayfa
        // üreten** koşusu. `run_id` NULL ise belge gönderilemez.
        const CURRENT_RUNS: &str = "WITH current_runs AS (\
               SELECT d.id AS document_id, \
                      (SELECT pr.id FROM parse_runs pr \
                        WHERE pr.document_version_id=d.current_version_id \
                          AND EXISTS (SELECT 1 FROM pages p WHERE p.parse_run_id=pr.id) \
                        ORDER BY pr.created_at DESC, pr.id DESC LIMIT 1) AS run_id \
                 FROM documents d \
                WHERE d.case_id=?1 AND d.deleted_at IS NULL\
             )";
        let q = |sql: &str| -> Result<i64, StorageError> {
            Ok(self.conn().query_row(sql, [case_id], |r| r.get(0))?)
        };
        let run_q = |tail: &str| format!("{CURRENT_RUNS} {tail}");
        let page_text = crate::intext_quotes::PAGE_TEXT_EXPR;
        let (text_pages, empty_pages, ocr_pages) = self.conn().query_row(
            &run_q(&format!(
                "SELECT \
                 COALESCE(sum(CASE WHEN trim({page_text}) <> '' THEN 1 ELSE 0 END),0),\
                 COALESCE(sum(CASE WHEN trim({page_text}) = '' THEN 1 ELSE 0 END),0),\
                 COALESCE(sum(CASE WHEN p.text_origin IN ('ocr','mixed') THEN 1 ELSE 0 END),0) \
                 FROM pages p JOIN current_runs c ON c.run_id=p.parse_run_id"
            )),
            [case_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        Ok(CoverageReport {
            documents: q(&run_q("SELECT count(*) FROM current_runs"))?,
            versions: q("SELECT count(*) FROM document_versions WHERE case_id=?1")?,
            parse_runs: q("SELECT count(*) FROM parse_runs WHERE case_id=?1")?,
            pages: text_pages.saturating_add(empty_pages),
            ocr_pages,
            segments: q(&run_q(
                "SELECT count(*) FROM text_segments s JOIN pages p ON p.id=s.page_id \
                 JOIN current_runs c ON c.run_id=p.parse_run_id",
            ))?,
            empty_pages,
            unparsed_documents: q(&run_q(
                "SELECT count(*) FROM current_runs WHERE run_id IS NULL",
            ))?,
            documents_without_text: q(&run_q(&format!(
                "SELECT count(*) FROM current_runs c WHERE c.run_id IS NOT NULL \
                 AND NOT EXISTS (SELECT 1 FROM pages p WHERE p.parse_run_id=c.run_id \
                                 AND trim({page_text}) <> '')"
            )))?,
            failed_parse_runs: q(
                "SELECT count(*) FROM parse_runs WHERE case_id=?1 AND status='failed'",
            )?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentPreview {
    pub documents: i64,
    pub segments: i64,
    /// Gönderilecek (maske verilmişse **maskeli**) metnin bayt sayısı.
    /// `mask.bytes_exact=false` ise bu sayı maskesiz **üst sınırdır**.
    pub bytes: i64,
    /// Maske **uygulanmadan** metnin bayt sayısı (karşılaştırma için).
    pub bytes_unmasked: i64,
    /// Gönderilecek ilk segmentin kısa örneği; maske verilmişse **maskeli**.
    /// Veri yoksa boş.
    pub sample: String,
    /// Kapsam dışı kalan belgeler ve nedeni (sessiz atlama yok).
    pub skipped: Vec<ScopeSkip>,
    /// Önizlemede uygulanacak maske özeti; maske verilmediyse `None`.
    /// **Terimler taşınmaz** — yalnız sayı ve plan kimliği.
    pub mask: Option<ConsentMaskPreview>,
}

/// Önizlemede uygulanacak maskenin özeti. Terim listesi burada da yazılmaz
/// (şartname §737: terimler kasaya girmez); yalnız ölçüm ve plan kimliği.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentMaskPreview {
    /// Doğrulanmış terim sayısı.
    pub term_count: i64,
    /// Kapsamda maskelenen parça sayısı. Bütçe aşılırsa yalnız **örnek**
    /// segment ölçülür (`bytes_exact=false` bunu söyler).
    pub masked_runs: i64,
    /// Kapsamda maskelenen özgün karakter sayısı (bütçe aşılırsa yalnız örnek).
    pub masked_chars: i64,
    /// Örnek segmentte en az bir eşleşme var mı (kullanıcı "maske uygulandı"
    /// sanmasın diye eşleşmesiz durum ayrıca yazılır).
    pub sample_masked: bool,
    /// `bytes` alanı maskeli metinden mi sayıldı? `false` ise bayt sayısı
    /// maskesiz **üst sınırdır** — "maskeli" diye sunulmaz.
    pub bytes_exact: bool,
    /// Karartma planı köprüsü kullanıldıysa plan kimliği (terim değil).
    pub plan_id: Option<String>,
}

/// Aktarım kapsamında gönderilecek tek segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSegment {
    pub segment_id: String,
    pub document_version_id: String,
    pub text: String,
}

/// Kapsama giren belge: güncel sürüm + o sürümün sayfa üreten en yeni koşusu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeDocument {
    pub document_id: String,
    pub document_version_id: String,
    pub segment_count: i64,
    pub bytes: i64,
}

/// Kapsam dışı kalan belge ve **nedeni**.
/// `missing` · `no_current_version` · `not_parsed` · `empty_text`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSkip {
    pub document_id: String,
    pub reason: String,
}

/// Kapsam çözümlemesinin ham sonucu (kapsam içi belgeler, kapsam dışı nedenler,
/// gönderilecek segmentler).
struct ResolvedScope {
    documents: Vec<ScopeDocument>,
    skipped: Vec<ScopeSkip>,
    segments: Vec<ScopeSegment>,
}

/// Onayın çözdüğü **gerçek** aktarım kapsamı.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentScope {
    pub consent_id: String,
    pub case_id: String,
    pub documents: Vec<ScopeDocument>,
    pub skipped: Vec<ScopeSkip>,
    pub segments: Vec<ScopeSegment>,
    pub bytes: i64,
}

impl Database {
    /// `api` tipi sağlayıcı profili kaydeder. **API anahtarı saklanmaz**;
    /// anahtarın kaynağı (OS güvenli deposu ya da `DIM_API_KEY`) profil
    /// künyesinde **iddia edilmez** — çalışma anında `dim_providers::secret`
    /// çözer ve `auth_mode_verified` olarak bildirir. Anahtar hiçbir
    /// tabloya/Denetim kaydına yazılmaz. Uç nokta `https://` olmalıdır.
    pub fn register_api_profile(
        &self,
        endpoint: &str,
        model: &str,
    ) -> Result<ProviderProfileRow, StorageError> {
        let endpoint = endpoint.trim();
        let model = model.trim();
        if !endpoint.starts_with("https://") || model.is_empty() {
            return Err(StorageError::InvalidSetting);
        }
        if let Some(existing) = self
            .list_provider_profiles()?
            .into_iter()
            .find(|p| p.provider_type == "api" && p.adapter_id == "openai-compatible")
        {
            // Aynı değerler: ikinci profil açılmaz (idempotent). **Farklı**
            // değerler: kayıt gerçekten güncellenir — eski davranış ikinci
            // kaydı sessizce yok sayıyordu, arayüz "Kaydedildi" yazarken
            // gönderim eski uç noktaya gidiyordu.
            let current = self.api_destination(&existing.id)?;
            if current.as_ref() == Some(&(endpoint.to_string(), model.to_string())) {
                return Ok(existing);
            }
            let capabilities = serde_json::json!({ "endpoint": endpoint, "model": model });
            self.conn().execute(
                "UPDATE provider_profiles
                 SET capabilities=?1, updated_at=?2, row_version=row_version+1
                 WHERE id=?3",
                rusqlite::params![capabilities.to_string(), now_iso(), existing.id],
            )?;
            self.append_audit(
                "provider_profile",
                Some(&existing.id),
                "update",
                "local-user",
                &serde_json::json!({"type": "api", "endpoint": endpoint, "model": model}),
            )?;
            return self
                .list_provider_profiles()?
                .into_iter()
                .find(|p| p.id == existing.id)
                .ok_or(StorageError::NotFound);
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        let capabilities = serde_json::json!({ "endpoint": endpoint, "model": model });
        self.conn().execute(
            "INSERT INTO provider_profiles (id,provider_type,adapter_id,execution_environment,auth_mode_requested,capabilities,policy_version,created_at,updated_at)
             VALUES (?1,'api','openai-compatible','remote','auto',?2,'1',?3,?3)",
            rusqlite::params![id, capabilities.to_string(), now],
        )?;
        self.append_audit(
            "provider_profile",
            Some(&id),
            "register",
            "local-user",
            &serde_json::json!({"type": "api", "endpoint": endpoint, "model": model}),
        )?;
        self.list_provider_profiles()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or(StorageError::NotFound)
    }

    /// Seçili **API** profilinin kayıtlı uç noktası ve modeli. Profil yoksa,
    /// `api` tipi değilse ya da kayıt eksikse `None`. Çağıran, içeriği hangi
    /// sağlayıcıya gönderdiğini **tahmin etmez**; seçili profilden okur.
    pub fn api_destination(
        &self,
        provider_id: &str,
    ) -> Result<Option<(String, String)>, StorageError> {
        let raw: Option<String> = self
            .conn()
            .query_row(
                "SELECT capabilities FROM provider_profiles WHERE id=?1 AND provider_type='api'",
                [provider_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(raw) = raw else { return Ok(None) };
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|_| StorageError::InvalidSetting)?;
        let endpoint = value.get("endpoint").and_then(|v| v.as_str());
        let model = value.get("model").and_then(|v| v.as_str());
        match (endpoint, model) {
            (Some(endpoint), Some(model)) if !endpoint.is_empty() && !model.is_empty() => {
                Ok(Some((endpoint.to_string(), model.to_string())))
            }
            _ => Ok(None),
        }
    }

    /// Aktarım kapsamını **tek** yerden çözer. Önizleme ile gönderim aynı
    /// işlevi kullanır; böylece "onaylanan içerik" ile "gönderilen içerik"
    /// ayrışamaz.
    ///
    /// Kural: belgenin **güncel** sürümü; o sürümün **sayfa üreten en yeni**
    /// ayrıştırma koşusu. Eski sürümün metni asla gönderilmez. Kapsam dışı
    /// kalan belge sessizce atlanmaz — nedeni `ScopeSkip` olarak döner
    /// (`missing`, `no_current_version`, `not_parsed`, `empty_text`).
    fn resolve_transfer_scope(
        &self,
        case_id: &str,
        document_ids: &[String],
    ) -> Result<ResolvedScope, StorageError> {
        let mut documents = Vec::new();
        let mut skipped = Vec::new();
        let mut segments = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for document_id in document_ids {
            if seen.contains(&document_id.as_str()) {
                continue;
            }
            seen.push(document_id.as_str());
            let version_id: Option<Option<String>> = self
                .conn()
                .query_row(
                    "SELECT current_version_id FROM documents
                     WHERE id=?1 AND case_id=?2 AND deleted_at IS NULL",
                    rusqlite::params![document_id, case_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?;
            let Some(version_id) = version_id else {
                skipped.push(ScopeSkip {
                    document_id: document_id.clone(),
                    reason: "missing".into(),
                });
                continue;
            };
            let Some(version_id) = version_id else {
                skipped.push(ScopeSkip {
                    document_id: document_id.clone(),
                    reason: "no_current_version".into(),
                });
                continue;
            };
            let run_id: Option<String> = self
                .conn()
                .query_row(
                    "SELECT pr.id FROM parse_runs pr
                     WHERE pr.document_version_id=?1
                       AND EXISTS (SELECT 1 FROM pages p WHERE p.parse_run_id=pr.id)
                     ORDER BY pr.created_at DESC, pr.id DESC LIMIT 1",
                    [&version_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(run_id) = run_id else {
                skipped.push(ScopeSkip {
                    document_id: document_id.clone(),
                    reason: "not_parsed".into(),
                });
                continue;
            };
            let mut statement = self.conn().prepare(
                "SELECT s.id, s.raw_text FROM text_segments s
                 JOIN pages p ON p.id = s.page_id
                 WHERE p.parse_run_id = ?1
                 ORDER BY p.page_index, s.rowid",
            )?;
            let rows = statement.query_map([&run_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut count = 0i64;
            let mut bytes = 0i64;
            let mut own = Vec::new();
            for row in rows {
                let (segment_id, text) = row?;
                count += 1;
                bytes += i64::try_from(text.len()).unwrap_or(i64::MAX);
                own.push(ScopeSegment {
                    segment_id,
                    document_version_id: version_id.clone(),
                    text,
                });
            }
            if count == 0 {
                skipped.push(ScopeSkip {
                    document_id: document_id.clone(),
                    reason: "empty_text".into(),
                });
                continue;
            }
            segments.extend(own);
            documents.push(ScopeDocument {
                document_id: document_id.clone(),
                document_version_id: version_id,
                segment_count: count,
                bytes,
            });
        }
        Ok(ResolvedScope {
            documents,
            skipped,
            segments,
        })
    }

    /// Onay kimliğinden çözülen **gerçek** aktarım kapsamı: gönderilecek
    /// segmentler onayın belge listesinden gelir, çağıranın verdiği listeden
    /// değil. İptal/süresi geçmiş onay ve başka dosyanın onayı reddedilir.
    pub fn consent_scope_segments(
        &self,
        case_id: &str,
        consent_id: &str,
    ) -> Result<ConsentScope, StorageError> {
        let consent = self
            .get_transfer_consent(consent_id)?
            .ok_or(StorageError::NotFound)?;
        if consent.case_scope != case_id || !self.consent_is_active(consent_id)? {
            return Err(StorageError::AiConsent);
        }
        let resolved = self.resolve_transfer_scope(case_id, &consent.selected_document_ids)?;
        let bytes = resolved
            .segments
            .iter()
            .map(|segment| i64::try_from(segment.text.len()).unwrap_or(i64::MAX))
            .sum();
        Ok(ConsentScope {
            consent_id: consent.id,
            case_id: case_id.to_string(),
            documents: resolved.documents,
            skipped: resolved.skipped,
            segments: resolved.segments,
            bytes,
        })
    }

    /// Aktarım onayından **önce** ne gönderileceğini özetler: belge, segment,
    /// bayt, kısa örnek ve **kapsam dışı kalan belgeler**. Gönderim yolu aynı
    /// çözümleyiciyi kullandığı için önizleme ile gönderilen içerik ayrışamaz.
    /// Tam metin arayüze dökülmez.
    ///
    /// **Maske:** `ai_run_execute` maskeyi uygulayıp **maskeli** metni
    /// gönderdiği için, maskesiz önizleme gönderilecek metni yanlış gösterirdi.
    /// `consent_preview_masked` gönderilecek metnin kendisini (maskeli örnek +
    /// maskeli bayt) döndürür; bu fonksiyon maske verilmeden çağırır.
    pub fn consent_preview(
        &self,
        case_id: &str,
        document_ids: &[String],
    ) -> Result<ConsentPreview, StorageError> {
        self.consent_preview_masked(case_id, document_ids, &[], None)
    }

    /// Maskeli önizleme: maske **doğrulanır**, örnek segment maskeli gösterilir
    /// ve bayt sayısı maskeli metinden sayılır. `plan` verilmişse (`(plan_id,
    /// document_version_id)`) plan bu kapsamdaki bir belgeye ait olmalıdır;
    /// değilse gönderim yoluyla **aynı** hata: `E_MASK_PLAN_SCOPE`.
    ///
    /// Terimler yalnız çağrı boyunca yaşar, kasaya yazılmaz.
    pub fn consent_preview_masked(
        &self,
        case_id: &str,
        document_ids: &[String],
        terms: &[String],
        plan: Option<(&str, &str)>,
    ) -> Result<ConsentPreview, StorageError> {
        self.consent_preview_with_budget(
            case_id,
            document_ids,
            terms,
            plan,
            PREVIEW_MASK_CHAR_BUDGET,
        )
    }

    /// Bütçe parametreli iç gerçekleme: önizleme tüm kapsamı maskelerken
    /// `mask_terms` maliyeti metin × terim olduğundan büyük kapsamda yalnız
    /// **örnek** maskelenir ve bayt sayısı maskesiz üst sınır olarak
    /// işaretlenir (`bytes_exact=false`). Sessizce "maskeli" denmez.
    fn consent_preview_with_budget(
        &self,
        case_id: &str,
        document_ids: &[String],
        terms: &[String],
        plan: Option<(&str, &str)>,
        budget: usize,
    ) -> Result<ConsentPreview, StorageError> {
        let terms = if terms.is_empty() {
            // Maske verilmedi: boş liste "geçersiz terim" değildir.
            Vec::new()
        } else {
            validate_terms(terms).map_err(mask_storage_error)?
        };
        let resolved = self.resolve_transfer_scope(case_id, document_ids)?;
        if let Some((_, version)) = plan {
            if !resolved
                .documents
                .iter()
                .any(|document| document.document_version_id == version)
            {
                return Err(StorageError::MaskRejected("E_MASK_PLAN_SCOPE"));
            }
        }
        let bytes_unmasked = resolved
            .segments
            .iter()
            .map(|segment| i64::try_from(segment.text.len()).unwrap_or(i64::MAX))
            .sum::<i64>();
        let mut bytes = bytes_unmasked;
        let mut sample = resolved
            .segments
            .first()
            .map(|segment| segment.text.chars().take(200).collect())
            .unwrap_or_default();
        let mut mask_summary = None;
        if !terms.is_empty() {
            let total_chars: usize = resolved
                .segments
                .iter()
                .map(|segment| segment.text.chars().count())
                .sum();
            let exact = total_chars <= budget;
            let mut masked_runs: i64 = 0;
            let mut masked_chars: i64 = 0;
            let mut masked_bytes: i64 = 0;
            let mut sample_masked = false;
            for (index, segment) in resolved.segments.iter().enumerate() {
                if !exact && index > 0 {
                    break;
                }
                let masked = mask_terms(&segment.text, &terms).map_err(mask_storage_error)?;
                masked_runs = masked_runs
                    .saturating_add(i64::try_from(masked.masked_runs).unwrap_or(i64::MAX));
                masked_chars = masked_chars
                    .saturating_add(i64::try_from(masked.masked_chars).unwrap_or(i64::MAX));
                masked_bytes = masked_bytes
                    .saturating_add(i64::try_from(masked.text.len()).unwrap_or(i64::MAX));
                if index == 0 {
                    sample_masked = masked.masked_runs > 0;
                    sample = masked.text.chars().take(200).collect();
                }
            }
            if exact {
                bytes = masked_bytes;
            }
            mask_summary = Some(ConsentMaskPreview {
                term_count: i64::try_from(terms.len()).unwrap_or(i64::MAX),
                masked_runs,
                masked_chars,
                sample_masked,
                bytes_exact: exact,
                plan_id: plan.map(|(plan_id, _)| plan_id.to_string()),
            });
        }
        Ok(ConsentPreview {
            documents: i64::try_from(resolved.documents.len()).unwrap_or(i64::MAX),
            segments: i64::try_from(resolved.segments.len()).unwrap_or(i64::MAX),
            bytes,
            bytes_unmasked,
            sample,
            skipped: resolved.skipped,
            mask: mask_summary,
        })
    }
}

fn map_run(row: &Row<'_>) -> rusqlite::Result<AiRunRow> {
    Ok(AiRunRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        task_type: row.get(2)?,
        provider_id: row.get(3)?,
        model_snapshot: row.get(4)?,
        input_manifest_hash: row.get(5)?,
        prompt_version: row.get(6)?,
        output_schema_version: row.get(7)?,
        consent_id: row.get(8)?,
        status: row.get(9)?,
        usage_report: row.get(10)?,
        created_at: row.get(11)?,
        completed_at: row.get(12)?,
    })
}

pub(crate) fn map_candidate(row: &Row<'_>) -> rusqlite::Result<AiCandidateRow> {
    let raw_quotes: String = row.get(8)?;
    Ok(AiCandidateRow {
        id: row.get(0)?,
        ai_run_id: row.get(1)?,
        candidate_type: row.get(2)?,
        structured_payload: row.get(3)?,
        validation_state: row.get(4)?,
        validation_errors: row.get(5)?,
        schema_state: row.get(6)?,
        quote_state: row.get(7)?,
        quote_report: serde_json::from_str(&raw_quotes).unwrap_or_default(),
        checked_at: row.get(9)?,
    })
}

/// Etkinlik kuralı **tek yerde**: liste (`list_transfer_consents`) ve çalıştırma
/// kapısı (`consent_is_active`) aynı fonksiyonu kullanır — arayüz ile çekirdek
/// ayrışamaz.
fn consent_active_at(consent: &TransferConsentRow, now: &str) -> bool {
    if consent.revoked_at.is_some() {
        return false;
    }
    match consent.expires_at.as_deref() {
        Some(expires) => expires > now,
        None => true,
    }
}

fn map_consent(row: &Row<'_>) -> rusqlite::Result<TransferConsentRow> {
    let ids: String = row.get(4)?;
    Ok(TransferConsentRow {
        id: row.get(0)?,
        provider_snapshot: row.get(1)?,
        case_scope: row.get(2)?,
        purpose: row.get(3)?,
        selected_document_ids: serde_json::from_str(&ids).unwrap_or_default(),
        expires_at: row.get(5)?,
        consent_text_version: row.get(6)?,
        approved_at: row.get(7)?,
        revoked_at: row.get(8)?,
        outbound_transform_id: row.get(9)?,
    })
}

/// Kimlik eşlemesi: gönderilen metin özgün metnin kendisidir.
fn identity_runs(len: usize) -> Vec<MaskRun> {
    if len == 0 {
        return Vec::new();
    }
    vec![MaskRun {
        out_start: 0,
        out_end: len,
        source_start: 0,
        source_end: len,
        masked: false,
    }]
}

/// Maske **tanımının** içerik adresi: terim listesi kanonik biçimde özetlenir
/// (terimlerin kendisi saklanmaz — maskenin amacı terimi kasadan çıkarmamak).
///
/// Tek kaynak: gönderim sicili (`outbound_transforms`), onay bağı
/// (`transfer_consents.outbound_transform_id`) ve çalıştırma kapısı **aynı**
/// işlevi kullanır. İki ayrı biçim yazılsaydı onay ile gönderim sessizce
/// ayrışırdı (kapı yeşilken içerik farklı çıkardı).
fn outbound_settings_hash(terms: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dim-outbound-transform-v1");
    hasher.update(MASK_PLACEHOLDER.as_bytes());
    for term in terms {
        hasher.update(b"\x1f");
        hasher.update(term.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Maskeyi **kanonik** biçime getirir (kırp, boşları at, doğrula, tekilleştir,
/// uzundan kısaya sırala).
///
/// Boş liste geçerlidir: "maske yok" bir maskesizlik kararıdır, geçersiz terim
/// değil (`validate_terms` boş listeyi reddeder — o yüzden bu sarmalayıcı var).
fn canonical_mask_terms(terms: &[String]) -> Result<Vec<String>, StorageError> {
    let cleaned: Vec<String> = terms
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .map(str::to_string)
        .collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }
    validate_terms(&cleaned).map_err(mask_storage_error)
}

/// Dönüşüm sürümünü **içerik adresli** olarak bulur/oluşturur. Aynı terim
/// listesi aynı satırı kullanır; terimlerin kendisi saklanmaz, yalnız özeti
/// (`settings_hash`) ve sayısı.
fn ensure_outbound_transform(
    tx: &rusqlite::Transaction<'_>,
    terms: &[String],
) -> Result<String, StorageError> {
    let settings_hash = outbound_settings_hash(terms);
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM outbound_transforms WHERE settings_hash=?1",
            [&settings_hash],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = uuid::Uuid::now_v7().to_string();
    tx.execute(
        "INSERT INTO outbound_transforms
           (id,kind,settings_hash,term_count,placeholder,created_at)
         VALUES (?1,'mask_terms',?2,?3,?4,?5)",
        rusqlite::params![
            id,
            settings_hash,
            i64::try_from(terms.len()).unwrap_or(i64::MAX),
            MASK_PLACEHOLDER,
            now_iso()
        ],
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::DatabaseKey;

    fn setup() -> (Database, String, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([47u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kira".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        db.conn()
            .execute(
                "INSERT INTO provider_profiles (id,provider_type,adapter_id,policy_version,created_at,updated_at)
                 VALUES ('p1','cli','claude-code','1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("provider");
        (db, case.id, "p1".to_string())
    }

    fn consent(db: &Database, case_id: &str) -> TransferConsentRow {
        let (document_id, _) = document_with_text(db, case_id, "Belge", &["Sözleşme metni"]);
        db.create_transfer_consent(&NewTransferConsent {
            provider_id: "p1".into(),
            case_scope: case_id.into(),
            purpose: "ozet".into(),
            selected_document_ids: vec![document_id],
            expires_at: None,
            consent_text_version: "1".into(),
            mask_terms: Vec::new(),
            mask_plan_id: None,
        })
        .expect("onay")
    }

    fn run(_case_id: &str, consent_id: &str) -> NewAiRun {
        NewAiRun {
            task_type: "summarize".into(),
            provider_id: "p1".into(),
            model_snapshot: "claude-sonnet".into(),
            input_manifest_hash: "h".into(),
            prompt_version: "p1".into(),
            output_schema_version: "1".into(),
            consent_id: consent_id.into(),
            mask_terms: Vec::new(),
        }
    }

    #[test]
    fn run_requires_active_consent() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let started = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        assert_eq!(started.status, "running");

        db.revoke_transfer_consent(&c.id).expect("iptal");
        let err = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .unwrap_err();
        assert_eq!(err.code(), "E_AI_CONSENT");
    }

    #[test]
    fn expired_consent_blocks_run() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let row = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: Some("2000-01-01T00:00:00Z".into()),
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        assert_eq!(
            db.start_ai_run(&case_id, &run(&case_id, &row.id))
                .unwrap_err()
                .code(),
            "E_AI_CONSENT"
        );
    }

    fn consent_for(
        db: &Database,
        case_id: &str,
        purpose: &str,
        documents: Vec<String>,
    ) -> TransferConsentRow {
        db.create_transfer_consent(&NewTransferConsent {
            provider_id: "p1".into(),
            case_scope: case_id.into(),
            purpose: purpose.into(),
            selected_document_ids: documents,
            expires_at: None,
            consent_text_version: "1".into(),
            mask_terms: Vec::new(),
            mask_plan_id: None,
        })
        .expect("onay")
    }

    /// Aynı amaç + aynı varış yeri yeniden onaylanınca **önceki yetki kapanır**:
    /// envanterde iki etkin kopya kalmaz, kapanma denetime yazılır.
    #[test]
    fn new_consent_supersedes_the_previous_one_for_the_same_purpose() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let first = consent_for(&db, &case_id, "ozet", vec![document_id.clone()]);
        assert!(db.consent_is_active(&first.id).expect("etkin"));

        let second = consent_for(&db, &case_id, "ozet", vec![document_id]);
        assert!(db.consent_is_active(&second.id).expect("yeni onay etkin"));
        assert!(
            !db.consent_is_active(&first.id).expect("eski onay"),
            "aynı amaç ve varış yeri yeniden onaylandı; eski yetki açık kalmamalı"
        );
        let old = db
            .get_transfer_consent(&first.id)
            .expect("kayıt")
            .expect("var");
        assert!(old.revoked_at.is_some());
        let audits: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM audit_events WHERE operation='supersede' AND entity_id=?1",
                [&first.id],
                |row| row.get(0),
            )
            .expect("denetim");
        assert_eq!(audits, 1, "kapanma denetime yazılmalı");
    }

    /// Varış yeri değişse bile **aynı amaç** yeniden onaylanınca eski yetki
    /// kapanır: iki etkin kopya kalmaz.
    #[test]
    fn reapproval_after_a_destination_change_also_supersedes() {
        let (db, case_id, _) = setup();
        let profile = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("profil");
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let first = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: profile.id.clone(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id.clone()],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("ilk onay");
        db.register_api_profile("https://b.example/v1", "model-b")
            .expect("uç nokta değişti");
        let second = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: profile.id,
                case_scope: case_id,
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("yeni onay");
        assert!(
            !db.consent_is_active(&first.id).expect("eski onay"),
            "aynı amaç yeniden onaylandı; varış yeri değişse de eski kapanmalı"
        );
        assert!(db.consent_is_active(&second.id).expect("yeni onay"));
    }

    /// Kapanmış onay yeniden kapatılmaz: iptal ve süre kayıtları yerinde kalır
    /// (envanterde "iptal edildi" ile "süresi geçti" ayrımı korunur).
    #[test]
    fn supersede_only_touches_live_consents() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let revoked = consent_for(&db, &case_id, "ozet", vec![document_id.clone()]);
        db.revoke_transfer_consent(&revoked.id).expect("iptal");
        let expired = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id.clone()],
                expires_at: Some("2000-01-01T00:00:00Z".into()),
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("süresi geçmiş onay");

        let live = consent_for(&db, &case_id, "ozet", vec![document_id]);
        assert!(db.consent_is_active(&live.id).expect("yeni onay"));

        let audits: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM audit_events WHERE operation='supersede' AND entity_id=?1",
                [&revoked.id],
                |row| row.get(0),
            )
            .expect("denetim");
        assert_eq!(audits, 0, "zaten iptal edilmiş onay yeniden kapatılmaz");
        let expired_row = db
            .get_transfer_consent(&expired.id)
            .expect("kayıt")
            .expect("var");
        assert!(
            expired_row.revoked_at.is_none(),
            "süresi geçmiş onay 'iptal edildi' işaretlenmez"
        );
    }

    /// Farklı amaç ya da farklı dosya **ayrı yetkidir**: yeni onay onları kapatmaz.
    #[test]
    fn consent_for_another_purpose_or_case_is_not_superseded() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let summary = consent_for(&db, &case_id, "ozet", vec![document_id.clone()]);
        let claims = consent_for(&db, &case_id, "iddia_cikarimi", vec![document_id]);
        assert!(
            db.consent_is_active(&summary.id).expect("özet"),
            "farklı amaç ayrı yetkidir"
        );
        assert!(db.consent_is_active(&claims.id).expect("iddia"));

        let other_case = case_titled(&db, "Başka dosya");
        let (other_doc, _) = document_with_text(&db, &other_case, "Belge", &["Metin"]);
        let other = consent_for(&db, &other_case, "ozet", vec![other_doc]);
        assert!(
            db.consent_is_active(&summary.id).expect("özet"),
            "başka dosyanın onayı bu dosyanın yetkisini kapatmaz"
        );
        assert!(db.consent_is_active(&other.id).expect("başka dosya"));
    }

    /// Envanter **dosyaya kapsanır**, iptal/süre durumunu çekirdekten yazar ve
    /// en yeni onayı önce döndürür.
    #[test]
    fn consent_inventory_is_case_scoped_and_reports_state() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let active = consent_for(&db, &case_id, "ozet", vec![document_id.clone()]);
        let revoked = consent_for(&db, &case_id, "iddia_cikarimi", vec![document_id.clone()]);
        db.revoke_transfer_consent(&revoked.id).expect("iptal");
        let expired = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "olay_cikarimi".into(),
                selected_document_ids: vec![document_id],
                expires_at: Some("2000-01-01T00:00:00Z".into()),
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("süresi geçmiş onay");

        let other_case = case_titled(&db, "Başka dosya");
        let (other_doc, _) = document_with_text(&db, &other_case, "Belge", &["Metin"]);
        consent_for(&db, &other_case, "ozet", vec![other_doc]);

        let rows = db.list_transfer_consents(&case_id).expect("envanter");
        assert_eq!(rows.len(), 3, "yalnız bu dosyanın onayları");
        let row = |id: &str| {
            rows.iter()
                .find(|item| item.consent.id == id)
                .expect("satır")
        };
        assert!(row(&active.id).active, "iptal edilmemiş, süresi dolmamış");
        assert!(!row(&revoked.id).active);
        assert!(row(&revoked.id).consent.revoked_at.is_some());
        assert!(!row(&expired.id).active, "süre doldu");
        assert!(
            row(&expired.id).consent.revoked_at.is_none(),
            "süre dolması iptal değildir"
        );
        assert!(rows.iter().all(|item| item.consent.case_scope == case_id));
        let keys: Vec<(String, String)> = rows
            .iter()
            .map(|item| (item.consent.approved_at.clone(), item.consent.id.clone()))
            .collect();
        let mut expected = keys.clone();
        expected.sort();
        expected.reverse();
        assert_eq!(keys, expected, "en yeni onay önce");
        assert!(db
            .list_transfer_consents("olmayan-dosya")
            .expect("boş envanter")
            .is_empty());
    }

    /// **Ölçülen kusur:** sağlayıcının uç noktası/modeli değişince onay
    /// yürütülemez hâle geliyordu ama envanter yalnız iptal/süreye baktığı için
    /// satır "etkin" kalıyordu. "Kayıt yürürlükte" ile "yürütülebilir" ayrı
    /// yazılmalı: `active` doğru kalır, `provider_current` düşer.
    #[test]
    fn consent_inventory_reports_provider_drift() {
        let (db, case_id, _) = setup();
        let profile = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("profil");
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: profile.id.clone(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");

        let rows = db.list_transfer_consents(&case_id).expect("envanter");
        assert!(rows[0].active);
        assert!(
            rows[0].provider_current,
            "onay kendi varış yeriyle eşleşiyor"
        );

        db.register_api_profile("https://b.example/v1", "model-a")
            .expect("uç nokta değişti");
        let rows = db.list_transfer_consents(&case_id).expect("envanter");
        assert!(rows[0].active, "iptal/süre yok: kayıt yürürlükte");
        assert!(
            !rows[0].provider_current,
            "varış yeri artık yürürlükteki profille aynı değil"
        );

        // Envanterin söylediği ile çalıştırma kapısı **aynı** olguyu anlatır.
        let mut approved = run(&case_id, &c.id);
        approved.provider_id = profile.id.clone();
        assert_eq!(
            db.start_ai_run(&case_id, &approved).unwrap_err().code(),
            "E_AI_PROVIDER_CHANGED",
            "onay var, varış yeri değişti: ayrı kod"
        );
    }

    /// Aşırı geçersizleştirme de bir yalandır: başka profilin değişmesi,
    /// kendi profilinde duran onayı "yenilenmeli" göstermemeli.
    #[test]
    fn provider_drift_is_scoped_to_the_consent_provider() {
        let (db, case_id, _) = setup();
        let _c = consent(&db, &case_id); // p1: cli/claude-code
        db.register_api_profile("https://a.example/v1", "model-a")
            .expect("api profili");
        assert!(
            db.list_transfer_consents(&case_id).expect("envanter")[0].provider_current,
            "cli profili duruyor: onay hâlâ yürütülebilir"
        );
        db.register_api_profile("https://b.example/v1", "model-b")
            .expect("api değişti");
        assert!(
            db.list_transfer_consents(&case_id).expect("envanter")[0].provider_current,
            "başka profilin değişimi bu onayı geçersiz kılmaz"
        );
    }

    /// Profil satırı **varış yerini** taşır: ekran onayın özetiyle
    /// karşılaştırabilsin diye aynı kuraldan türetilir.
    #[test]
    fn provider_profile_rows_carry_their_destination_snapshot() {
        let (db, case_id, _) = setup();
        let profile = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("profil");
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: profile.id.clone(),
                case_scope: case_id,
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let rows = db.list_provider_profiles().expect("profiller");
        let api = rows.iter().find(|p| p.id == profile.id).expect("api");
        assert_eq!(
            api.provider_snapshot, c.provider_snapshot,
            "profil özeti onayınkiyle aynı kuraldan türetilir"
        );
        assert!(
            api.provider_snapshot.contains('#'),
            "uç nokta + model parmak izi özete girer"
        );
        let cli = rows.iter().find(|p| p.id == "p1").expect("cli");
        assert_eq!(
            cli.provider_snapshot, "claude-code@1",
            "capabilities yok: adapter@politika"
        );
    }

    #[test]
    fn consent_is_case_scoped() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let other = db
            .create_case(&NewCase {
                title: "Başka".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        assert_eq!(
            db.start_ai_run(&other.id, &run(&other.id, &c.id))
                .unwrap_err()
                .code(),
            "E_AI_CONSENT"
        );
    }

    /// Onay sağlayıcıya bağlıdır: farklı profille (yeni uç nokta/model) aynı onay
    /// kullanılamaz; izin kendiliğinden genişlemez.
    #[test]
    fn consent_is_provider_scoped() {
        let (db, case_id, _) = setup();
        db.conn()
            .execute(
                "INSERT INTO provider_profiles (id,provider_type,adapter_id,policy_version,created_at,updated_at)
                 VALUES ('p2','api','openai-compatible','1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
                [],
            )
            .expect("ikinci sağlayıcı");
        let c = consent(&db, &case_id);
        // Aynı sağlayıcı: geçer.
        db.start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("aynı sağlayıcı");
        // Başka sağlayıcı: onay kapsamaz — ve bu "onay yok" değil, "varış yeri
        // değişti"dir; ayrı kod döner (ekran yenileme söyleyebilsin diye).
        let mut other = run(&case_id, &c.id);
        other.provider_id = "p2".into();
        assert_eq!(
            db.start_ai_run(&case_id, &other).unwrap_err().code(),
            "E_AI_PROVIDER_CHANGED"
        );
        // Hiç olmayan sağlayıcı: ayrı kod.
        let mut missing = run(&case_id, &c.id);
        missing.provider_id = "yok".into();
        assert_eq!(
            db.start_ai_run(&case_id, &missing).unwrap_err().code(),
            "E_AI_PROVIDER"
        );
    }

    #[test]
    fn unknown_provider_rejected() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let mut r = run(&case_id, &c.id);
        r.provider_id = "yok".into();
        assert_eq!(
            db.start_ai_run(&case_id, &r).unwrap_err().code(),
            "E_AI_PROVIDER"
        );
    }

    #[test]
    fn candidate_promotes_to_pending_review_item() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let candidate = db
            .record_ai_candidate(
                &r.id,
                "event_candidate",
                "{\"schemaVersion\":1,\"title\":\"Ödeme\"}",
            )
            .expect("aday");
        assert_eq!(candidate.validation_state, "partial");
        assert_eq!(candidate.schema_state, "valid");
        assert_eq!(candidate.quote_state, "no_quotes");
        assert_eq!(db.list_ai_candidates(&r.id).expect("liste").len(), 1);
        let review_id = db
            .promote_candidate(&candidate.id, &case_id)
            .expect("terfi");
        let item = db.get_review_item(&review_id).expect("get").expect("var");
        assert_eq!(item.state, "pending");
        assert_eq!(item.item_kind, "event_candidate");
    }

    #[test]
    fn coverage_report_counts_empty_pages() {
        let (db, case_id, _) = setup();
        let report = db.coverage_report(&case_id).expect("rapor");
        assert_eq!(report.documents, 0);
        assert_eq!(report.empty_pages, 0);
    }

    /// Kapsam raporu **gönderilebilir korpusu** anlatır: aynı sürümün ikinci
    /// ayrıştırması sayfa sayısını ikiye katlamaz, yalnız en yeni sayfa üreten
    /// koşu sayılır (gönderim kapsamıyla aynı kural).
    #[test]
    fn coverage_report_counts_the_corpus_an_ai_run_would_see() {
        let (db, case_id, _) = setup();
        let (_doc, version) =
            document_with_text(&db, &case_id, "Dilekçe", &["birinci sayfa", "ikinci sayfa"]);
        let run = db
            .start_parse_run(&version, &case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("koşu");
        db.store_pages(
            &run.id,
            &case_id,
            &[page(0, "bir"), page(1, "iki"), page(2, "üç")],
        )
        .expect("sayfalar");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("bitir");
        let report = db.coverage_report(&case_id).expect("rapor");
        assert_eq!(report.documents, 1);
        assert_eq!(report.parse_runs, 2, "koşu sayısı tarihçedir");
        assert_eq!(report.pages, 3, "yalnız en yeni sayfa üreten koşu");
        assert_eq!(report.segments, 3);
        assert_eq!(report.empty_pages, 0);
    }

    /// Güncel sürüm ayrıştırılmadıysa belge **ayrıştırılmamış** sayılır: eski
    /// sürümün metni gönderim kapsamına girmez, kapsam raporu da onu saymaz.
    #[test]
    fn coverage_report_counts_a_document_whose_current_version_is_unparsed() {
        let (db, case_id, _) = setup();
        let (document_id, _v1) = document_with_text(&db, &case_id, "Dilekçe", &["eski metin"]);
        let _v2 = unparsed_version(&db, &case_id, &document_id);
        let report = db.coverage_report(&case_id).expect("rapor");
        assert_eq!(report.documents, 1);
        assert_eq!(report.unparsed_documents, 1, "güncel sürüm taranmadı");
        assert_eq!(report.pages, 0, "eski sürümün sayfaları sayılmaz");
        assert_eq!(report.segments, 0);
        assert_eq!(report.empty_pages, 0);
    }

    /// Ayrıştırma koşusu olup hiçbir sayfasında kullanılabilir metin olmayan
    /// belge "metinli" sayılmaz: OCR'sız taranmış belge gönderilemez.
    #[test]
    fn coverage_report_counts_documents_without_usable_text() {
        let (db, case_id, _) = setup();
        document_with_text(&db, &case_id, "Taranmış", &["", "   "]);
        let report = db.coverage_report(&case_id).expect("rapor");
        assert_eq!(report.documents, 1);
        assert_eq!(report.unparsed_documents, 0, "koşu var, metin yok");
        assert_eq!(report.pages, 2);
        assert_eq!(report.empty_pages, 2);
        assert_eq!(report.documents_without_text, 1);
    }

    /// "Metin var" ölçütü alıntı denetimiyle **aynı**dır: yalnız boşluk taşıyan
    /// segment satırı metin sayılmaz (iki tanım ayrışırsa kapsam "metinli"
    /// derken alıntı taraması sayfayı atlardı).
    #[test]
    fn coverage_report_treats_a_whitespace_only_segment_as_no_text() {
        let (db, case_id, _) = setup();
        let (_doc, version) = document_with_text(&db, &case_id, "Boşluk", &[""]);
        let run: String = db
            .conn()
            .query_row(
                "SELECT id FROM parse_runs WHERE document_version_id=?1",
                [&version],
                |row| row.get(0),
            )
            .expect("koşu");
        let page_id: String = db
            .conn()
            .query_row(
                "SELECT id FROM pages WHERE parse_run_id=?1",
                [&run],
                |row| row.get(0),
            )
            .expect("sayfa");
        // Doğrudan yazım (store_pages boş metni zaten yazmaz): yalnız boşluk
        // taşıyan bir segment satırı, "metin var" sayılmamalı.
        db.conn()
            .execute(
                "INSERT INTO text_segments
                 (id,parse_run_id,page_id,case_id,structural_path,raw_text,normalized_text,offset_map,bbox_list,created_at)
                 VALUES (?1,?2,?3,?4,'page/0',?5,?5,'[]','[]',?6)",
                rusqlite::params![
                    uuid::Uuid::now_v7().to_string(),
                    &run,
                    &page_id,
                    &case_id,
                    "   ",
                    crate::now_iso(),
                ],
            )
            .expect("segment");
        let report = db.coverage_report(&case_id).expect("rapor");
        assert_eq!(report.pages, 1);
        assert_eq!(report.empty_pages, 1, "boşluk metin değildir");
        assert_eq!(report.documents_without_text, 1);
        assert_eq!(report.segments, 1, "segment satırı yine sayılır");
    }

    /// Tek kural kanıtı: kapsam raporunun "metni var" saydığı belge sayısı,
    /// aktarım kapsamının gerçekten kabul ettiği belge sayısına eşittir.
    #[test]
    fn coverage_report_agrees_with_the_transfer_scope() {
        let (db, case_id, _) = setup();
        let (ok, _) = document_with_text(&db, &case_id, "Tam", &["alıntı metni"]);
        let (stale, _) = document_with_text(&db, &case_id, "Eski", &["başka metin"]);
        let _v2 = unparsed_version(&db, &case_id, &stale);
        let (blank, _) = document_with_text(&db, &case_id, "Taranmış", &[""]);
        let ids = vec![ok, stale, blank];
        let scope = db
            .resolve_transfer_scope(&case_id, &ids)
            .expect("gönderim kapsamı");
        assert_eq!(scope.documents.len(), 1, "yalnız 'Tam' gönderilebilir");
        assert_eq!(scope.skipped.len(), 2);
        let report = db.coverage_report(&case_id).expect("rapor");
        assert_eq!(
            report.documents - report.unparsed_documents - report.documents_without_text,
            i64::try_from(scope.documents.len()).unwrap_or(i64::MAX),
            "kapsam raporu ile gönderim kapsamı aynı dili konuşur"
        );
        assert_eq!(report.pages, 2, "gönderilebilir korpusun sayfaları");
        assert_eq!(report.empty_pages, 1, "taranmış belgenin sayfası metinsiz");
        assert_eq!(report.segments, 1);
    }

    #[test]
    fn api_profile_requires_https_and_is_idempotent() {
        let (db, _case_id, _provider) = setup();
        assert_eq!(
            db.register_api_profile("http://example.com", "model")
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
        assert_eq!(
            db.register_api_profile("https://example.com", "  ")
                .unwrap_err()
                .code(),
            "E_DB_SETTING"
        );
        let first = db
            .register_api_profile("https://example.com/v1", "ornek-model")
            .expect("profil");
        assert_eq!(first.provider_type, "api");
        let again = db
            .register_api_profile("https://example.com/v1", "ornek-model")
            .expect("tekrar");
        assert_eq!(first.id, again.id, "aynı profil ikinci kez eklenmemeli");
        let profiles = db.list_provider_profiles().expect("liste");
        assert_eq!(
            profiles.iter().filter(|p| p.provider_type == "api").count(),
            1
        );
    }

    /// Profil künyesi anahtarın **kaynağını iddia etmez**: anahtar OS güvenli
    /// deposundan da gelebilir. Sabit `auth: env:DIM_API_KEY` yazmak, zincir
    /// kullanıldığında Denetim kaydında yanlış beyan olurdu.
    #[test]
    fn api_profile_does_not_claim_the_key_source() {
        let (db, _case_id, _provider) = setup();
        db.register_api_profile("https://example.com/v1", "ornek-model")
            .expect("profil");
        let raw: String = db
            .conn()
            .query_row(
                "SELECT capabilities FROM provider_profiles WHERE provider_type='api'",
                [],
                |row| row.get(0),
            )
            .expect("kayıt");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert!(value.get("auth").is_none(), "{raw}");
        assert_eq!(value["endpoint"], "https://example.com/v1");
        assert_eq!(value["model"], "ornek-model");
        // Ters yön: uç nokta/model **gerçekten** yazılı (iddiasızlık bilgi
        // kaybı değil).
        assert_eq!(
            db.api_destination(
                &db.list_provider_profiles()
                    .expect("liste")
                    .into_iter()
                    .find(|p| p.provider_type == "api")
                    .expect("profil")
                    .id
            )
            .expect("varış"),
            Some((
                "https://example.com/v1".to_string(),
                "ornek-model".to_string()
            ))
        );
        let requested: String = db
            .conn()
            .query_row(
                "SELECT auth_mode_requested FROM provider_profiles WHERE provider_type='api'",
                [],
                |row| row.get(0),
            )
            .expect("kip");
        assert_eq!(requested, "auto");
    }

    /// API profili **güncellenebilir** olmalı: kullanıcı uç noktayı/modeli
    /// değiştirdiğinde çekirdek gerçekten yazmalı. Eski davranış ikinci kaydı
    /// sessizce yok sayıyordu (arayüz "Kaydedildi" yazarken gönderim eski
    /// uç noktaya gidiyordu).
    #[test]
    fn api_profile_update_replaces_destination() {
        let (db, _case_id, _provider) = setup();
        let first = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("profil");
        let same = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("tekrar");
        assert_eq!(first.id, same.id, "aynı değerler ikinci profil açmamalı");
        assert_eq!(
            db.api_destination(&first.id).expect("varış"),
            Some(("https://a.example/v1".to_string(), "model-a".to_string()))
        );
        let updated = db
            .register_api_profile("https://b.example/v1", "model-b")
            .expect("güncelle");
        assert_eq!(updated.id, first.id, "profil çoğalmaz");
        assert_eq!(
            db.api_destination(&first.id).expect("varış"),
            Some(("https://b.example/v1".to_string(), "model-b".to_string())),
            "kaydedilen uç nokta/model gerçekten değişmeli"
        );
    }

    /// Varış yeri değişince **eski onay geçersizdir**: aynı adapter sürümüyle
    /// başka uç noktaya gönderim, önceki onayla yapılamaz.
    #[test]
    fn api_endpoint_change_invalidates_old_consent() {
        let (db, case_id, _provider) = setup();
        let profile = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("profil");
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: profile.id.clone(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let mut approved = run(&case_id, &c.id);
        approved.provider_id = profile.id.clone();
        db.start_ai_run(&case_id, &approved)
            .expect("aynı varış yeri");
        db.register_api_profile("https://b.example/v1", "model-a")
            .expect("uç nokta değişti");
        assert_eq!(
            db.start_ai_run(&case_id, &approved).unwrap_err().code(),
            "E_AI_PROVIDER_CHANGED",
            "uç nokta değişti; eski onay yeni varış yerini kapsamaz"
        );
    }

    /// Model de varış yerinin parçasıdır: aynı uç noktada başka model eski
    /// onayla kullanılamaz.
    #[test]
    fn api_model_change_invalidates_old_consent() {
        let (db, case_id, _provider) = setup();
        let profile = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("profil");
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: profile.id.clone(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let mut approved = run(&case_id, &c.id);
        approved.provider_id = profile.id.clone();
        db.start_ai_run(&case_id, &approved)
            .expect("aynı varış yeri");
        db.register_api_profile("https://a.example/v1", "model-b")
            .expect("model değişti");
        assert_eq!(
            db.start_ai_run(&case_id, &approved).unwrap_err().code(),
            "E_AI_PROVIDER_CHANGED",
            "model değişti; eski onay yeni varış yerini kapsamaz"
        );
    }

    /// Onay yalnız **var olan** sağlayıcı profiline bağlanabilir; çağıranın
    /// uydurduğu özet dizesi kabul edilmez.
    #[test]
    fn consent_requires_a_known_provider() {
        let (db, case_id, _provider) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let err = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "uydurma".into(),
                case_scope: case_id,
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect_err("bilinmeyen sağlayıcı");
        assert_eq!(err.code(), "E_AI_PROVIDER");
    }

    /// AI kapalıyken aktarım onayı alınamaz: "hiçbir yere gönderme" onayı bir
    /// yetki değildir.
    #[test]
    fn consent_for_off_provider_is_rejected() {
        let (db, case_id, _provider) = setup();
        let off = db.ensure_off_provider().expect("off profili");
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let err = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: off.id,
                case_scope: case_id,
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect_err("AI kapalı");
        assert_eq!(err.code(), "E_AI_OFF");
    }

    /// Varış yeri **seçili profile** bağlıdır: CLI profili API varış yeri
    /// sayılmaz; gönderim yolu "ilk api profili"ni tahmin etmez.
    #[test]
    fn api_destination_is_profile_scoped() {
        let (db, _case_id, cli_provider) = setup();
        let api = db
            .register_api_profile("https://a.example/v1", "model-a")
            .expect("profil");
        assert_eq!(
            db.api_destination(&api.id).expect("api varış"),
            Some(("https://a.example/v1".to_string(), "model-a".to_string()))
        );
        assert_eq!(
            db.api_destination(&cli_provider).expect("cli varış"),
            None,
            "CLI profili API varış yeri değildir"
        );
        assert_eq!(
            db.api_destination("olmayan-profil").expect("yok"),
            None,
            "olmayan profil varış yeri üretmez"
        );
    }

    #[test]
    fn consent_preview_is_zero_without_documents() {
        let (db, case_id, _) = setup();
        let preview = db.consent_preview(&case_id, &[]).expect("önizleme");
        assert_eq!(preview.documents, 0);
        assert_eq!(preview.segments, 0);
        assert_eq!(preview.bytes, 0);
        assert_eq!(preview.bytes_unmasked, 0);
        assert!(preview.sample.is_empty());
        assert!(preview.mask.is_none(), "maske verilmediyse özet yok");

        let missing = db
            .consent_preview(&case_id, &["olmayan-belge".to_string()])
            .expect("önizleme");
        assert_eq!(missing.documents, 0);
    }

    /// Önizleme **gönderilecek metni** göstermeli: maskeli örnek + maskeli bayt.
    /// En güçlü kanıt, önizlemenin gerçek gönderimle birebir aynı olmasıdır.
    #[test]
    fn preview_with_mask_matches_what_is_sent() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(
            &db,
            &case_id,
            "Belge",
            &["Ahmet Yılmaz kira sözleşmesini imzaladı"],
        );
        let terms = vec!["Ahmet Yılmaz".to_string()];
        let preview = db
            .consent_preview_masked(&case_id, std::slice::from_ref(&document_id), &terms, None)
            .expect("önizleme");
        let mask = preview.mask.as_ref().expect("maske özeti");
        assert_eq!(mask.term_count, 1);
        assert!(mask.sample_masked, "örnekte eşleşme var");
        assert!(mask.bytes_exact, "küçük kapsam birebir sayılır");
        assert!(
            preview.bytes < preview.bytes_unmasked,
            "maske bayt sayısını küçültür"
        );
        assert_eq!(
            preview.bytes_unmasked,
            i64::try_from("Ahmet Yılmaz kira sözleşmesini imzaladı".len()).unwrap_or(i64::MAX),
            "maskesiz bayt ham metinden"
        );
        assert!(preview.sample.contains("[MASKED]"), "örnek maskeli");
        assert!(
            !preview.sample.contains("Ahmet Yılmaz"),
            "önizleme adı göstermez"
        );

        // Gönderim yolu aynı maskeyi uygular: bayt ve örnek birebir aynı olmalı.
        let consent = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id.clone()],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let started = db
            .start_ai_run(&case_id, &run(&case_id, &consent.id))
            .expect("çalıştır");
        let scope = db
            .consent_scope_segments(&case_id, &consent.id)
            .expect("kapsam");
        let refs: Vec<OutboundSegmentRef> = scope
            .segments
            .iter()
            .map(|segment| OutboundSegmentRef {
                segment_id: segment.segment_id.clone(),
                document_version_id: segment.document_version_id.clone(),
            })
            .collect();
        let sent = db
            .record_outbound_segments(&started.id, &refs, Some(&OutboundMask { terms }))
            .expect("gönderim sicili");
        let sent_bytes: usize = sent.segments.iter().map(|segment| segment.text.len()).sum();
        assert_eq!(
            preview.bytes,
            i64::try_from(sent_bytes).unwrap_or(i64::MAX),
            "önizleme baytı gönderilen baytla birebir"
        );
        let sent_sample: String = sent.segments[0].text.chars().take(200).collect();
        assert_eq!(
            preview.sample, sent_sample,
            "önizleme örneği gönderilen metin"
        );
    }

    /// Maske **verilmediyse** davranış değişmez: örnek ham, bayt maskesiz.
    #[test]
    fn preview_without_mask_shows_raw_text() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(
            &db,
            &case_id,
            "Belge",
            &["Ahmet Yılmaz kira sözleşmesini imzaladı"],
        );
        let preview = db
            .consent_preview(&case_id, &[document_id])
            .expect("önizleme");
        assert!(preview.mask.is_none());
        assert_eq!(preview.bytes, preview.bytes_unmasked);
        assert!(preview.sample.contains("Ahmet Yılmaz"));
    }

    /// Eşleşme yoksa "maske uygulandı" denmez: sayaç sıfır, bayt değişmez.
    #[test]
    fn preview_with_mask_reports_no_match() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["kira sözleşmesi"]);
        let preview = db
            .consent_preview_masked(&case_id, &[document_id], &["Kemal".to_string()], None)
            .expect("önizleme");
        let mask = preview.mask.expect("maske özeti");
        assert!(!mask.sample_masked);
        assert_eq!(mask.masked_runs, 0);
        assert_eq!(mask.masked_chars, 0);
        assert_eq!(preview.bytes, preview.bytes_unmasked);
    }

    /// Plan köprüsü: plan bu kapsamdaki bir belgeye ait değilse gönderim
    /// yoluyla **aynı** hata (`E_MASK_PLAN_SCOPE`) — önizleme sessiz geçmez.
    #[test]
    fn preview_rejects_plan_out_of_scope() {
        let (db, case_id, _) = setup();
        let (document_id, version_id) =
            document_with_text(&db, &case_id, "Belge", &["Ahmet Yılmaz imzaladı"]);
        let (_other_document, other_version) =
            document_with_text(&db, &case_id, "Diğer", &["başka metin"]);
        let terms = vec!["Ahmet Yılmaz".to_string()];
        let error = db
            .consent_preview_masked(
                &case_id,
                std::slice::from_ref(&document_id),
                &terms,
                Some(("plan-1", &other_version)),
            )
            .expect_err("kapsam dışı plan");
        assert_eq!(error.code(), "E_MASK_PLAN_SCOPE");

        let inside = db
            .consent_preview_masked(
                &case_id,
                &[document_id],
                &terms,
                Some(("plan-1", &version_id)),
            )
            .expect("kapsam içi plan");
        assert_eq!(
            inside.mask.expect("maske özeti").plan_id.as_deref(),
            Some("plan-1")
        );
    }

    /// Büyük kapsamda yalnız örnek maskelenir; bayt sayısı **maskesiz üst
    /// sınır** olarak işaretlenir (sessizce "maskeli" denmez).
    #[test]
    fn preview_budget_marks_bytes_as_upper_bound() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(
            &db,
            &case_id,
            "Belge",
            &["Ahmet Yılmaz kira sözleşmesini imzaladı"],
        );
        let preview = db
            .consent_preview_with_budget(
                &case_id,
                &[document_id],
                &["Ahmet Yılmaz".to_string()],
                None,
                1,
            )
            .expect("önizleme");
        let mask = preview.mask.expect("maske özeti");
        assert!(!mask.bytes_exact, "bütçe aşıldı");
        assert!(mask.sample_masked, "örnek yine maskelenir");
        assert_eq!(preview.bytes, preview.bytes_unmasked);
    }

    /// Geçersiz terim önizlemede de reddedilir (gönderimle aynı kapı).
    #[test]
    fn preview_rejects_invalid_terms() {
        let (db, case_id, _) = setup();
        let error = db
            .consent_preview_masked(&case_id, &[], &["a".to_string()], None)
            .expect_err("kısa terim");
        assert_eq!(error.code(), "E_MASK_TERM");
    }

    #[test]
    fn context_items_and_receipt_are_recorded() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let items = vec![NewAiContextItem {
            document_version_id: "v1".into(),
            source_anchor_id: None,
            inclusion_reason: "seçili paragraf".into(),
            bytes_sent: 42,
            segment_ids: Vec::new(),
        }];
        assert_eq!(db.record_context_items(&r.id, &items).expect("bağlam"), 1);
        assert_eq!(db.list_context_items(&r.id).expect("liste").len(), 1);
        let receipt = db
            .begin_transfer_receipt(&r.id, "claude-code", &"a".repeat(64), 1, 42)
            .expect("makbuz");
        db.finish_transfer_receipt(&receipt, "dispatched")
            .expect("makbuz kapandı");
        db.finish_ai_run(&r.id, "succeeded", "{\"billing\":\"unknown\"}")
            .expect("bitir");
        let finished = db.get_ai_run(&r.id).expect("get").expect("var");
        assert_eq!(finished.status, "succeeded");
    }

    /// Denetim sicili **okunabilir** olmalı: satır yalnız sürüm kimliği değil,
    /// belgenin adını ve sürüm numarasını taşır. Sıra **kayıt sırasıdır** —
    /// alfabetik ya da damga sırası değil.
    #[test]
    fn context_items_are_readable_with_title_and_version_in_record_order() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let (z_doc, z_version) = document_with_text(&db, &case_id, "Zabıt", &["ZABIT METNİ"]);
        let (a_doc, a_version) = document_with_text(&db, &case_id, "Ara karar", &["ARA KARAR"]);
        assert_ne!(z_doc, a_doc);
        let items = vec![
            NewAiContextItem {
                document_version_id: z_version.clone(),
                source_anchor_id: None,
                inclusion_reason: "onaylanan belge".into(),
                bytes_sent: 120,
                segment_ids: Vec::new(),
            },
            NewAiContextItem {
                document_version_id: a_version.clone(),
                source_anchor_id: None,
                inclusion_reason: "onaylanan belge".into(),
                bytes_sent: 64,
                segment_ids: Vec::new(),
            },
        ];
        assert_eq!(db.record_context_items(&r.id, &items).expect("bağlam"), 2);

        let listed = db.list_context_items(&r.id).expect("liste");
        assert_eq!(listed.len(), 2);
        // Alfabetik değil: kayıt sırası (Zabıt önce yazıldı).
        assert_eq!(listed[0].document_title.as_deref(), Some("Zabıt"));
        assert_eq!(listed[0].version_no, Some(1));
        assert_eq!(listed[0].bytes_sent, 120);
        assert_eq!(listed[1].document_title.as_deref(), Some("Ara karar"));
        assert_eq!(listed[1].version_no, Some(1));
        assert_eq!(listed[1].document_version_id, a_version);
    }

    /// Belge/sürüm silinse de **denetim satırı düşmez**; ad alanı boş kalır.
    /// Kimlik dururken ad uydurulmaz — ekran kimliği yazar.
    #[test]
    fn context_item_without_document_keeps_the_row_without_inventing_a_name() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        db.record_context_items(
            &r.id,
            &[NewAiContextItem {
                document_version_id: "silinmis-surum".into(),
                source_anchor_id: None,
                inclusion_reason: "onaylanan belge".into(),
                bytes_sent: 7,
                segment_ids: Vec::new(),
            }],
        )
        .expect("bağlam");

        let listed = db.list_context_items(&r.id).expect("liste");
        assert_eq!(listed.len(), 1, "denetim satırı sessizce düşmemeli");
        assert_eq!(listed[0].document_version_id, "silinmis-surum");
        assert_eq!(listed[0].document_title, None);
        assert_eq!(listed[0].version_no, None);
        assert_eq!(listed[0].bytes_sent, 7);
    }

    /// Makbuz **deneme anında** açılır. Eski davranış yalnız başarıda yazıyordu:
    /// zaman aşımına düşen bir aktarımın sicilde hiç izi kalmıyor, "denendi" ile
    /// "gönderildi" aynı kutuya giriyordu.
    #[test]
    fn transfer_attempt_is_recorded_before_dispatch() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let id = db
            .begin_transfer_receipt(&r.id, "api.ornek.com · m", &"c".repeat(64), 2, 99)
            .expect("deneme");
        let rows = db.list_transfer_receipts(&case_id).expect("liste");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].status, "attempted",
            "gönderim bitmeden «gönderildi» yazılmaz"
        );
        assert_eq!(rows[0].destination_label, "api.ornek.com · m");
        assert_eq!(rows[0].item_count, 2);
        assert_eq!(rows[0].byte_count, 99);
        assert_eq!(rows[0].outbound_hash, "c".repeat(64));
        assert_eq!(rows[0].ai_run_id, r.id);

        db.finish_transfer_receipt(&id, "dispatched")
            .expect("kapanış");
        let rows = db.list_transfer_receipts(&case_id).expect("liste");
        assert_eq!(rows[0].status, "dispatched");
    }

    /// Düşen gönderim makbuzu **failed** olarak kapanır ve satır durur:
    /// "denendi, gönderilemedi" sicilden silinmez.
    #[test]
    fn failed_transfer_keeps_a_failed_receipt() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let id = db
            .begin_transfer_receipt(&r.id, "api.ornek.com · m", &"e".repeat(64), 1, 12)
            .expect("deneme");
        db.finish_transfer_receipt(&id, "failed")
            .expect("başarısız kapanış");
        let rows = db.list_transfer_receipts(&case_id).expect("liste");
        assert_eq!(rows.len(), 1, "düşen deneme listeden kaybolmaz");
        assert_eq!(rows[0].status, "failed");
    }

    /// Sicil geriye dönük değiştirilemez: sonuçlanmış makbuz yeniden
    /// yazılamaz, bilinmeyen durum reddedilir ve reddedilen yazım satıra
    /// dokunmaz.
    #[test]
    fn receipt_status_is_final_and_validated() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");

        let done = db
            .begin_transfer_receipt(&r.id, "api.ornek.com · m", &"f".repeat(64), 1, 5)
            .expect("deneme");
        db.finish_transfer_receipt(&done, "dispatched")
            .expect("kapanış");
        let err = db
            .finish_transfer_receipt(&done, "failed")
            .expect_err("sonuçlanmış makbuz yeniden yazılmaz");
        assert_eq!(err.code(), "E_AI_RECEIPT");

        let open = db
            .begin_transfer_receipt(&r.id, "api.ornek.com · m", &"0".repeat(64), 1, 6)
            .expect("deneme");
        let err = db
            .finish_transfer_receipt(&open, "sent")
            .expect_err("bilinmeyen durum");
        assert_eq!(err.code(), "E_AI_RECEIPT");

        let err = db
            .finish_transfer_receipt("olmayan-makbuz", "dispatched")
            .expect_err("olmayan makbuz");
        assert_eq!(err.code(), "E_NOT_FOUND");

        let rows = db.list_transfer_receipts(&case_id).expect("liste");
        let open_row = rows.iter().find(|row| row.id == open).expect("satır");
        assert_eq!(
            open_row.status, "attempted",
            "reddedilen durum satıra yazılmaz"
        );
    }

    /// Liste **dosyaya kapsanır**: başka dosyanın makbuzu bu dosyanın
    /// envanterine girmez (denetim kanıtı dosya sınırını aşmaz).
    #[test]
    fn receipts_are_scoped_to_their_case() {
        let (db, case_id, _) = setup();
        let other = case_titled(&db, "Başka dosya");
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        db.begin_transfer_receipt(&r.id, "a.example · m", &"d".repeat(64), 1, 10)
            .expect("makbuz");
        assert_eq!(db.list_transfer_receipts(&case_id).expect("liste").len(), 1);
        assert!(
            db.list_transfer_receipts(&other).expect("liste").is_empty(),
            "başka dosyanın makbuzu sızmaz"
        );
    }

    /// En yeni makbuz önce gelir (ekran "son gönderim"i üstte yazar).
    #[test]
    fn receipts_are_listed_newest_first() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let older = db
            .begin_transfer_receipt(&r.id, "a.example · m", &"1".repeat(64), 1, 1)
            .expect("eski");
        let newer = db
            .begin_transfer_receipt(&r.id, "a.example · m", &"2".repeat(64), 1, 2)
            .expect("yeni");
        // Zaman damgası aynı milisaniyeye düşebilir; sıra sözleşmesini
        // sabitlemek için yaşlar açıkça yazılır.
        db.conn()
            .execute(
                "UPDATE transfer_receipts SET dispatched_at='2026-01-01T00:00:00Z' WHERE id=?1",
                [&older],
            )
            .expect("eski zaman");
        db.conn()
            .execute(
                "UPDATE transfer_receipts SET dispatched_at='2026-01-02T00:00:00Z' WHERE id=?1",
                [&newer],
            )
            .expect("yeni zaman");
        let rows = db.list_transfer_receipts(&case_id).expect("liste");
        assert_eq!(rows[0].id, newer, "en yeni önce");
        assert_eq!(rows[1].id, older);
    }

    /// Açılış uzlaştırması: sonucu yazılmadan kapanmış denemeler **interrupted**
    /// olur — sonsuza kadar "uçuşta" görünmez; sonuçlanmış satıra dokunulmaz ve
    /// geçiş kesindir (aynı makbuz sonradan sonuçlandırılamaz).
    #[test]
    fn reconcile_closes_orphaned_attempts_as_interrupted() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let orphan = db
            .begin_transfer_receipt(&r.id, "api.ornek.com · m", &"a".repeat(64), 1, 10)
            .expect("deneme");
        let sent = db
            .begin_transfer_receipt(&r.id, "api.ornek.com · m", &"b".repeat(64), 1, 11)
            .expect("deneme");
        let rejected = db
            .begin_transfer_receipt(&r.id, "api.ornek.com · m", &"c".repeat(64), 1, 12)
            .expect("deneme");
        db.finish_transfer_receipt(&sent, "dispatched")
            .expect("kapanış");
        db.finish_transfer_receipt(&rejected, "failed")
            .expect("kapanış");

        assert_eq!(
            db.reconcile_attempted_receipts().expect("uzlaştırma"),
            1,
            "yalnız yarım kalan satır kapanır"
        );
        let rows = db.list_transfer_receipts(&case_id).expect("liste");
        let status = |id: &str| {
            rows.iter()
                .find(|row| row.id == id)
                .map(|row| row.status.clone())
                .expect("satır")
        };
        assert_eq!(status(&orphan), "interrupted");
        assert_eq!(
            status(&sent),
            "dispatched",
            "sonuçlanmış satır geriye dönük değişmez"
        );
        assert_eq!(status(&rejected), "failed");

        // Geçiş **kesin**: kesilen makbuz sonradan sonuçlandırılamaz.
        let err = db
            .finish_transfer_receipt(&orphan, "dispatched")
            .expect_err("kesilen makbuz sonuçlanmaz");
        assert_eq!(err.code(), "E_AI_RECEIPT");
        // İkinci açılış yapacak bir şey bulmaz (idempotent).
        assert_eq!(db.reconcile_attempted_receipts().expect("yeniden"), 0);

        // Sayı denetime yazılır; makbuz kimliği, hedef ve özet **yazılmaz**.
        let (count, metadata): (i64, String) = db
            .conn()
            .query_row(
                "SELECT count(*), coalesce(max(metadata_allowlist),'') FROM audit_events \
                 WHERE operation='interrupt' AND category='transfer_receipt'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("denetim");
        assert_eq!(count, 1, "uzlaştırma denetime bir kez yazılır");
        assert!(
            metadata.contains("\"interrupted\":1"),
            "sayı denetime yazılır: {metadata}"
        );
        assert!(!metadata.contains("ornek.com"), "hedef denetime girmez");
        assert!(
            !metadata.contains(&orphan),
            "makbuz kimliği denetime girmez"
        );
    }

    /// Açılış uzlaştırması: sonucu yazılmadan kapanmış **çalıştırmalar**
    /// sonsuza kadar `running` görünmez. Kesin durum `failed` + `usage_report`
    /// işaretiyle yazılır; bitiş zamanı uydurulmaz, sonuçlanmış satıra
    /// dokunulmaz ve ikinci açılış 0 döner.
    #[test]
    fn reconcile_closes_orphaned_runs_as_interrupted() {
        let (db, case_id, _) = setup();
        let c = consent(&db, &case_id);
        let orphan = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let done = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        db.finish_ai_run(&done.id, "succeeded", "{\"billing\":\"unknown\"}")
            .expect("bitir");

        assert_eq!(
            db.reconcile_running_ai_runs().expect("uzlaştırma"),
            1,
            "yalnız yarım kalan çalıştırma kapanır"
        );

        let orphan_row = db.get_ai_run(&orphan.id).expect("oku").expect("satır");
        assert_eq!(orphan_row.status, "failed");
        assert_eq!(
            orphan_row.completed_at, None,
            "bitiş zamanı bilinmiyor; uydurulmaz"
        );
        let report: serde_json::Value =
            serde_json::from_str(&orphan_row.usage_report).expect("rapor");
        assert_eq!(report["error"], "interrupted");
        assert_eq!(report["reason"], "process_exit");

        let done_row = db.get_ai_run(&done.id).expect("oku").expect("satır");
        assert_eq!(
            done_row.status, "succeeded",
            "sonuçlanmış çalıştırma geriye dönük değişmez"
        );
        assert!(done_row.completed_at.is_some());

        assert_eq!(
            db.reconcile_running_ai_runs().expect("yeniden"),
            0,
            "idempotent"
        );

        let (count, metadata): (i64, String) = db
            .conn()
            .query_row(
                "SELECT count(*), coalesce(max(metadata_allowlist),'') FROM audit_events \
                 WHERE operation='interrupt' AND category='ai_run'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("denetim");
        assert_eq!(count, 1, "uzlaştırma denetime bir kez yazılır");
        assert!(
            metadata.contains("\"interrupted\":1"),
            "sayı denetime yazılır: {metadata}"
        );
        assert!(
            !metadata.contains(&orphan.id),
            "çalıştırma kimliği denetime girmez"
        );
    }

    fn case_titled(db: &Database, title: &str) -> String {
        db.create_case(&NewCase {
            title: title.into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: crate::Sensitivity::Normal,
            default_ai_policy: crate::AiPolicy::Off,
        })
        .expect("case")
        .id
    }

    fn page(index: i64, text: &str) -> crate::NewPage {
        crate::NewPage {
            index,
            text: text.into(),
            width: Some(595.0),
            height: Some(842.0),
            origin: "native".into(),
            warnings: Vec::new(),
            rotation: 0,
            coordinate_space: Some("unrotated_cropbox_points".into()),
            page_label: None,
            blocks: Vec::new(),
        }
    }

    /// Belge + sürüm + başarılı ayrıştırma + sayfalar. Dönüş: (belge, sürüm).
    fn document_with_text(
        db: &Database,
        case_id: &str,
        name: &str,
        pages: &[&str],
    ) -> (String, String) {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &document.id,
                case_id,
                &blob,
                &"d".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let stored: Vec<crate::NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, text)| page(i64::try_from(index).unwrap_or(i64::MAX), text))
            .collect();
        db.store_pages(&run.id, case_id, &stored).expect("pages");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("finish");
        (document.id, version.id)
    }

    /// Var olan belgeye **ayrıştırılmamış** yeni sürüm ekler (güncel sürüm olur).
    fn unparsed_version(db: &Database, case_id: &str, document_id: &str) -> String {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        db.add_version(
            document_id,
            case_id,
            &blob,
            &"e".repeat(64),
            10,
            "application/pdf",
        )
        .expect("version")
        .id
    }

    /// Var olan belgeye **ayrıştırılmış** yeni sürüm ekler (güncel sürüm olur).
    fn document_with_text_version(
        db: &Database,
        case_id: &str,
        document_id: &str,
        pages: &[&str],
    ) -> String {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let version = db
            .add_version(
                document_id,
                case_id,
                &blob,
                &"f".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let stored: Vec<crate::NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, text)| page(i64::try_from(index).unwrap_or(i64::MAX), text))
            .collect();
        db.store_pages(&run.id, case_id, &stored).expect("pages");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("finish");
        version.id
    }

    /// Önizlemede görünmeyen metin gönderilmemeli: güncel sürüm ayrıştırılmamışsa
    /// eski sürümün metni **ne** önizlemede **ne** gönderim kapsamında olmalı.
    #[test]
    fn preview_and_send_scope_never_diverge() {
        let (db, case_id, _) = setup();
        let (document_id, _v1) =
            document_with_text(&db, &case_id, "Sozlesme", &["BİRİNCİ SÜRÜM METNİ"]);
        let _v2 = unparsed_version(&db, &case_id, &document_id);
        let ids = vec![document_id.clone()];
        let preview = db.consent_preview(&case_id, &ids).expect("önizleme");
        // Aynı belge kümesiyle onay: gönderim kapsamı önizlemeyle **birebir** olmalı.
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: ids,
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let scope = db.consent_scope_segments(&case_id, &c.id).expect("kapsam");
        assert_eq!(
            preview.documents, 0,
            "güncel sürüm ayrıştırılmadı → önizlemede belge sayılmamalı"
        );
        assert!(
            scope.segments.is_empty(),
            "önizlemede görünmeyen eski sürüm metni gönderilmemeli ({} segment)",
            scope.segments.len()
        );
        assert_eq!(preview.segments, scope.segments.len() as i64);
        assert_eq!(preview.bytes, scope.bytes);
        assert_eq!(
            preview.skipped.first().map(|s| s.reason.as_str()),
            Some("not_parsed"),
            "kapsam dışı neden sessizce kaybolmamalı"
        );
        assert_eq!(
            scope.skipped.first().map(|s| s.reason.as_str()),
            Some("not_parsed")
        );
    }

    /// Gönderim kapsamı **güncel** sürümün metnidir; eski sürümün metni ve
    /// sayfa üretmeyen (başarısız) yeni koşu kapsamı bozamaz.
    #[test]
    fn scope_uses_the_current_version_and_the_latest_run_with_pages() {
        let (db, case_id, _) = setup();
        let (document_id, _v1) = document_with_text(&db, &case_id, "Belge", &["ESKİ METİN"]);
        let v2 = document_with_text_version(&db, &case_id, &document_id, &["YENİ METİN"]);
        // v2 üzerinde sayfa üretmeyen (başarısız) daha yeni bir koşu:
        let failed = db
            .start_parse_run(&v2, &case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("koşu");
        db.finish_parse_run(&failed.id, "failed", &[])
            .expect("bitir");
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let scope = db.consent_scope_segments(&case_id, &c.id).expect("kapsam");
        let texts: Vec<&str> = scope.segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["YENİ METİN"], "güncel sürüm metni gönderilmeli");
        assert!(scope.skipped.is_empty());
    }

    /// Kapsam yalnız onaydan gelir: onaya yazılmamış belge, çağıran istese de
    /// gönderim kapsamına girmez.
    #[test]
    fn send_scope_comes_from_the_consent_only() {
        let (db, case_id, _) = setup();
        let (approved, _) = document_with_text(&db, &case_id, "Onaylı", &["ONAYLI METİN"]);
        let (_other, _) = document_with_text(&db, &case_id, "Onaysız", &["ONAYSIZ METİN"]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![approved],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let scope = db.consent_scope_segments(&case_id, &c.id).expect("kapsam");
        let texts: Vec<&str> = scope.segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["ONAYLI METİN"]);
    }

    /// Onaydan sonra seçili belge silinirse kapsam daralmıştır: yeni çalıştırma
    /// açılmaz (sessizce eksik kapsamla gönderim yapılmaz).
    #[test]
    fn run_blocked_when_an_approved_document_is_deleted() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Metin"]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id.clone()],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        db.conn()
            .execute(
                "UPDATE documents SET deleted_at=?1 WHERE id=?2",
                rusqlite::params![now_iso(), document_id],
            )
            .expect("sil");
        assert_eq!(
            db.start_ai_run(&case_id, &run(&case_id, &c.id))
                .unwrap_err()
                .code(),
            "E_AI_CONSENT"
        );
        assert_eq!(
            db.consent_scope_segments(&case_id, &c.id)
                .expect("kapsam")
                .segments
                .len(),
            0
        );
    }

    /// Aktarım sicili belge **metnini** saklamaz: denetim/koşu/bağlam/makbuz
    /// satırlarında belge metni geçmez (yalnız kimlik, sayı ve özet).
    #[test]
    fn transfer_metadata_never_stores_document_text() {
        fn table_has(db: &Database, table: &str, needle: &str) -> bool {
            let mut stmt = db
                .conn()
                .prepare(&format!("PRAGMA table_info({table})"))
                .expect("şema");
            let columns: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .expect("sütunlar")
                .map(|column| column.expect("sütun"))
                .collect();
            for column in columns {
                let sql = format!(
                    "SELECT count(*) FROM {table} WHERE instr(CAST(\"{column}\" AS TEXT), ?1) > 0"
                );
                let hits: i64 = db
                    .conn()
                    .query_row(&sql, [needle], |row| row.get(0))
                    .expect("tarama");
                if hits > 0 {
                    return true;
                }
            }
            false
        }

        let (db, case_id, _) = setup();
        let canary = "GİZLİ-İBARE-9F3A";
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &[canary]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let scope = db.consent_scope_segments(&case_id, &c.id).expect("kapsam");
        assert_eq!(scope.segments.len(), 1);
        let run = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        db.record_context_items(
            &run.id,
            &[NewAiContextItem {
                document_version_id: scope.documents[0].document_version_id.clone(),
                source_anchor_id: None,
                inclusion_reason: "onaylanan belge".into(),
                bytes_sent: scope.documents[0].bytes,
                segment_ids: Vec::new(),
            }],
        )
        .expect("bağlam");
        // Şartname `transfer_segments` tablosu için **gönderilen metni** ister
        // (şifreli kasa içinde). Metin yalnız orada ve kasadaki segmenttedir;
        // makbuz/denetim/çalıştırma satırları metni taşımaz.
        db.record_transfer_segments(
            &run.id,
            &[NewTransferSegment {
                segment_id: scope.segments[0].segment_id.clone(),
                document_version_id: scope.segments[0].document_version_id.clone(),
                text: scope.segments[0].text.clone(),
            }],
        )
        .expect("gönderim sicili");
        let receipt = db
            .begin_transfer_receipt(
                &run.id,
                "api.ornek.com · m",
                &"b".repeat(64),
                1,
                scope.bytes,
            )
            .expect("makbuz");
        db.finish_transfer_receipt(&receipt, "dispatched")
            .expect("kapanış");
        db.finish_ai_run(&run.id, "succeeded", "{}").expect("bitir");
        for table in [
            "audit_log",
            "ai_runs",
            "ai_context_items",
            "transfer_receipts",
            "transfer_consents",
        ] {
            assert!(
                !table_has(&db, table, canary),
                "{table} belge metnini saklamamalı"
            );
        }
        // Metin gerçekten kasadadır (yanlış negatif olmasın): kontrol.
        assert!(table_has(&db, "text_segments", canary));
        // Gönderim sicili metni **taşır** (şartname gereği, kasa içinde).
        assert!(table_has(&db, "transfer_segments", canary));
    }

    /// Onay, kapsamındaki belgeleri **gerçekten** adlandırmalı: başka dosyanın
    /// belgesi ya da var olmayan kimlik onaya yazılamaz.
    #[test]
    fn consent_cannot_name_a_document_outside_the_case() {
        let (db, case_id, _) = setup();
        let other_case = case_titled(&db, "Başka dosya");
        let (foreign_document, _) =
            document_with_text(&db, &other_case, "Yabanci", &["BAŞKA DOSYA METNİ"]);
        for (label, ids) in [
            ("başka dosyanın belgesi", vec![foreign_document.clone()]),
            ("var olmayan kimlik", vec!["olmayan-belge".to_string()]),
        ] {
            let err = db
                .create_transfer_consent(&NewTransferConsent {
                    provider_id: "p1".into(),
                    case_scope: case_id.clone(),
                    purpose: "ozet".into(),
                    selected_document_ids: ids,
                    expires_at: None,
                    consent_text_version: "1".into(),
                    mask_terms: Vec::new(),
                    mask_plan_id: None,
                })
                .expect_err(label);
            assert_eq!(err.code(), "E_AI_CONSENT", "{label}");
        }
    }

    /// Belge seçimi taşımayan onay hiçbir şeyi yetkilendirmez.
    #[test]
    fn consent_requires_a_concrete_document_selection() {
        let (db, case_id, _) = setup();
        let err = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id,
                purpose: "ozet".into(),
                selected_document_ids: vec![],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect_err("boş seçim");
        assert_eq!(err.code(), "E_AI_CONSENT");
    }

    /// Verilen sürümün segmentleri (kimlik, metin) — kayıt sırası.
    fn segments_for_version(db: &Database, version_id: &str) -> Vec<(String, String)> {
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT s.id, s.raw_text FROM text_segments s
                 JOIN parse_runs pr ON pr.id = s.parse_run_id
                 WHERE pr.document_version_id=?1 ORDER BY s.rowid",
            )
            .expect("stmt");
        let rows = stmt
            .query_map([version_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("rows");
        rows.map(|row| row.expect("satır")).collect()
    }

    /// Belge kümesine bağlı onay (test yardımcısı).
    fn consent_for_document(db: &Database, case_id: &str, document_id: &str) -> TransferConsentRow {
        db.create_transfer_consent(&NewTransferConsent {
            provider_id: "p1".into(),
            case_scope: case_id.into(),
            purpose: "ozet".into(),
            selected_document_ids: vec![document_id.to_string()],
            expires_at: None,
            consent_text_version: "1".into(),
            mask_terms: Vec::new(),
            mask_plan_id: None,
        })
        .expect("onay")
    }

    /// Gönderim sicili **metnin kendisini** taşır: kimlik eşlemesi, karakter/bayt
    /// sayısı ve sha256 çekirdekte hesaplanır; sıra kayıt sırasıdır.
    #[test]
    fn transfer_segments_record_identity_map_and_hash() {
        let (db, case_id, _) = setup();
        let (document_id, version_id) =
            document_with_text(&db, &case_id, "Belge", &["Kira bedeli ödenmedi."]);
        let c = consent_for_document(&db, &case_id, &document_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let segments = segments_for_version(&db, &version_id);
        assert_eq!(segments.len(), 1, "tek sayfa tek segment üretir");
        let (segment_id, text) = segments[0].clone();
        assert_eq!(
            db.record_transfer_segments(
                &r.id,
                &[NewTransferSegment {
                    segment_id: segment_id.clone(),
                    document_version_id: version_id.clone(),
                    text: text.clone(),
                }],
            )
            .expect("sicil"),
            1
        );

        let rows = db.list_transfer_segments(&r.id).expect("liste");
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.segment_id, segment_id);
        assert_eq!(
            row.document_version_id.as_deref(),
            Some(version_id.as_str())
        );
        assert_eq!(row.char_count, i64::try_from(text.chars().count()).unwrap());
        assert_eq!(row.byte_count, i64::try_from(text.len()).unwrap());
        assert_eq!(row.transform_kind, "identity");
        assert_eq!(row.outbound_hash, sha256_hex(text.as_bytes()));
        assert_eq!(row.outbound_hash.len(), 64);
        assert_eq!(row.text_head, text);

        // Aralık eşlemesi kimliktir: gönderilen aralık = özgün aralık.
        let map: String = db
            .conn()
            .query_row(
                "SELECT source_range_map FROM transfer_segments WHERE ai_run_id=?1",
                [&r.id],
                |row| row.get(0),
            )
            .expect("harita");
        let value: serde_json::Value = serde_json::from_str(&map).expect("json");
        assert_eq!(value[0]["outStart"], 0);
        assert_eq!(value[0]["outEnd"], text.chars().count());
        assert_eq!(value[0]["sourceStart"], 0);
        assert_eq!(value[0]["sourceEnd"], text.chars().count());

        // Tam metin yalnız doğrulama yolunda; başka çalıştırmanın sicili ayrıdır.
        let sent = db.sent_segments(&r.id).expect("gönderilen");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].text, text);
        let other = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("ikinci çalıştırma");
        assert!(db
            .list_transfer_segments(&other.id)
            .expect("liste")
            .is_empty());
    }

    /// Beyan edilen metin kasadaki segmentle birebir aynı değilse sicile
    /// **hiçbir** satır yazılmaz; mükerrer kayıt ham SQL hatası olarak değil
    /// `E_DB_SETTING` olarak reddedilir.
    #[test]
    fn transfer_segment_text_must_match_the_vault() {
        let (db, case_id, _) = setup();
        let (document_id, version_id) =
            document_with_text(&db, &case_id, "Belge", &["Kira bedeli ödenmedi."]);
        let c = consent_for_document(&db, &case_id, &document_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let (segment_id, text) = segments_for_version(&db, &version_id)[0].clone();

        let err = db
            .record_transfer_segments(
                &r.id,
                &[NewTransferSegment {
                    segment_id: segment_id.clone(),
                    document_version_id: version_id.clone(),
                    text: "uydurulmuş metin".into(),
                }],
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_DB_SETTING");
        assert!(
            db.list_transfer_segments(&r.id).expect("liste").is_empty(),
            "hata sonrası satır kalmaz"
        );

        let err = db
            .record_transfer_segments(
                &r.id,
                &[NewTransferSegment {
                    segment_id: "olmayan-segment".into(),
                    document_version_id: version_id.clone(),
                    text: text.clone(),
                }],
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND", "var olmayan segment yazılamaz");

        db.record_transfer_segments(
            &r.id,
            &[NewTransferSegment {
                segment_id: segment_id.clone(),
                document_version_id: version_id.clone(),
                text: text.clone(),
            }],
        )
        .expect("geçerli sicil");
        let err = db
            .record_transfer_segments(
                &r.id,
                &[NewTransferSegment {
                    segment_id: segment_id.clone(),
                    document_version_id: version_id.clone(),
                    text: text.clone(),
                }],
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_DB_SETTING", "mükerrer satır reddedilir");
        assert_eq!(db.list_transfer_segments(&r.id).expect("liste").len(), 1);
    }

    /// `ai_context_items.segment_ids` artık gerçek kimlikleri taşır; uydurma
    /// kimlik yazılamaz ve satır sayısı okunabilir.
    #[test]
    fn context_items_record_real_segment_ids() {
        let (db, case_id, _) = setup();
        let (document_id, version_id) =
            document_with_text(&db, &case_id, "Belge", &["Kira bedeli ödenmedi."]);
        let c = consent_for_document(&db, &case_id, &document_id);
        let r = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let (segment_id, text) = segments_for_version(&db, &version_id)[0].clone();
        let bytes = i64::try_from(text.len()).unwrap();
        db.record_context_items(
            &r.id,
            &[NewAiContextItem {
                document_version_id: version_id.clone(),
                source_anchor_id: None,
                inclusion_reason: "onaylanan belge".into(),
                bytes_sent: bytes,
                segment_ids: vec![segment_id],
            }],
        )
        .expect("bağlam");
        let rows = db.list_context_items(&r.id).expect("liste");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].segment_count, Some(1));

        let err = db
            .record_context_items(
                &r.id,
                &[NewAiContextItem {
                    document_version_id: version_id.clone(),
                    source_anchor_id: None,
                    inclusion_reason: "onaylanan belge".into(),
                    bytes_sent: bytes,
                    segment_ids: vec!["uydurma-segment".into()],
                }],
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_NOT_FOUND");
        assert_eq!(
            db.list_context_items(&r.id).expect("liste").len(),
            1,
            "hata sonrası ikinci satır yazılmaz"
        );
    }

    /// Maske uygulanan gönderim: dönüşüm sürümü + özgün aralık eşlemesi yazılır
    /// (şartname §737). Maskelenen ad **gönderilen metinde de sicilde de**
    /// görünmez; eşleme yalnız kasa içinde durur.
    #[test]
    fn masked_outbound_records_transform_and_original_range_map() {
        use crate::outbound_mask::map_span_in_runs;

        let (db, case_id, _) = setup();
        let raw = "Ahmet Yılmaz 12.000 TL ödedi.";
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &[raw]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let scope = db.consent_scope_segments(&case_id, &c.id).expect("kapsam");
        let ai_run = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let record = db
            .record_outbound_segments(
                &ai_run.id,
                &[OutboundSegmentRef {
                    segment_id: scope.segments[0].segment_id.clone(),
                    document_version_id: scope.segments[0].document_version_id.clone(),
                }],
                Some(&OutboundMask {
                    terms: vec!["Ahmet Yılmaz".into()],
                }),
            )
            .expect("maskeli gönderim");
        assert_eq!(record.transform_kind, "mask_terms");
        assert_eq!(record.term_count, 1);
        assert_eq!(record.masked_runs, 1);
        assert_eq!(record.masked_chars, 12);
        assert_eq!(record.segments[0].text, "[MASKED] 12.000 TL ödedi.");
        assert!(record.transform_id.is_some(), "dönüşüm sürümü yazılmalı");

        let rows = db.list_transfer_segments(&ai_run.id).expect("sicil");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].transform_kind, "mask_terms");
        assert_eq!(rows[0].mask_term_count, 1);
        assert!(
            !rows[0].text_head.contains("Ahmet"),
            "maskelenen ad sicil başlığında görünmemeli: {}",
            rows[0].text_head
        );

        let sent = db.sent_segments(&ai_run.id).expect("gönderilen");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].transform_kind, "mask_terms");
        let runs = sent[0].runs.as_deref().expect("harita");
        // Kuyruk birebir eşlenir: gönderilen 8..25 ↔ özgün 12..29.
        assert_eq!(
            map_span_in_runs(runs, 8, 25),
            Some((12, 29)),
            "kopya parçası özgün aralığa dönmeli"
        );
        assert_eq!(
            map_span_in_runs(runs, 0, 8),
            None,
            "maske parçası geri eşlenemez"
        );

        // Terimlerin kendisi hiçbir tabloya yazılmaz (yalnız özet + sayı).
        for (table, column) in [
            ("outbound_transforms", "settings_hash"),
            ("outbound_transforms", "placeholder"),
        ] {
            let hits: i64 = db
                .conn()
                .query_row(
                    &format!(
                        "SELECT count(*) FROM {table} WHERE instr(CAST(\"{column}\" AS TEXT),'Ahmet') > 0"
                    ),
                    [],
                    |row| row.get(0),
                )
                .expect("tarama");
            assert_eq!(hits, 0, "{table}.{column} terimi saklamamalı");
        }
    }

    /// Kullanılamayan maske **hiçbir satır yazmadan** reddedilir: kısa terim,
    /// yer tutucu çakışması, yanlış segment kimliği.
    #[test]
    fn outbound_mask_failures_write_no_rows() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Ahmet Yılmaz ödedi."]);
        let (placeholder_document, _) =
            document_with_text(&db, &case_id, "YerTutucu", &["Metin [MASKED] içeriyor."]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id.clone(), placeholder_document],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let scope = db.consent_scope_segments(&case_id, &c.id).expect("kapsam");
        let ai_run = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let first = &scope.segments[0];
        let refs = vec![OutboundSegmentRef {
            segment_id: first.segment_id.clone(),
            document_version_id: first.document_version_id.clone(),
        }];
        let cases: Vec<(Option<&str>, &str)> =
            vec![(Some("a"), "E_MASK_TERM"), (None, "E_MASK_PLACEHOLDER")];
        for (index, (term, expected)) in cases.into_iter().enumerate() {
            let target = if index == 0 {
                refs.clone()
            } else {
                // Yer tutucu taşıyan segment (ikinci belge).
                scope
                    .segments
                    .iter()
                    .filter(|segment| segment.segment_id != first.segment_id)
                    .map(|segment| OutboundSegmentRef {
                        segment_id: segment.segment_id.clone(),
                        document_version_id: segment.document_version_id.clone(),
                    })
                    .collect()
            };
            let mask = OutboundMask {
                terms: term.map_or_else(|| vec!["Ahmet".to_string()], |t| vec![t.to_string()]),
            };
            let error = db
                .record_outbound_segments(&ai_run.id, &target, Some(&mask))
                .expect_err("reddedilmeli");
            assert_eq!(error.code(), expected);
            assert_eq!(
                db.list_transfer_segments(&ai_run.id).expect("sicil").len(),
                0,
                "reddedilen maske satır bırakmamalı"
            );
        }
        // Yanlış sürüm kimliği: segment başka sürüme ait sayılmaz.
        let bad = vec![OutboundSegmentRef {
            segment_id: first.segment_id.clone(),
            document_version_id: "baska-surum".into(),
        }];
        let error = db
            .record_outbound_segments(&ai_run.id, &bad, None)
            .expect_err("reddedilmeli");
        assert_eq!(error.code(), "E_NOT_FOUND");
        assert_eq!(
            db.list_transfer_segments(&ai_run.id).expect("sicil").len(),
            0
        );
    }

    /// Maske uygulanmasa bile kimlik eşlemesi **yazılır**: alıntı doğrulaması
    /// hangi metnin gittiğini ve nasıl eşlendiğini bilir.
    #[test]
    fn identity_outbound_records_identity_map() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["Düz metin."]);
        let c = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: None,
            })
            .expect("onay");
        let scope = db.consent_scope_segments(&case_id, &c.id).expect("kapsam");
        let ai_run = db
            .start_ai_run(&case_id, &run(&case_id, &c.id))
            .expect("çalıştır");
        let record = db
            .record_outbound_segments(
                &ai_run.id,
                &[OutboundSegmentRef {
                    segment_id: scope.segments[0].segment_id.clone(),
                    document_version_id: scope.segments[0].document_version_id.clone(),
                }],
                None,
            )
            .expect("gönderim");
        assert_eq!(record.transform_kind, "identity");
        assert!(record.transform_id.is_none());
        assert_eq!(record.segments[0].text, "Düz metin.");
        let sent = db.sent_segments(&ai_run.id).expect("gönderilen");
        let runs = sent[0].runs.as_deref().expect("kimlik haritası");
        assert_eq!(
            crate::outbound_mask::map_span_in_runs(runs, 0, 4),
            Some((0, 4))
        );
        let rows = db.list_transfer_segments(&ai_run.id).expect("sicil");
        assert_eq!(rows[0].transform_kind, "identity");
        assert_eq!(rows[0].mask_term_count, 0);
    }

    fn started_run(db: &Database, case_id: &str) -> String {
        let c = consent(db, case_id);
        db.start_ai_run(case_id, &run(case_id, &c.id))
            .expect("çalıştır")
            .id
    }

    #[test]
    fn visible_answer_is_recorded_and_read_back() {
        let (db, case_id, _) = setup();
        let run_id = started_run(&db, &case_id);
        let refs = vec!["seg-1".to_string(), "seg-2".to_string()];
        let row = db
            .record_ai_message(
                &run_id,
                "assistant",
                "Kira borcu 12.000 TL görünüyor.",
                &refs,
                "visible",
            )
            .expect("yanıt");
        assert_eq!(row.role, "assistant");
        assert_eq!(row.state, "visible");
        assert_eq!(row.ai_run_id, run_id);
        assert_eq!(row.visible_content, "Kira borcu 12.000 TL görünüyor.");
        assert_eq!(row.source_refs, refs);

        let listed = db.list_ai_messages(&run_id).expect("liste");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, row.id);
        assert_eq!(listed[0].source_refs, refs);
    }

    #[test]
    fn answer_text_never_reaches_the_audit_log() {
        let (db, case_id, _) = setup();
        let run_id = started_run(&db, &case_id);
        db.record_ai_message(
            &run_id,
            "assistant",
            "Gizli müvekkil adı: Ayşe.",
            &[],
            "visible",
        )
        .expect("yanıt");
        let events = db
            .list_audit_events(50, Some("ai_message"))
            .expect("denetim");
        assert_eq!(events.len(), 1);
        let metadata = serde_json::to_string(&events[0].metadata).expect("meta");
        assert!(!metadata.contains("Ayşe"), "{metadata}");
        assert!(metadata.contains("\"chars\""), "{metadata}");
    }

    #[test]
    fn message_role_and_state_are_closed_sets() {
        let (db, case_id, _) = setup();
        let run_id = started_run(&db, &case_id);
        assert_eq!(
            db.record_ai_message(&run_id, "vali", "metin", &[], "visible")
                .unwrap_err()
                .code(),
            "E_AI_MESSAGE"
        );
        assert_eq!(
            db.record_ai_message(&run_id, "assistant", "metin", &[], "onaylandı")
                .unwrap_err()
                .code(),
            "E_AI_MESSAGE"
        );
        assert_eq!(
            db.record_ai_message("bilinmeyen-run", "assistant", "metin", &[], "visible")
                .unwrap_err()
                .code(),
            "E_NOT_FOUND"
        );
    }

    #[test]
    fn empty_or_control_content_is_rejected() {
        let (db, case_id, _) = setup();
        let run_id = started_run(&db, &case_id);
        assert_eq!(
            db.record_ai_message(&run_id, "assistant", "   ", &[], "visible")
                .unwrap_err()
                .code(),
            "E_AI_MESSAGE"
        );
        assert_eq!(
            db.record_ai_message(&run_id, "assistant", "a\u{0}b", &[], "visible")
                .unwrap_err()
                .code(),
            "E_AI_MESSAGE"
        );
        // Satır başı/sekme serbesttir: model yanıtı çok satırlı olabilir.
        db.record_ai_message(
            &run_id,
            "assistant",
            "satır 1\nsatır 2\tbitti",
            &[],
            "visible",
        )
        .expect("çok satırlı yanıt");
    }

    #[test]
    fn long_answer_is_clipped_and_marked_truncated() {
        let (db, case_id, _) = setup();
        let run_id = started_run(&db, &case_id);
        let long = "ş".repeat(Database::AI_MESSAGE_CHAR_LIMIT + 10);
        let row = db
            .record_ai_message(&run_id, "assistant", &long, &[], "visible")
            .expect("yanıt");
        assert_eq!(row.state, "truncated");
        assert_eq!(
            row.visible_content.chars().count(),
            Database::AI_MESSAGE_CHAR_LIMIT
        );
        // Kırpma sessiz değil: satır durumu ve uzunluk okuyan tarafından görülür.
        let listed = db.list_ai_messages(&run_id).expect("liste");
        assert_eq!(listed[0].state, "truncated");
    }

    #[test]
    fn messages_are_scoped_and_ordered() {
        let (db, case_id, _) = setup();
        let first = started_run(&db, &case_id);
        let second = started_run(&db, &case_id);
        db.record_ai_message(&first, "assistant", "ilk yanıt", &[], "visible")
            .expect("1");
        db.record_ai_message(&first, "assistant", "ikinci yanıt", &[], "unparsed")
            .expect("2");
        db.record_ai_message(&second, "assistant", "başka çalıştırma", &[], "visible")
            .expect("3");
        let listed = db.list_ai_messages(&first).expect("liste");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].visible_content, "ilk yanıt");
        assert_eq!(listed[1].visible_content, "ikinci yanıt");
        assert_eq!(listed[1].state, "unparsed");
        assert!(db.list_ai_messages(&second).expect("liste")[0]
            .visible_content
            .contains("başka"));
    }

    #[test]
    fn malformed_reference_is_rejected() {
        let (db, case_id, _) = setup();
        let run_id = started_run(&db, &case_id);
        for bad in [
            vec![" ".to_string()],
            vec![" seg".to_string()],
            vec!["a".repeat(129)],
            vec!["seg-1".to_string(), "seg-1".to_string()],
        ] {
            assert_eq!(
                db.record_ai_message(&run_id, "assistant", "metin", &bad, "visible")
                    .unwrap_err()
                    .code(),
                "E_AI_MESSAGE",
                "{bad:?}"
            );
        }
    }

    /// Belge + sürüm + sayfa + **tek metin bloğu** (bölge kesişimi için).
    ///
    /// `document_with_text` blok kutusu yazmaz; plan maskesi blok–bölge
    /// kesişiminden türediği için bu fikstür gerekli.
    fn document_with_block(
        db: &Database,
        case_id: &str,
        name: &str,
        text: &str,
        bbox: [f64; 4],
    ) -> (String, String) {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &document.id,
                case_id,
                &blob,
                &"b".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let pages = vec![crate::NewPage {
            index: 0,
            text: text.into(),
            width: Some(595.0),
            height: Some(842.0),
            origin: "native".into(),
            warnings: Vec::new(),
            rotation: 0,
            coordinate_space: Some("unrotated_cropbox_points".into()),
            page_label: None,
            blocks: vec![crate::SegmentBlock {
                start: 0,
                end: i64::try_from(text.chars().count()).unwrap_or(i64::MAX),
                bbox: Some(bbox),
                lines: Vec::new(),
                path: None,
                kind: None,
            }],
        }];
        db.store_pages(&run.id, case_id, &pages).expect("pages");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("finish");
        (document.id, version.id)
    }

    // ---- Onay maskeye bağlıdır (şartname §763) ----

    /// Onay **maskeyle** alınır: serbest terimler + (varsa) plandan türetilen
    /// terimler onayın parçasıdır. Terimler kasaya yazılmaz — onay yalnız
    /// `outbound_transforms` tanımına bağlanır.
    fn consent_with_mask(
        db: &Database,
        case_id: &str,
        document_id: &str,
        terms: &[&str],
    ) -> TransferConsentRow {
        db.create_transfer_consent(&NewTransferConsent {
            provider_id: "p1".into(),
            case_scope: case_id.into(),
            purpose: "ozet".into(),
            selected_document_ids: vec![document_id.to_string()],
            expires_at: None,
            consent_text_version: "1".into(),
            mask_terms: terms.iter().map(|term| (*term).to_string()).collect(),
            mask_plan_id: None,
        })
        .expect("onay")
    }

    fn run_with_mask(consent_id: &str, terms: &[&str]) -> NewAiRun {
        NewAiRun {
            mask_terms: terms.iter().map(|term| (*term).to_string()).collect(),
            ..run("", consent_id)
        }
    }

    #[test]
    fn consent_binds_the_mask_that_was_approved() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(
            &db,
            &case_id,
            "Belge",
            &["Ahmet Yılmaz sözleşmeyi imzaladı"],
        );
        let consent = consent_with_mask(&db, &case_id, &document_id, &["Ahmet Yılmaz"]);
        assert!(
            consent.outbound_transform_id.is_some(),
            "onay maskeye bağlanmalı"
        );
        // Aynı maske → çalışır.
        db.start_ai_run(&case_id, &run_with_mask(&consent.id, &["Ahmet Yılmaz"]))
            .expect("aynı maske");
        // Kanonik biçim eşitliği: kırpma/tekrar aynı tanımdır (yeni onay istemez).
        db.start_ai_run(
            &case_id,
            &run_with_mask(&consent.id, &["  Ahmet Yılmaz  ", "Ahmet Yılmaz"]),
        )
        .expect("kanonik eşit maske");
        // Maskenin **kaldırılması** izni genişletir → reddedilir.
        assert_eq!(
            db.start_ai_run(&case_id, &run_with_mask(&consent.id, &[]))
                .unwrap_err()
                .code(),
            "E_AI_MASK_CHANGED"
        );
        // Farklı maske de onaylanmayan bir metin gönderir → reddedilir.
        assert_eq!(
            db.start_ai_run(&case_id, &run_with_mask(&consent.id, &["Mehmet Demir"]))
                .unwrap_err()
                .code(),
            "E_AI_MASK_CHANGED"
        );
    }

    #[test]
    fn consent_without_mask_refuses_a_mask_at_execution() {
        let (db, case_id, _) = setup();
        let consent = consent(&db, &case_id);
        assert!(
            consent.outbound_transform_id.is_some(),
            "maskesiz onay da bir maske tanımına (0 terim) bağlanır"
        );
        db.start_ai_run(&case_id, &run(&case_id, &consent.id))
            .expect("maskesiz onay + maskesiz gönderim");
        assert_eq!(
            db.start_ai_run(&case_id, &run_with_mask(&consent.id, &["Ahmet Yılmaz"]))
                .unwrap_err()
                .code(),
            "E_AI_MASK_CHANGED",
            "maskesiz onayla maskeli gönderim onaylanmayan metin üretir"
        );
    }

    #[test]
    fn pre_v30_consent_without_mask_binding_is_refused() {
        let (db, case_id, _) = setup();
        let consent = consent(&db, &case_id);
        // v29 kasasından gelen onay: maske bağı yok (\u2192 NULL).
        db.conn()
            .execute(
                "UPDATE transfer_consents SET outbound_transform_id=NULL WHERE id=?1",
                [&consent.id],
            )
            .expect("eski onay");
        assert_eq!(
            db.start_ai_run(&case_id, &run(&case_id, &consent.id))
                .unwrap_err()
                .code(),
            "E_AI_CONSENT_UNBOUND",
            "bağ bilinmiyorsa maskesiz gönderim sessizce geçmemeli"
        );
    }

    #[test]
    fn consent_mask_plan_terms_are_part_of_the_binding() {
        let (db, case_id, _) = setup();
        let (document_id, version_id) = document_with_block(
            &db,
            &case_id,
            "Karar",
            "Ahmet Yılmaz",
            [0.0, 0.0, 200.0, 50.0],
        );
        let plan = db
            .create_redaction_plan(
                &case_id,
                &crate::NewRedactionPlan {
                    document_version_id: version_id.clone(),
                    parse_run_id: None,
                    render_hash: "b".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        db.add_redaction_regions(
            &plan.id,
            &[crate::RedactionRegionInput {
                page_index: 0,
                rect: [10.0, 10.0, 150.0, 40.0],
                term_hash: "h".into(),
            }],
        )
        .expect("bölge");
        let derived = db
            .outbound_terms_from_plan(&case_id, &plan.id)
            .expect("plan terimleri");
        assert!(
            !derived.terms.is_empty(),
            "bölge kesişen bloktan terim üretmeli: {derived:?}"
        );
        let consent = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id.clone()],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: Some(plan.id.clone()),
            })
            .expect("planlı onay");
        // Plandan türetilen terimler onayın parçasıdır: maskesiz gönderim reddedilir.
        assert_eq!(
            db.start_ai_run(&case_id, &run(&case_id, &consent.id))
                .unwrap_err()
                .code(),
            "E_AI_MASK_CHANGED"
        );
        db.start_ai_run(
            &case_id,
            &NewAiRun {
                mask_terms: derived.terms.clone(),
                ..run(&case_id, &consent.id)
            },
        )
        .expect("plan maskesi onayla eşleşir");
    }

    #[test]
    fn consent_refuses_a_plan_outside_the_selected_documents() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["metin"]);
        let (_other_document, other_version) =
            document_with_text(&db, &case_id, "Başka belge", &["başka metin"]);
        let plan = db
            .create_redaction_plan(
                &case_id,
                &crate::NewRedactionPlan {
                    document_version_id: other_version,
                    parse_run_id: None,
                    render_hash: "d".repeat(64),
                    output_profile: "share".into(),
                    output_version_id: None,
                },
            )
            .expect("plan");
        let error = db
            .create_transfer_consent(&NewTransferConsent {
                provider_id: "p1".into(),
                case_scope: case_id.clone(),
                purpose: "ozet".into(),
                selected_document_ids: vec![document_id],
                expires_at: None,
                consent_text_version: "1".into(),
                mask_terms: Vec::new(),
                mask_plan_id: Some(plan.id),
            })
            .unwrap_err();
        assert_eq!(error.code(), "E_MASK_PLAN_SCOPE");
    }

    #[test]
    fn consent_list_reports_the_mask_definition() {
        let (db, case_id, _) = setup();
        let (document_id, _) = document_with_text(&db, &case_id, "Belge", &["metin"]);
        let masked = consent_with_mask(&db, &case_id, &document_id, &["Ahmet", "Yılmaz"]);
        let plain = consent(&db, &case_id);
        let rows = db.list_transfer_consents(&case_id).expect("liste");
        let masked_row = rows
            .iter()
            .find(|row| row.consent.id == masked.id)
            .expect("maskeli satır");
        assert_eq!(masked_row.mask_term_count, 2);
        let plain_row = rows
            .iter()
            .find(|row| row.consent.id == plain.id)
            .expect("düz satır");
        assert_eq!(plain_row.mask_term_count, 0);
        assert!(plain_row.consent.outbound_transform_id.is_some());
    }
}
