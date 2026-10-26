//! İçe aktarma → kuyruk → ayrıştırma el değiştirmesi (F03-T07).
//!
//! Bu testin kanıtladığı şey tek bir cümlede: **içe aktarma gerçek bir iş
//! kaydı açıyor ve ayrıştırma yolu o işi gerçekten sahipleniyor.** Önceden
//! `parse_document` işi sonsuza kadar `queued` kalıyordu; hiçbir katman
//! sahiplenmiyordu. Burada Python worker çalıştırılmaz (worker ortamı bu
//! makinede yok); sahiplenme/bitirme çekirdek API'siyle yapılır.

use std::path::Path;

use dim_ingest::{import_path, ImportResult};
use dim_storage::{CaseKind, Database, DatabaseKey, NewCase, Sensitivity};
use dim_vault::VaultFile;

fn setup(dir: &Path) -> (Database, dim_vault::UnlockedVault, String) {
    let db =
        Database::open(&dir.join("handoff.dimvault"), &DatabaseKey::new([23u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "El değiştirme".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: dim_storage::AiPolicy::Off,
        })
        .expect("dosya");
    let (_file, _recovery, vault) =
        VaultFile::create(&dir.join("v.dimvault"), "parola-uzun-123").expect("kasa dosyası");
    (db, vault, case.id)
}

#[test]
fn import_opens_a_job_and_the_parse_path_can_claim_it() {
    let dir = tempfile::tempdir().expect("dizin");
    let (db, vault, case_id) = setup(dir.path());
    let blob_dir = dir.path().join("blobs");
    std::fs::create_dir_all(&blob_dir).expect("blob dizini");
    let source = dir.path().join("dilekce.txt");
    std::fs::write(&source, "Sayın Mahkeme, dava dilekçemiz ektedir.\n").expect("yaz");

    let imported = import_path(&db, &vault, &blob_dir, &case_id, &source, false).expect("içe al");
    let version = match imported {
        ImportResult::Imported { version, .. } => version,
        other => panic!("ilk içe aktarma yeni sürüm olmalı: {other:?}"),
    };

    // 1) İçe aktarma **gerçekten** bir iş kaydı açtı.
    let key = format!("parse:{}", version.id);
    let job = db.job_by_key(&key).expect("oku").expect("iş kaydı yok");
    assert_eq!(job.job_type, "parse_document");
    assert_eq!(job.state, "queued");
    assert_eq!(job.attempt_count, 0);
    assert!(job.next_run_at.is_some(), "iş hemen hazır olmalı");
    assert_eq!(job.case_id.as_deref(), Some(case_id.as_str()));

    // 2) Aynı dosya ikinci kez içe alınınca **yeni iş** açılmaz (idempotency).
    let again = import_path(&db, &vault, &blob_dir, &case_id, &source, false).expect("tekrar");
    assert!(matches!(again, ImportResult::Duplicate { .. }));
    assert_eq!(db.job_queue_summary().expect("özet").total, 1);

    // 3) Ayrıştırma yolu işi sahiplenir; iş "yürüyor" olur ve ikinci kez
    //    sahiplenilemez.
    let owner = "parse-test";
    let claimed = db
        .claim_job_by_key(&key, owner, 300, dim_storage::queue::now_utc())
        .expect("sahiplen")
        .expect("sahiplenilebilmeli");
    assert_eq!(claimed.state, "running");
    assert_eq!(claimed.attempt_count, 1);
    assert!(db
        .claim_job_by_key(&key, owner, 300, dim_storage::queue::now_utc())
        .expect("tekrar")
        .is_none());

    // 4) Bitince kuyruk boşalır; iş `succeeded`.
    let done = db
        .complete_job(&claimed.id, owner, dim_storage::queue::now_utc())
        .expect("bitir");
    assert_eq!(done.state, "succeeded");
    let summary = db.job_queue_summary().expect("özet");
    assert_eq!(summary.total, 1);
    assert_eq!(summary.due_now, 0);
}
