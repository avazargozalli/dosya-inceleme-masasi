//! F08-T12 — AI ve internet kapalı uçtan uca manuel inceleme koşusu.
//!
//! Akış gerçek bileşenlerle yürür: şifreli kasa → içe aktarma (blob AEAD) →
//! ayrıştırma kaydı → arama → kaynak anchor → iddia → delil bağı → inceleme
//! kararı (atomik hedef kayıt + outbox) → görev ve yerel hatırlatıcı.
//!
//! Ayrıştırma çıktısı bu testte `store_pages` ile verilir; gerçek PDF/DOCX
//! ayrıştırması Python worker'ının sorumluluğundadır ve `workers/document/tests`
//! ile F06 kapsamında ölçülür. Buradaki amaç, AI **kapalıyken** manuel inceleme
//! akışının veri bütünlüğünü uçtan uca kanıtlamaktır.
//!
//! Ağ: hiçbir sağlayıcı profili oluşturulmaz, `ai_runs`/`transfer_consents`
//! kaydı yazılmaz; test sonunda bunlar ayrıca doğrulanır.

use std::path::Path;

use dim_ingest::{import_path, ImportResult};
use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, NewAnchor, NewCase, NewClaim, NewEvidenceLink,
    NewPage, NewReviewItem, NewTask, ReviewTarget, SegmentBlock, Sensitivity,
    EVENT_DECISION_COMMITTED, EVENT_TASK_REMINDER,
};
use dim_vault::{UnlockedVault, VaultFile};

const PAGE_TEXT: &str = "Kira bedeli ödenmedi. İhtar 12.03.2026 tarihinde tebliğ edildi.";
const QUOTE: &str = "Kira bedeli ödenmedi.";

fn setup(dir: &Path) -> (Database, UnlockedVault, String) {
    let (_file, _recovery, vault) =
        VaultFile::create(&dir.join("kasa.dimvault"), "parola-uzun-1234").expect("kasa");
    let key = DatabaseKey::derive_from_master(vault.master_key(), "e2e-kasa").expect("anahtar");
    let db = Database::open(&dir.join("vault.db"), &key).expect("veritabanı");
    let case = db
        .create_case(&NewCase {
            title: "Kurgu Kira Dosyası".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: Some("Kira Hukuku".into()),
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");
    (db, vault, case.id)
}

fn count(db: &Database, sql: &str) -> i64 {
    db.conn()
        .query_row(sql, [], |row| row.get(0))
        .expect("sayım")
}

#[test]
fn manual_review_runs_end_to_end_with_ai_off() {
    let dir = tempfile::tempdir().expect("dizin");
    let (db, vault, case_id) = setup(dir.path());
    assert_eq!(db.schema_version(), dim_storage::CURRENT_SCHEMA_VERSION);

    // 1) İçe aktarma: içerik türü uzantıdan değil baytlardan doğrulanır.
    let doc_path = dir.path().join("kira-inceleme.txt");
    std::fs::write(&doc_path, PAGE_TEXT.as_bytes()).expect("kurgu belge");
    let blob_dir = dir.path().join("blobs");
    let imported = import_path(&db, &vault, &blob_dir, &case_id, &doc_path, false).expect("içe al");
    let (document, version) = match imported {
        ImportResult::Imported { document, version } => (document, version),
        other => panic!("ilk içe aktarma yeni sürüm olmalı: {other:?}"),
    };
    assert_eq!(document.case_id, case_id);
    assert_eq!(document.document_type, "text");

    // Aynı baytlar ikinci kez yeni sürüm üretmez.
    match import_path(&db, &vault, &blob_dir, &case_id, &doc_path, false).expect("tekrar") {
        ImportResult::Duplicate { version_id, .. } => assert_eq!(version_id, version.id),
        other => panic!("beklenen yinelenen içe aktarma: {other:?}"),
    }

    // 2) Blob şifreli: diskte düz metin yok, doğru AAD ile geri okunur.
    let mut blobs: Vec<_> = std::fs::read_dir(&blob_dir)
        .expect("blob dizini")
        .map(|entry| entry.expect("girdi").path())
        .collect();
    assert_eq!(blobs.len(), 1);
    let blob_path = blobs.pop().expect("blob");
    let cipher = std::fs::read(&blob_path).expect("şifreli blob");
    assert!(
        !cipher.windows(4).any(|window| window == b"Kira"),
        "şifreli blob düz metin taşımamalı"
    );
    let aad = format!("{case_id}:{}", version.original_sha256);
    let plain = vault.read_blob(&blob_path, aad.as_bytes()).expect("çöz");
    assert_eq!(plain, PAGE_TEXT.as_bytes());

    // 3) Ayrıştırma kaydı (worker çıktısı burada doğrudan yazılır).
    let run = db
        .start_parse_run(
            &version.id,
            &case_id,
            "pymupdf",
            "1.24.14",
            "text/plain|ocr=false|lang=eng",
        )
        .expect("koşu");
    let stored = db
        .store_pages(
            &run.id,
            &case_id,
            &[NewPage {
                index: 0,
                text: PAGE_TEXT.to_string(),
                width: Some(595.0),
                height: Some(842.0),
                origin: "native".into(),
                warnings: Vec::new(),
                rotation: 0,
                coordinate_space: Some("pdf_points".into()),
                page_label: Some("1".into()),
                blocks: vec![SegmentBlock {
                    start: 0,
                    end: i64::try_from(PAGE_TEXT.chars().count()).expect("uzunluk"),
                    bbox: Some([72.0, 760.0, 500.0, 780.0]),
                    lines: Vec::new(),
                    path: None,
                    kind: None,
                }],
            }],
        )
        .expect("sayfa");
    assert_eq!(stored, 1);
    db.finish_parse_run(&run.id, "succeeded", &[])
        .expect("bitir");

    // 4) Arama: yalnız bu dosyada bulunur, sonuç doğru sürümü taşır.
    let hits = db.search_segments(&case_id, "ödenmedi", 10).expect("arama");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].raw_text, PAGE_TEXT);
    assert_eq!(
        hits[0].document_version_id.as_deref(),
        Some(version.id.as_str())
    );
    assert_eq!(hits[0].page_index, Some(0));

    let other_case = db
        .create_case(&NewCase {
            title: "Başka dosya".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("diğer dosya");
    assert!(
        db.search_segments(&other_case.id, "ödenmedi", 10)
            .expect("arama")
            .is_empty(),
        "dosya kapsamı sızmamalı"
    );

    // 5) Kaynak anchor: alıntı segmentte doğrulanır.
    let anchor = db
        .create_anchor(&NewAnchor {
            case_id: case_id.clone(),
            document_version_id: version.id.clone(),
            parse_run_id: run.id.clone(),
            segment_id: hits[0].segment_id.clone(),
            start_offset: 0,
            end_offset: i64::try_from(QUOTE.chars().count()).expect("uzunluk"),
            exact_quote: QUOTE.to_string(),
        })
        .expect("anchor");
    let location = db
        .locate_anchor(&anchor.id)
        .expect("konum")
        .expect("anchor var");
    assert!(location.quote_intact);
    assert_eq!(location.page_index, Some(0));

    // 6) İddia + delil bağı.
    let claim = db
        .create_claim(
            &case_id,
            &NewClaim {
                claimant_actor_id: None,
                claim_text: "Kira bedeli ödenmedi.".into(),
                claim_kind: "allegation".into(),
                source_anchor_id: Some(anchor.id.clone()),
            },
        )
        .expect("iddia");
    assert_eq!(claim.status, "draft");
    let link = db
        .link_evidence(
            &case_id,
            &NewEvidenceLink {
                claim_id: claim.id.clone(),
                source_anchor_id: anchor.id.clone(),
                relation: "supports_candidate".into(),
                rationale: "Alıntı ödeme yapılmadığını gösteriyor".into(),
            },
        )
        .expect("delil bağı");
    assert_eq!(link.review_state, "accepted");

    // 7) İnceleme kuyruğu → karar → hedef kayıt + outbox atomik.
    let item = db
        .create_review_item(
            &case_id,
            &NewReviewItem {
                item_kind: "claim_candidate".into(),
                payload: serde_json::json!({
                    "claimText": "Kira bedeli ödenmedi.",
                    "claimKind": "allegation",
                    "anchorIds": [anchor.id],
                }),
                source_anchor_ids: vec![anchor.id.clone()],
                ai_run_id: None,
            },
        )
        .expect("inceleme öğesi");
    assert_eq!(item.ai_run_id, None, "öğe AI çalışmasına bağlı değil");

    let committed = db
        .commit_review_decision(
            &item.id,
            "accept",
            &ReviewTarget::Claim {
                claim_text: "Kira bedeli ödenmedi.".into(),
                claim_kind: "allegation".into(),
                claimant_actor_id: None,
                source_anchor_id: Some(anchor.id.clone()),
            },
            "avukat",
            "Kaynak alıntı doğrulandı",
        )
        .expect("karar");
    assert_eq!(committed.target_kind, "claim");
    assert_ne!(committed.target_id, claim.id);
    assert_eq!(count(&db, "SELECT count(*) FROM claims"), 2);

    // Aynı öğe ikinci kez karara bağlanamaz: çift kayıt üretilmez.
    let again = db
        .commit_review_decision(
            &item.id,
            "accept",
            &ReviewTarget::Claim {
                claim_text: "Kira bedeli ödenmedi.".into(),
                claim_kind: "allegation".into(),
                claimant_actor_id: None,
                source_anchor_id: Some(anchor.id.clone()),
            },
            "avukat",
            "tekrar",
        )
        .expect_err("ikinci karar reddedilmeli");
    assert_eq!(again.code(), "E_DB_SETTING");
    assert_eq!(count(&db, "SELECT count(*) FROM claims"), 2);

    let outbox = db.list_outbox_events(Some(&case_id), true).expect("outbox");
    let decision_event = outbox
        .iter()
        .find(|event| event.event_type == EVENT_DECISION_COMMITTED)
        .expect("karar olayı");
    let payload = decision_event.payload.to_string();
    assert!(payload.contains(&committed.target_id));
    assert!(
        !payload.contains("Kira bedeli"),
        "outbox iddia metnini taşımamalı"
    );

    // 8) Görev + kullanıcı onaylı tarih + yerel hatırlatıcı.
    let task = db
        .create_task(
            &case_id,
            &NewTask {
                title: "Tebliğ tarihini kontrol et".into(),
                user_due_date: Some("2026-10-23".into()),
                date_timezone: Some("Europe/Istanbul".into()),
                assignee_label: None,
                reminder_policy: "on_day".into(),
            },
        )
        .expect("görev");
    db.confirm_task_date(&task.id).expect("tarih onayı");
    let queued = db
        .enqueue_task_reminders(&case_id, "2026-10-23")
        .expect("hatırlatıcı");
    assert_eq!(queued.queued, 1);
    let reminder_event = db
        .list_outbox_events(Some(&case_id), true)
        .expect("outbox")
        .into_iter()
        .find(|event| event.event_type == EVENT_TASK_REMINDER)
        .expect("hatırlatıcı olayı");
    assert!(
        !reminder_event.payload.to_string().contains("Tebliğ"),
        "hatırlatıcı görev başlığını taşımamalı"
    );

    // 9) AI kapalı: politika `off`, hiçbir sağlayıcı/çalışma/aktarım kaydı yok.
    let case = db.get_case(&case_id).expect("dosya").expect("var");
    assert_eq!(case.default_ai_policy, "off");
    assert_eq!(count(&db, "SELECT count(*) FROM ai_runs"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM ai_candidates"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM transfer_consents"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM provider_profiles"), 0);

    // 10) Kapsam raporu ve denetim zinciri tutarlı.
    let coverage = db.coverage_report(&case_id).expect("kapsam");
    assert_eq!(coverage.documents, 1);
    assert_eq!(coverage.versions, 1);
    assert_eq!(coverage.parse_runs, 1);
    assert_eq!(coverage.pages, 1);
    assert_eq!(coverage.segments, 1);
    assert_eq!(coverage.unparsed_documents, 0);
    assert_eq!(coverage.failed_parse_runs, 0);
    assert!(db.verify_audit_chain().expect("denetim") >= 8);
}
