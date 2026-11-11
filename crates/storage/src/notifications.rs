//! Bildirim outbox'ının **teslim** defteri (şartname REQ-028, F08/F14).
//!
//! `outbox_events` iki sınıf olay taşır:
//! - **bildirim** — kullanıcıya gösterilecek yerel bildirim (`task.reminder_due`),
//! - **alan olayı** — başka tüketiciler için yayın (`review.decision_committed`,
//!   `review.item_decided`).
//!
//! Ölçülen kusur: `delivered_at`ı yazan **hiçbir** kod yoktu. Her satır
//! `delivered_at IS NULL` kalıyordu; arayüz de iki sınıfı ayırmadan hepsini
//! birden "bildirim bekliyor" diye sayıyordu. Sonuç: alan olayları bildirim
//! gibi görünüyordu ve **hiçbir sayı hiçbir zaman düşmüyordu** — kullanıcı
//! hiç gelmeyecek bir bildirimi bekliyordu.
//!
//! Bu modül teslimi ölçülebilir yapar:
//! - [`pending_notifications`] **yalnız** bildirim sınıfını okur,
//! - [`notification_text`] metni yalnız **politikadan** üretir: görev başlığı,
//!   dosya adı, taraf adı ve belge metni bildirime **girmez** (şartname §331;
//!   kilit ekranı bildirimi genel metindir),
//! - [`mark_notifications_delivered`] yalnız gönderen bileşenin kabul ettiği
//!   satırları kapatır (koşullu `UPDATE`; ikinci çağrı **0** yazar),
//! - [`record_notification_failures`] denemeyi **sayar** ama satırı açmaz:
//!   gönderilemeyen bildirim "gönderildi" görünmez.
//!
//! Alan olayları bu defterin kapsamı **dışındadır**: teslim edilebilir
//! bildirim değildirler ve `delivered_at` ile kapatılmazlar.

use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::tasks::EVENT_TASK_REMINDER;
use crate::{now_iso, Database, StorageError};

/// Bildirim olarak **gösterilebilen** olay türleri (kapalı küme).
///
/// Yeni bir bildirim türü eklemek bilinçli bir karardır: metni gizlilik
/// sözleşmesine uymalı ve bu modülde test edilmelidir. Bilinmeyen tür
/// "bildirim" sayılmaz — sessizce kullanıcıya gösterilmez.
pub const NOTIFICATION_EVENT_TYPES: [&str; 1] = [EVENT_TASK_REMINDER];

/// Tek çağrıda teslim edilecek en fazla bildirim. OS bildirim merkezini tek
/// seferde onlarca satırla doldurmamak için sınır **çekirdekte** uygulanır
/// (arayüz kırpamaz; kalan satırlar bir sonraki çağrıda gider).
pub const MAX_NOTIFICATIONS_PER_CALL: usize = 50;

/// Kuyrukta bekleyen tek bildirim.
///
/// `policy` payload'tan okunur ve **yalnız** metin seçiminde kullanılır;
/// payload çözülemezse `None` olur ve satır yine döner (kayıt kaybolmaz,
/// metin genel kalır). Görev kimliği/başlığı bu yapıya **alınmaz**: bildirim
/// metni üretmek için gerekli değildir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingNotification {
    pub id: String,
    pub event_type: String,
    pub policy: Option<String>,
    pub created_at: String,
    pub attempt_count: i64,
}

/// Bildirim başlığı ve gövdesi.
///
/// **Gizlilik sözleşmesi:** metin yalnız zaman ifadesi taşır. Görev başlığı,
/// dosya adı, taraf/kişi adı, belge metni veya kimlik **asla** girmez
/// (şartname §331: kilitli uygulama bildirimde dosya adı, taraf adı veya metin
/// göstermez). Bu yüzden metin payload'tan değil **politikadan** üretilir;
/// bilinmeyen politika genel cümleye düşer.
pub fn notification_text(policy: Option<&str>) -> (&'static str, &'static str) {
    const TITLE: &str = "Görev hatırlatıcısı";
    match policy {
        Some("on_day") => (TITLE, "Bugün son günü olan bir göreviniz var."),
        Some("day_before") => (TITLE, "Yarın son günü olan bir göreviniz var."),
        _ => (TITLE, "Süresi yaklaşan bir göreviniz var."),
    }
}

/// Bildirim türlerini SQL `IN` listesine çevirir. Değerler **kendi sabitimiz**
/// olduğu için tırnak kaçırma yeterlidir; kullanıcı girdisi buraya gelmez.
fn event_type_list() -> String {
    NOTIFICATION_EVENT_TYPES
        .iter()
        .map(|t| format!("'{}'", t.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",")
}

fn map_pending(row: &Row<'_>) -> rusqlite::Result<PendingNotification> {
    let payload: String = row.get(2)?;
    let policy = serde_json::from_str::<serde_json::Value>(&payload)
        .ok()
        .and_then(|v| v.get("policy").and_then(|p| p.as_str()).map(str::to_string));
    Ok(PendingNotification {
        id: row.get(0)?,
        event_type: row.get(1)?,
        policy,
        created_at: row.get(3)?,
        attempt_count: row.get(4)?,
    })
}

impl Database {
    /// Teslim edilmemiş **bildirim** satırları (alan olayları hariç).
    ///
    /// Sıra `created_at ASC, rowid ASC`: aynı saniyede yazılan satırlar da
    /// yazılma sırasıyla gelir. `limit` çekirdekte kırpılır.
    pub fn pending_notifications(
        &self,
        case_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PendingNotification>, StorageError> {
        let limit = limit.clamp(1, MAX_NOTIFICATIONS_PER_CALL);
        let mut sql = format!(
            "SELECT id,event_type,payload,created_at,attempt_count
             FROM outbox_events
             WHERE delivered_at IS NULL AND event_type IN ({})",
            event_type_list()
        );
        if case_id.is_some() {
            sql.push_str(" AND case_id=?1");
        }
        sql.push_str(&format!(
            " ORDER BY created_at ASC, rowid ASC LIMIT {limit}"
        ));
        let mut stmt = self.conn().prepare(&sql)?;
        let mut out = Vec::new();
        if let Some(case_id) = case_id {
            for row in stmt.query_map([case_id], map_pending)? {
                out.push(row?);
            }
        } else {
            for row in stmt.query_map([], map_pending)? {
                out.push(row?);
            }
        }
        Ok(out)
    }

    /// Teslim edilmemiş bildirim sayısı (alan olayları sayılmaz).
    pub fn pending_notification_count(&self, case_id: Option<&str>) -> Result<i64, StorageError> {
        let sql = format!(
            "SELECT count(*) FROM outbox_events
             WHERE delivered_at IS NULL AND event_type IN ({}){}",
            event_type_list(),
            if case_id.is_some() {
                " AND case_id=?1"
            } else {
                ""
            }
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let count: i64 = if let Some(case_id) = case_id {
            stmt.query_row([case_id], |row| row.get(0))?
        } else {
            stmt.query_row([], |row| row.get(0))?
        };
        Ok(count)
    }

    /// Bildirimleri **teslim edildi** olarak kapatır ve gerçekten değişen satır
    /// sayısını döndürür.
    ///
    /// Koşullu `UPDATE` (`delivered_at IS NULL`): aynı kimlik ikinci kez
    /// gelirse satır ezilmez, sayı 0 artar — teslim anı korunur. Alan olayı
    /// kimliği verilirse **hiçbir şey değişmez** (bildirim değildir).
    /// Boş liste hata değildir (gönderilecek bir şey yoktu).
    pub fn mark_notifications_delivered(
        &self,
        ids: &[String],
        delivered_at: &str,
    ) -> Result<usize, StorageError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let delivered_at = delivered_at.trim();
        if delivered_at.is_empty() || delivered_at.len() > 40 {
            return Err(StorageError::InvalidSetting);
        }
        let sql = format!(
            "UPDATE outbox_events SET delivered_at=?1
             WHERE id=?2 AND delivered_at IS NULL AND event_type IN ({})",
            event_type_list()
        );
        let tx = self.conn().unchecked_transaction()?;
        let mut changed = 0usize;
        {
            let mut stmt = tx.prepare(&sql)?;
            for id in ids {
                changed += stmt.execute(rusqlite::params![delivered_at, id])?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Gönderilemeyen denemeyi **sayar**: `attempt_count` artar, `delivered_at`
    /// **açılmaz**. Böylece satır "gönderildi" görünmez ve bir sonraki çağrıda
    /// yeniden denenir. Değişen satır sayısı döner.
    pub fn record_notification_failures(&self, ids: &[String]) -> Result<usize, StorageError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let sql = format!(
            "UPDATE outbox_events SET attempt_count = attempt_count + 1
             WHERE id=?1 AND delivered_at IS NULL AND event_type IN ({})",
            event_type_list()
        );
        let tx = self.conn().unchecked_transaction()?;
        let mut changed = 0usize;
        {
            let mut stmt = tx.prepare(&sql)?;
            for id in ids {
                changed += stmt.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }
}

/// `now_iso()` yeniden dışa aktarılır: teslim damgası çağıran tarafta
/// uydurulmasın, çekirdeğin saat biçimi kullanılsın.
pub fn delivery_stamp() -> String {
    now_iso()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::tasks::NewTask;
    use crate::{AiPolicy, DatabaseKey, Sensitivity};

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([41u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Ayşe Yılmaz — kira uyuşmazlığı".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: AiPolicy::Off,
            })
            .expect("case");
        (db, case.id)
    }

    fn confirmed_task(
        db: &Database,
        case_id: &str,
        title: &str,
        date: &str,
        policy: &str,
    ) -> String {
        let task = db
            .create_task(
                case_id,
                &NewTask {
                    title: title.into(),
                    user_due_date: Some(date.into()),
                    date_timezone: Some("Europe/Istanbul".into()),
                    assignee_label: None,
                    reminder_policy: policy.into(),
                },
            )
            .expect("görev");
        db.confirm_task_date(&task.id).expect("onay");
        task.id
    }

    /// Alan olayı yazan diğer yol (`commit_review_decision`) burada kurulum
    /// gerektirdiği için satır doğrudan yazılır: ölçülen şey **okuma**
    /// tarafının iki sınıfı ayırmasıdır, olayın üretim yolu değil.
    fn insert_domain_event(db: &Database, case_id: &str, id: &str) {
        db.conn()
            .execute(
                "INSERT INTO outbox_events
                   (id,aggregate_id,case_id,event_type,payload_version,payload,created_at,delivered_at,attempt_count)
                 VALUES (?1,'agg',?2,'review.decision_committed',1,'{\"targetId\":\"x\"}',?3,NULL,0)",
                rusqlite::params![id, case_id, now_iso()],
            )
            .expect("alan olayı");
    }

    #[test]
    fn pending_notifications_ignore_domain_events() {
        let (db, case_id) = setup();
        confirmed_task(&db, &case_id, "Dilekçe son gün", "2026-11-11", "on_day");
        db.enqueue_task_reminders(&case_id, "2026-11-11")
            .expect("kuyruk");
        insert_domain_event(&db, &case_id, "outbox:decision-1");

        let pending = db.pending_notifications(Some(&case_id), 50).expect("liste");
        assert_eq!(pending.len(), 1, "yalnız hatırlatıcı bildirim sayılır");
        assert_eq!(pending[0].event_type, EVENT_TASK_REMINDER);
        assert_eq!(pending[0].policy.as_deref(), Some("on_day"));
        // Alan olayı kuyrukta duruyor ama bildirim sayılmıyor.
        assert_eq!(
            db.pending_notification_count(Some(&case_id)).expect("sayı"),
            1
        );
    }

    #[test]
    fn delivered_marking_is_conditional_and_idempotent() {
        let (db, case_id) = setup();
        confirmed_task(&db, &case_id, "X", "2026-11-11", "on_day");
        db.enqueue_task_reminders(&case_id, "2026-11-11")
            .expect("kuyruk");
        let ids: Vec<String> = db
            .pending_notifications(Some(&case_id), 50)
            .expect("liste")
            .into_iter()
            .map(|n| n.id)
            .collect();

        assert_eq!(
            db.mark_notifications_delivered(&ids, "2026-11-11T09:00:00Z")
                .expect("teslim"),
            1
        );
        // İkinci çağrı satırı ezmez: teslim anı korunur.
        assert_eq!(
            db.mark_notifications_delivered(&ids, "2026-11-12T09:00:00Z")
                .expect("teslim"),
            0
        );
        assert!(db
            .pending_notifications(Some(&case_id), 50)
            .expect("liste")
            .is_empty());
        assert_eq!(
            db.pending_notification_count(Some(&case_id)).expect("sayı"),
            0
        );
    }

    #[test]
    fn domain_event_cannot_be_marked_delivered() {
        let (db, case_id) = setup();
        insert_domain_event(&db, &case_id, "outbox:decision-2");
        let changed = db
            .mark_notifications_delivered(
                &["outbox:decision-2".to_string()],
                "2026-11-11T09:00:00Z",
            )
            .expect("teslim");
        assert_eq!(changed, 0, "alan olayı bildirim gibi kapatılamaz");
        // Satır hâlâ teslim edilmemiş durumda.
        let rows = db.list_outbox_events(Some(&case_id), true).expect("outbox");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].delivered_at.is_none());
    }

    #[test]
    fn failed_delivery_counts_attempt_but_stays_pending() {
        let (db, case_id) = setup();
        confirmed_task(&db, &case_id, "X", "2026-11-11", "on_day");
        db.enqueue_task_reminders(&case_id, "2026-11-11")
            .expect("kuyruk");
        let ids: Vec<String> = db
            .pending_notifications(Some(&case_id), 50)
            .expect("liste")
            .into_iter()
            .map(|n| n.id)
            .collect();

        assert_eq!(db.record_notification_failures(&ids).expect("deneme"), 1);
        let pending = db.pending_notifications(Some(&case_id), 50).expect("liste");
        assert_eq!(pending.len(), 1, "başarısız deneme satırı kapatmaz");
        assert_eq!(pending[0].attempt_count, 1);
    }

    #[test]
    fn notification_text_never_leaks_task_case_or_document() {
        let (_db, _case_id) = setup();
        for policy in [Some("on_day"), Some("day_before"), None, Some("bilinmeyen")] {
            let (title, body) = notification_text(policy);
            let text = format!("{title} {body}");
            for forbidden in [
                "Ayşe",
                "Yılmaz",
                "kira",
                "uyuşmazlığı",
                "Dilekçe",
                "task-",
                "outbox:",
                "Ayşe Yılmaz — kira uyuşmazlığı",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "bildirim metni «{forbidden}» taşımamalı: {text}"
                );
            }
            assert!(!title.is_empty() && !body.is_empty());
        }
        assert_eq!(
            notification_text(Some("on_day")).1,
            "Bugün son günü olan bir göreviniz var."
        );
        assert_eq!(
            notification_text(Some("day_before")).1,
            "Yarın son günü olan bir göreviniz var."
        );
    }

    #[test]
    fn unreadable_payload_keeps_row_and_falls_back_to_generic_text() {
        let (db, case_id) = setup();
        // Bozuk payload: satır düşürülmez, politika okunamaz.
        db.conn()
            .execute(
                "INSERT INTO outbox_events
                   (id,aggregate_id,case_id,event_type,payload_version,payload,created_at,delivered_at,attempt_count)
                 VALUES ('task-reminder:bozuk','agg',?1,?2,1,'{bozuk',?3,NULL,0)",
                rusqlite::params![case_id, EVENT_TASK_REMINDER, now_iso()],
            )
            .expect("bozuk satır");
        let pending = db.pending_notifications(Some(&case_id), 50).expect("liste");
        assert_eq!(pending.len(), 1, "bozuk payload satırı kaybettirmez");
        assert!(pending[0].policy.is_none());
        let (_, body) = notification_text(pending[0].policy.as_deref());
        assert_eq!(body, "Süresi yaklaşan bir göreviniz var.");
    }

    #[test]
    fn limit_is_capped_and_rows_are_ordered() {
        let (db, case_id) = setup();
        for i in 0..(MAX_NOTIFICATIONS_PER_CALL + 5) {
            db.conn()
                .execute(
                    "INSERT INTO outbox_events
                       (id,aggregate_id,case_id,event_type,payload_version,payload,created_at,delivered_at,attempt_count)
                     VALUES (?1,'agg',?2,?3,1,'{\"policy\":\"on_day\"}',?4,NULL,0)",
                    rusqlite::params![
                        format!("task-reminder:{i:03}"),
                        case_id,
                        EVENT_TASK_REMINDER,
                        format!("2026-11-11T09:00:{:02}Z", i % 60)
                    ],
                )
                .expect("satır");
        }
        let pending = db
            .pending_notifications(Some(&case_id), 10_000)
            .expect("liste");
        assert_eq!(
            pending.len(),
            MAX_NOTIFICATIONS_PER_CALL,
            "sınır çekirdekte"
        );
        assert!(pending[0].id.ends_with("000"), "sıra yazılma sırasıdır");
    }

    #[test]
    fn pending_is_scoped_by_case() {
        let (db, case_id) = setup();
        let other = db
            .create_case(&NewCase {
                title: "Başka dosya".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: AiPolicy::Off,
            })
            .expect("case2");
        confirmed_task(&db, &case_id, "A", "2026-11-11", "on_day");
        confirmed_task(&db, &other.id, "B", "2026-11-11", "on_day");
        db.enqueue_task_reminders(&case_id, "2026-11-11")
            .expect("k1");
        db.enqueue_task_reminders(&other.id, "2026-11-11")
            .expect("k2");

        assert_eq!(
            db.pending_notifications(Some(&case_id), 50)
                .expect("liste")
                .len(),
            1
        );
        assert_eq!(
            db.pending_notifications(Some(&other.id), 50)
                .expect("liste")
                .len(),
            1
        );
        assert_eq!(db.pending_notification_count(None).expect("tümü"), 2);
    }

    #[test]
    fn empty_id_list_is_not_an_error() {
        let (db, _) = setup();
        assert_eq!(
            db.mark_notifications_delivered(&[], "2026-11-11T09:00:00Z")
                .expect("boş"),
            0
        );
        assert_eq!(db.record_notification_failures(&[]).expect("boş"), 0);
    }
}
