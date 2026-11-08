//! Dosya tarafları — şema `actors` + `case_participants` (+ `actor_merges`).
//!
//! Neden bu modül: şartname 08 tablo sözleşmesi kişi/kurum kaydını ve dosya
//! rolünü **ayrı** tutar (`actors` / `case_participants`), şartname F08-T01
//! "kişi/kurum ve dosya rolünü ayır; birleştirmeyi inceleme ve **geri alma**
//! işlemine bağla" der. Tablolar şemanın ilk sürümünden beri var ama modülde
//! **tek SQL yoktu** (`git grep`: tablolara tek çağrı yok) — yani taraf/rol
//! kaydı üründe hiç yoktu; iddia ekranındaki "taraf" alanı da bu yüzden
//! bağlanamamıştı.
//!
//! Kurallar:
//! - `kind` **kapalı küme** (tablo CHECK'iyle aynı). Bilinmeyen tür yazılmaz.
//! - Ad/rol kırpılır; boş, kontrol karakterli veya sınır üstü değer yazılmaz.
//! - Yeni taraf **tek işlemde** yazılır: aktör + katılımcı. Katılımcı yazımı
//!   düşerse aktör de geri alınır (sahipsiz kişi kaydı kalmaz).
//! - Aynı dosyada aynı taraf + aynı rol ikinci kez yazılmaz (tablo UNIQUE'i;
//!   hata `E_DB_ACTOR` olarak eşlenir, ham SQL hatası arayüze sızmaz).
//! - **Birleştirme** yalnız çakışma yoksa yapılır: aynı dosyada aynı rol iki
//!   tarafta da kayıtlıysa birleştirme reddedilir (sessiz yinelenen rol
//!   üretilmez). Birleştirme kaydı **kaynak/hedef/taşınan satırlar** ile
//!   saklanır; geri alma rolleri kaynağa döndürür ve kaynağın **önceki
//!   inceleme durumunu** birebir yazar.
//! - `normalized_names` basit küçük harf normalizasyonudur; eşleştirme
//!   iddiası taşımaz (Türkçe büyük/küçük harf incelikleri ayrı iş).

use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::{now_iso, Database, StorageError};

/// Tablodaki `kind` CHECK kısıtıyla **aynı** kapalı küme.
pub const ACTOR_KINDS: [&str; 3] = ["person", "organization", "unknown"];

/// Taraf adı üst sınırı (kırpılmış karakter sayısı).
pub const MAX_PARTY_NAME: usize = 200;
/// Rol üst sınırı.
pub const MAX_PARTY_ROLE: usize = 80;
/// Rol notu üst sınırı.
pub const MAX_PARTY_NOTE: usize = 500;

/// Liste üst sınırı (sessizce sınırsız liste dönmez).
pub const MAX_PARTY_LIST: usize = 200;

/// Birleştirme geçmişi üst sınırı.
pub const MAX_ACTOR_MERGE_LIST: usize = 50;

/// Yeni taraf girdisi. `actor_id` doluysa **mevcut** tarafa rol eklenir;
/// boşsa `display_name` + `kind` ile yeni taraf açılır.
pub struct NewCaseParty<'a> {
    pub display_name: &'a str,
    pub kind: &'a str,
    pub role: &'a str,
    pub role_note: Option<&'a str>,
    pub actor_id: Option<&'a str>,
}

/// Dosya taraflarının okunmuş satırı. Alan adları arayüzde camelCase'tir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantRow {
    pub id: String,
    pub case_id: String,
    pub actor_id: String,
    pub display_name: String,
    pub kind: String,
    pub role: String,
    pub role_note: Option<String>,
    pub created_at: String,
}

/// Birleştirme kaydı. `case_ids` taşınan satırların dosyalarıdır; arayüz
/// geçmişi dosya kapsamında süzmek için kullanır.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorMergeRow {
    pub id: String,
    pub source_actor_id: String,
    pub source_display_name: String,
    pub target_actor_id: String,
    pub target_display_name: String,
    pub moved_participant_ids: Vec<String>,
    pub case_ids: Vec<String>,
    pub reviewer_label: String,
    pub created_at: String,
    pub undone_at: Option<String>,
}

/// Dosya tarafları + birleştirme geçmişi (tek okumada).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseParties {
    pub participants: Vec<ParticipantRow>,
    pub merges: Vec<ActorMergeRow>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MovedParticipant {
    id: String,
    case_id: String,
}

fn invalid(reason: &str) -> StorageError {
    StorageError::ActorInvalid {
        reason: reason.to_string(),
    }
}

fn party_name(value: &str) -> Result<String, StorageError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid("taraf adı boş olamaz"));
    }
    if trimmed.chars().count() > MAX_PARTY_NAME {
        return Err(invalid("taraf adı 200 karakterden uzun"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(invalid("taraf adı kontrol karakteri taşıyor"));
    }
    Ok(trimmed.to_string())
}

fn party_kind(value: &str) -> Result<&str, StorageError> {
    let trimmed = value.trim();
    if ACTOR_KINDS.contains(&trimmed) {
        Ok(trimmed)
    } else {
        Err(invalid("bilinmeyen taraf türü"))
    }
}

fn party_role(value: &str) -> Result<String, StorageError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid("rol boş olamaz"));
    }
    if trimmed.chars().count() > MAX_PARTY_ROLE {
        return Err(invalid("rol 80 karakterden uzun"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(invalid("rol kontrol karakteri taşıyor"));
    }
    Ok(trimmed.to_string())
}

fn party_note(value: Option<&str>) -> Result<Option<String>, StorageError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_PARTY_NOTE {
        return Err(invalid("rol notu 500 karakterden uzun"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(invalid("rol notu kontrol karakteri taşıyor"));
    }
    Ok(Some(trimmed.to_string()))
}

/// Basit küçük harf normalizasyonu. Eşleştirme iddiası **yoktur**; yalnız
/// ileride öneri üretilebilmesi için adın bir kopyası saklanır.
fn normalized_names(name: &str) -> String {
    serde_json::json!([name.to_lowercase()]).to_string()
}

fn map_participant(row: &Row<'_>) -> rusqlite::Result<ParticipantRow> {
    Ok(ParticipantRow {
        id: row.get(0)?,
        case_id: row.get(1)?,
        actor_id: row.get(2)?,
        display_name: row.get(3)?,
        kind: row.get(4)?,
        role: row.get(5)?,
        role_note: row.get(6)?,
        created_at: row.get(7)?,
    })
}

impl Database {
    /// Dosyaya taraf ekler (gerekirse yeni aktör açar) ve satırı **geri okur**.
    ///
    /// Tek işlem: katılımcı yazılamazsa yeni açılan aktör de yazılmamış olur.
    pub fn add_case_party(
        &self,
        case_id: &str,
        party: &NewCaseParty<'_>,
    ) -> Result<ParticipantRow, StorageError> {
        let case = case_id.trim();
        if case.is_empty() || !self.case_is_open(case)? {
            return Err(StorageError::NotFound);
        }
        let role = party_role(party.role)?;
        let note = party_note(party.role_note)?;
        let now = now_iso();
        let tx = self.conn().unchecked_transaction()?;
        let requested_actor = party
            .actor_id
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let actor_id = if let Some(actor_id) = requested_actor {
            let state: Option<String> = tx
                .query_row(
                    "SELECT review_state FROM actors WHERE id = ?1 AND deleted_at IS NULL",
                    [actor_id],
                    |row| row.get(0),
                )
                .optional()?;
            match state.as_deref() {
                None => return Err(invalid("seçilen taraf bulunamadı")),
                Some("merged") => {
                    return Err(invalid(
                        "birleştirilmiş tarafa yeni rol eklenemez; hedef tarafı seçin",
                    ))
                }
                Some(_) => actor_id.to_string(),
            }
        } else {
            let name = party_name(party.display_name)?;
            let kind = party_kind(party.kind)?;
            let id = uuid::Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO actors
                    (id, display_name, kind, normalized_names, review_state, created_at, updated_at, row_version)
                 VALUES (?1, ?2, ?3, ?4, 'accepted', ?5, ?5, 1)",
                rusqlite::params![id, name, kind, normalized_names(&name), now],
            )?;
            id
        };
        let participant_id = uuid::Uuid::now_v7().to_string();
        let inserted = tx.execute(
            "INSERT INTO case_participants
                (id, case_id, actor_id, role, role_note, created_at, updated_at, row_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1)",
            rusqlite::params![participant_id, case, actor_id, role, note, now],
        );
        match inserted {
            Ok(_) => {}
            // Tablo UNIQUE (case_id, actor_id, role): ham SQL hatası arayüze
            // sızmaz; kullanıcı "bu rol zaten kayıtlı" görür.
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                return Err(invalid("aynı dosyada aynı taraf için bu rol zaten kayıtlı"))
            }
            Err(error) => return Err(error.into()),
        }
        self.append_audit(
            "case",
            Some(case),
            "party_add",
            "local-user",
            &serde_json::json!({ "actorId": actor_id, "role": role }),
        )?;
        tx.commit()?;
        self.get_participant(&participant_id)?
            .ok_or(StorageError::NotFound)
    }

    fn case_is_open(&self, case_id: &str) -> Result<bool, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT 1 FROM cases WHERE id = ?1 AND deleted_at IS NULL",
                [case_id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false))
    }

    fn get_participant(
        &self,
        participant_id: &str,
    ) -> Result<Option<ParticipantRow>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT p.id, p.case_id, p.actor_id, a.display_name, a.kind,
                        p.role, p.role_note, p.created_at
                 FROM case_participants p
                 JOIN actors a ON a.id = p.actor_id
                 WHERE p.id = ?1 AND p.deleted_at IS NULL",
                [participant_id],
                map_participant,
            )
            .optional()?)
    }

    /// Dosyanın tarafları (kayıt sırası) + bu dosyayı etkileyen birleştirmeler
    /// (en yeni önce). Birleştirme kaydı çözülemezse **satır atlanır**: sicil
    /// taraflar listesini kilitlemez; bozuk kayıt uydurulmaz.
    pub fn list_case_parties(&self, case_id: &str) -> Result<CaseParties, StorageError> {
        let case = case_id.trim();
        if case.is_empty() || !self.case_is_open(case)? {
            return Err(StorageError::NotFound);
        }
        let mut stmt = self.conn().prepare(
            "SELECT p.id, p.case_id, p.actor_id, a.display_name, a.kind,
                    p.role, p.role_note, p.created_at
             FROM case_participants p
             JOIN actors a ON a.id = p.actor_id
             WHERE p.case_id = ?1 AND p.deleted_at IS NULL
             ORDER BY p.created_at ASC, p.rowid ASC
             LIMIT ?2",
        )?;
        let limit = i64::try_from(MAX_PARTY_LIST).unwrap_or(200);
        let participants = stmt
            .query_map(rusqlite::params![case, limit], map_participant)?
            .collect::<Result<Vec<_>, _>>()?;

        let mut merge_stmt = self.conn().prepare(
            "SELECT m.id, m.source_actor_id, s.display_name,
                    m.target_actor_id, t.display_name,
                    m.moved_participants, m.reviewer_label, m.created_at, m.undone_at
             FROM actor_merges m
             JOIN actors s ON s.id = m.source_actor_id
             JOIN actors t ON t.id = m.target_actor_id
             ORDER BY m.created_at DESC, m.rowid DESC
             LIMIT ?1",
        )?;
        let merge_limit = i64::try_from(MAX_ACTOR_MERGE_LIST.saturating_mul(4)).unwrap_or(200);
        let mut merges = Vec::new();
        for row in merge_stmt.query_map([merge_limit], map_merge)? {
            let merge = row?;
            if merge.case_ids.iter().any(|id| id == case) {
                merges.push(merge);
            }
        }
        Ok(CaseParties {
            participants,
            merges,
        })
    }

    /// Dosya rolünü kaldırır (geri dönüşümlü silme: satır `deleted_at` alır).
    pub fn remove_case_participant(&self, participant_id: &str) -> Result<(), StorageError> {
        let id = participant_id.trim();
        let (case_id, role): (String, String) = self
            .conn()
            .query_row(
                "SELECT case_id, role FROM case_participants
                 WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;
        let now = now_iso();
        let changed = self.conn().execute(
            "UPDATE case_participants
             SET deleted_at = ?1, updated_at = ?1, row_version = row_version + 1
             WHERE id = ?2 AND deleted_at IS NULL",
            rusqlite::params![now, id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.append_audit(
            "case",
            Some(&case_id),
            "party_remove",
            "local-user",
            &serde_json::json!({ "participantId": id, "role": role }),
        )?;
        Ok(())
    }

    /// İki tarafı birleştirir: kaynağın rollerini hedefe taşır, kaynağı
    /// `merged` işaretler ve geri alınabilir bir kayıt bırakır.
    pub fn merge_actors(
        &self,
        source_actor_id: &str,
        target_actor_id: &str,
        reviewer: &str,
    ) -> Result<ActorMergeRow, StorageError> {
        let source = source_actor_id.trim();
        let target = target_actor_id.trim();
        let reviewer = reviewer.trim();
        if reviewer.is_empty() {
            return Err(invalid("birleştiren kimliği boş olamaz"));
        }
        if source.is_empty() || target.is_empty() {
            return Err(invalid("kaynak ve hedef zorunlu"));
        }
        if source == target {
            return Err(invalid("kaynak ve hedef aynı taraf olamaz"));
        }
        let tx = self.conn().unchecked_transaction()?;
        let source_state: Option<String> = tx
            .query_row(
                "SELECT review_state FROM actors WHERE id = ?1 AND deleted_at IS NULL",
                [source],
                |row| row.get(0),
            )
            .optional()?;
        let previous_state = match source_state.as_deref() {
            None => return Err(StorageError::NotFound),
            Some("merged") => return Err(invalid("kaynak taraf zaten birleştirilmiş")),
            Some(state) => state.to_string(),
        };
        let target_state: Option<String> = tx
            .query_row(
                "SELECT review_state FROM actors WHERE id = ?1 AND deleted_at IS NULL",
                [target],
                |row| row.get(0),
            )
            .optional()?;
        match target_state.as_deref() {
            None => return Err(StorageError::NotFound),
            Some("merged") => return Err(invalid("hedef taraf birleştirilmiş bir kayıt olamaz")),
            Some(_) => {}
        }
        // Aynı dosyada aynı rol iki tarafta da varsa birleştirme sessizce
        // yinelenen rol üretmemeli: reddedilir, kullanıcı önce yineleneni siler.
        let conflicts: i64 = tx.query_row(
            "SELECT COUNT(*) FROM case_participants s
             JOIN case_participants t ON t.case_id = s.case_id AND t.role = s.role
             WHERE s.actor_id = ?1 AND t.actor_id = ?2
               AND s.deleted_at IS NULL AND t.deleted_at IS NULL",
            rusqlite::params![source, target],
            |row| row.get(0),
        )?;
        if conflicts > 0 {
            return Err(invalid(
                "aynı dosyada aynı rol iki tarafta da kayıtlı; önce yinelenen rolü silin",
            ));
        }
        let mut moved = Vec::new();
        {
            let mut stmt = tx.prepare(
                "SELECT id, case_id FROM case_participants
                 WHERE actor_id = ?1 AND deleted_at IS NULL
                 ORDER BY created_at ASC, rowid ASC",
            )?;
            let rows = stmt.query_map([source], |row| {
                Ok(MovedParticipant {
                    id: row.get(0)?,
                    case_id: row.get(1)?,
                })
            })?;
            for row in rows {
                moved.push(row?);
            }
        }
        let moved_json = serde_json::to_string(&moved).unwrap_or_else(|_| "[]".to_string());
        let merge_id = uuid::Uuid::now_v7().to_string();
        let now = now_iso();
        tx.execute(
            "INSERT INTO actor_merges
                (id, source_actor_id, target_actor_id, source_previous_state,
                 moved_participants, reviewer_label, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                merge_id,
                source,
                target,
                previous_state,
                moved_json,
                reviewer,
                now
            ],
        )?;
        tx.execute(
            "UPDATE case_participants
             SET actor_id = ?1, updated_at = ?2, row_version = row_version + 1
             WHERE actor_id = ?3 AND deleted_at IS NULL",
            rusqlite::params![target, now, source],
        )?;
        tx.execute(
            "UPDATE actors
             SET review_state = 'merged', merged_into_actor_id = ?1,
                 updated_at = ?2, row_version = row_version + 1
             WHERE id = ?3",
            rusqlite::params![target, now, source],
        )?;
        self.append_audit(
            "actor",
            Some(source),
            "merge",
            reviewer,
            &serde_json::json!({ "targetActorId": target, "movedCount": moved.len() }),
        )?;
        tx.commit()?;
        self.get_merge(&merge_id)?.ok_or(StorageError::NotFound)
    }

    /// Birleştirmeyi geri alır: taşınan roller kaynağa döner, kaynak **önceki**
    /// inceleme durumuna yazılır. Birleştirmeden sonra **silinmiş** roller
    /// geri gelmez (silme bilinçli kullanıcı eylemidir; uydurulmaz).
    pub fn undo_actor_merge(&self, merge_id: &str) -> Result<ActorMergeRow, StorageError> {
        let id = merge_id.trim();
        let tx = self.conn().unchecked_transaction()?;
        let (source, target, previous_state, moved_json, undone_at): (
            String,
            String,
            String,
            String,
            Option<String>,
        ) = tx
            .query_row(
                "SELECT source_actor_id, target_actor_id, source_previous_state,
                        moved_participants, undone_at
                 FROM actor_merges WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;
        if undone_at.is_some() {
            return Err(invalid("birleştirme zaten geri alınmış"));
        }
        let state: Option<String> = tx
            .query_row(
                "SELECT review_state FROM actors WHERE id = ?1 AND deleted_at IS NULL",
                [&source],
                |row| row.get(0),
            )
            .optional()?;
        match state.as_deref() {
            None => return Err(invalid("kaynak taraf artık yok; geri alınamaz")),
            Some("merged") => {}
            Some(_) => return Err(invalid("kaynak taraf birleştirilmiş durumda değil")),
        }
        let moved: Vec<MovedParticipant> = serde_json::from_str(&moved_json)
            .map_err(|_| invalid("birleştirme kaydı okunamadı; geri alınmadı"))?;
        let now = now_iso();
        for item in &moved {
            tx.execute(
                "UPDATE case_participants
                 SET actor_id = ?1, updated_at = ?2, row_version = row_version + 1
                 WHERE id = ?3 AND actor_id = ?4 AND deleted_at IS NULL",
                rusqlite::params![source, now, item.id, target],
            )?;
        }
        tx.execute(
            "UPDATE actors
             SET review_state = ?1, merged_into_actor_id = NULL,
                 updated_at = ?2, row_version = row_version + 1
             WHERE id = ?3",
            rusqlite::params![previous_state, now, source],
        )?;
        tx.execute(
            "UPDATE actor_merges SET undone_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;
        self.append_audit(
            "actor",
            Some(&source),
            "merge_undo",
            "local-user",
            &serde_json::json!({ "mergeId": id, "targetActorId": target }),
        )?;
        tx.commit()?;
        self.get_merge(id)?.ok_or(StorageError::NotFound)
    }

    fn get_merge(&self, merge_id: &str) -> Result<Option<ActorMergeRow>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT m.id, m.source_actor_id, s.display_name,
                        m.target_actor_id, t.display_name,
                        m.moved_participants, m.reviewer_label, m.created_at, m.undone_at
                 FROM actor_merges m
                 JOIN actors s ON s.id = m.source_actor_id
                 JOIN actors t ON t.id = m.target_actor_id
                 WHERE m.id = ?1",
                [merge_id],
                map_merge,
            )
            .optional()?)
    }
}

fn map_merge(row: &Row<'_>) -> rusqlite::Result<ActorMergeRow> {
    let moved_json: String = row.get(5)?;
    let moved: Vec<MovedParticipant> = serde_json::from_str(&moved_json).unwrap_or_default();
    let mut case_ids: Vec<String> = Vec::new();
    for item in &moved {
        if !case_ids.contains(&item.case_id) {
            case_ids.push(item.case_id.clone());
        }
    }
    Ok(ActorMergeRow {
        id: row.get(0)?,
        source_actor_id: row.get(1)?,
        source_display_name: row.get(2)?,
        target_actor_id: row.get(3)?,
        target_display_name: row.get(4)?,
        moved_participant_ids: moved.iter().map(|item| item.id.clone()).collect(),
        case_ids,
        reviewer_label: row.get(6)?,
        created_at: row.get(7)?,
        undone_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseKey, NewCase};

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([47u8; 32])).expect("db")
    }

    fn case(db: &Database, title: &str) -> String {
        db.create_case(&NewCase {
            title: title.to_string(),
            case_kind: crate::CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: crate::Sensitivity::Normal,
            default_ai_policy: crate::AiPolicy::Off,
        })
        .expect("dosya")
        .id
    }

    fn party<'a>(
        name: &'a str,
        kind: &'a str,
        role: &'a str,
        note: Option<&'a str>,
    ) -> NewCaseParty<'a> {
        NewCaseParty {
            display_name: name,
            kind,
            role,
            role_note: note,
            actor_id: None,
        }
    }

    #[test]
    fn party_is_added_and_read_back_with_trimmed_fields() {
        let db = db();
        let case_id = case(&db, "Kira");
        let row = db
            .add_case_party(
                &case_id,
                &party(
                    "  Ayşe Yılmaz  ",
                    " person ",
                    "  davacı  ",
                    Some("  vekil  "),
                ),
            )
            .expect("taraf");
        assert_eq!(row.display_name, "Ayşe Yılmaz");
        assert_eq!(row.kind, "person");
        assert_eq!(row.role, "davacı");
        assert_eq!(row.role_note.as_deref(), Some("vekil"));
        let parties = db.list_case_parties(&case_id).expect("liste");
        assert_eq!(parties.participants.len(), 1);
        assert!(parties.merges.is_empty());
    }

    #[test]
    fn blank_or_unknown_fields_are_rejected_without_writing() {
        let db = db();
        let case_id = case(&db, "Kira");
        for bad in [
            party("   ", "person", "davacı", None),
            party("Ayşe", "robot", "davacı", None),
            party("Ayşe", "person", "  ", None),
        ] {
            let err = db.add_case_party(&case_id, &bad).expect_err("reddedilmeli");
            assert!(matches!(err, StorageError::ActorInvalid { .. }));
        }
        assert!(db
            .list_case_parties(&case_id)
            .expect("liste")
            .participants
            .is_empty());
    }

    #[test]
    fn duplicate_role_is_reported_as_actor_error_not_sql() {
        let db = db();
        let case_id = case(&db, "Kira");
        let row = db
            .add_case_party(&case_id, &party("Ayşe", "person", "davacı", None))
            .expect("ilk");
        let mut second = party("Ayşe", "person", "davacı", None);
        second.actor_id = Some(&row.actor_id);
        let err = db
            .add_case_party(&case_id, &second)
            .expect_err("yinelenmez");
        assert_eq!(err.code(), "E_DB_ACTOR");
        assert_eq!(
            db.list_case_parties(&case_id)
                .expect("liste")
                .participants
                .len(),
            1
        );
    }

    #[test]
    fn same_person_can_hold_two_roles_in_one_case() {
        let db = db();
        let case_id = case(&db, "Kira");
        let first = db
            .add_case_party(&case_id, &party("Ayşe", "person", "davacı", None))
            .expect("1");
        let mut second = party("Ayşe", "person", "tanık", None);
        second.actor_id = Some(&first.actor_id);
        let row = db.add_case_party(&case_id, &second).expect("2");
        assert_eq!(row.actor_id, first.actor_id);
        assert_eq!(
            db.list_case_parties(&case_id)
                .expect("liste")
                .participants
                .len(),
            2
        );
    }

    #[test]
    fn merge_moves_roles_and_undo_restores_them() {
        let db = db();
        let case_id = case(&db, "Kira");
        let source = db
            .add_case_party(&case_id, &party("A. Yılmaz", "person", "davacı", None))
            .expect("kaynak");
        let target = db
            .add_case_party(&case_id, &party("Ayşe Yılmaz", "person", "tanık", None))
            .expect("hedef");
        let merge = db
            .merge_actors(&source.actor_id, &target.actor_id, "local-user")
            .expect("birleştir");
        assert_eq!(merge.moved_participant_ids, vec![source.id.clone()]);
        assert_eq!(merge.case_ids, vec![case_id.clone()]);
        let after = db.list_case_parties(&case_id).expect("liste");
        assert_eq!(after.participants.len(), 2);
        assert!(after
            .participants
            .iter()
            .all(|row| row.actor_id == target.actor_id));
        assert_eq!(after.merges.len(), 1);
        assert!(after.merges[0].undone_at.is_none());

        let undone = db.undo_actor_merge(&merge.id).expect("geri al");
        assert!(undone.undone_at.is_some());
        let restored = db.list_case_parties(&case_id).expect("liste");
        let roles: Vec<(String, String)> = restored
            .participants
            .iter()
            .map(|row| (row.actor_id.clone(), row.role.clone()))
            .collect();
        assert!(roles.contains(&(source.actor_id.clone(), "davacı".to_string())));
        assert!(roles.contains(&(target.actor_id.clone(), "tanık".to_string())));
        // Kaynak tekrar kullanılabilir: `merged` durumu geri alındı.
        let mut again = party("A. Yılmaz", "person", "tanık", None);
        again.actor_id = Some(&source.actor_id);
        db.add_case_party(&case_id, &again)
            .expect("kaynak yeniden açık");
    }

    #[test]
    fn merge_refuses_conflicting_role_in_same_case() {
        let db = db();
        let case_id = case(&db, "Kira");
        let a = db
            .add_case_party(&case_id, &party("A", "person", "davacı", None))
            .expect("a");
        let b = db
            .add_case_party(&case_id, &party("B", "person", "davacı", None))
            .expect("b");
        let err = db
            .merge_actors(&a.actor_id, &b.actor_id, "local-user")
            .expect_err("çakışma reddedilir");
        assert_eq!(err.code(), "E_DB_ACTOR");
        // Hiçbir şey taşınmadı, kaynak birleştirilmedi.
        let rows = db.list_case_parties(&case_id).expect("liste").participants;
        assert!(rows.iter().any(|row| row.actor_id == a.actor_id));
        assert!(db.undo_actor_merge("yok").is_err());
    }

    #[test]
    fn merged_actor_cannot_receive_new_roles() {
        let db = db();
        let case_id = case(&db, "Kira");
        let source = db
            .add_case_party(&case_id, &party("A", "person", "davacı", None))
            .expect("a");
        let target = db
            .add_case_party(&case_id, &party("B", "person", "tanık", None))
            .expect("b");
        db.merge_actors(&source.actor_id, &target.actor_id, "local-user")
            .expect("birleştir");
        let mut extra = party("A", "person", "bilirkişi", None);
        extra.actor_id = Some(&source.actor_id);
        let err = db.add_case_party(&case_id, &extra).expect_err("reddedilir");
        assert_eq!(err.code(), "E_DB_ACTOR");
    }

    #[test]
    fn removed_participant_leaves_the_list_but_audit_remains() {
        let db = db();
        let case_id = case(&db, "Kira");
        let row = db
            .add_case_party(&case_id, &party("Ayşe", "person", "davacı", None))
            .expect("taraf");
        db.remove_case_participant(&row.id).expect("kaldır");
        assert!(db
            .list_case_parties(&case_id)
            .expect("liste")
            .participants
            .is_empty());
        let events = db.list_audit_events(50, Some("case")).expect("defter");
        assert!(events.iter().any(|event| event.operation == "party_remove"));
    }

    #[test]
    fn parties_are_listed_in_recording_order() {
        let db = db();
        let case_id = case(&db, "Kira");
        for role in ["davacı", "davalı", "tanık"] {
            db.add_case_party(&case_id, &party(role, "person", role, None))
                .expect("taraf");
        }
        let roles: Vec<String> = db
            .list_case_parties(&case_id)
            .expect("liste")
            .participants
            .into_iter()
            .map(|row| row.role)
            .collect();
        assert_eq!(roles, vec!["davacı", "davalı", "tanık"]);
    }

    #[test]
    fn merge_history_is_scoped_to_the_case() {
        let db = db();
        let first = case(&db, "Birinci");
        let second = case(&db, "İkinci");
        let a = db
            .add_case_party(&first, &party("A", "person", "davacı", None))
            .expect("a");
        let b = db
            .add_case_party(&first, &party("B", "person", "tanık", None))
            .expect("b");
        db.merge_actors(&a.actor_id, &b.actor_id, "local-user")
            .expect("birleştir");
        assert_eq!(db.list_case_parties(&first).expect("1").merges.len(), 1);
        assert!(db.list_case_parties(&second).expect("2").merges.is_empty());
    }
}
