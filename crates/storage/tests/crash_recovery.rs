//! **F15-T06 — gerçek süreç öldürme tatbikatı (iş kuyruğu).**
//!
//! Uygulama ayrıştırma sırasında SIGKILL ile ölürse kasada ne kalır? Tatbikat
//! gerçek bir çocuk süreç başlatır: çocuk kasayı açar, işi sahiplenir (kira
//! **gelecekte**), hazır olduğunu bildirir ve öldürülmeyi bekler. Ebeveyn
//! `SIGKILL` gönderir, sonra kasayı yeniden açar:
//!
//! 1. satır gerçekten `running` ve kira gelecekte (çökme izi),
//! 2. orta oturum kurtarması (`recover_stale_jobs`) bu işe **dokunmaz**,
//! 3. açılış kurtarması (`recover_orphaned_jobs`) işi keser ve yeniden sıraya
//!    alır — kullanıcı uygulamayı hemen yeniden açtığında iş "Yürüyor"da asılı
//!    kalmaz.
//!
//! Çocuk kipi yalnız `DIM_CRASH_CHILD=1` iken koşar; normal test turunda
//! gövde boş döner (yan etkisiz).

#![allow(clippy::disallowed_methods)] // gerekçe: tatbikat kendi test ikilisini SIGKILL için başlatır; ürün yolunda süreç başlatma yok (ADR-05).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, NewCase, Sensitivity, DEFAULT_LEASE_SECONDS,
};
use time::format_description::well_known::Rfc3339;

const CHILD_TEST: &str = "drill_child_claims_a_job_and_dies";
const READY: &str = "DIM-CRASH-READY";

fn key() -> DatabaseKey {
    DatabaseKey::new([41u8; 32])
}

fn at(seconds: i64) -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc() + time::Duration::seconds(seconds)
}

/// Çocuk süreç gövdesi: işi sahiplenir, `READY` yazar ve öldürülmeyi bekler.
#[test]
fn drill_child_claims_a_job_and_dies() {
    if std::env::var("DIM_CRASH_CHILD").is_err() {
        return;
    }
    let path = std::env::var("DIM_CRASH_VAULT").expect("kasa yolu");
    let db = Database::open(Path::new(&path), &key()).expect("çocuk kasayı açar");
    let case = db
        .create_case(&NewCase {
            title: "Çökme tatbikatı".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");
    db.enqueue_job_with_priority(
        "parse_document",
        Some(&case.id),
        "fp-crash",
        "parse:sürüm-1",
        0,
    )
    .expect("iş");
    let claimed = db
        .claim_job_by_key("parse:sürüm-1", "parse-999-1", DEFAULT_LEASE_SECONDS, at(1))
        .expect("sahiplen")
        .expect("iş var");
    assert_eq!(claimed.state, "running");
    // Ebeveyn bu satırı görünce süreci öldürür: sahiplenme tamam, temizleme yok.
    println!("{READY} {}", claimed.id);
    std::thread::sleep(Duration::from_secs(120));
}

#[test]
fn killed_process_leaves_the_job_running_and_the_next_open_requeues_it() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("crash.db");
    let exe = std::env::current_exe().expect("test ikilisi");
    let mut child = Command::new(exe)
        .args(["--exact", CHILD_TEST, "--nocapture"])
        .env("DIM_CRASH_CHILD", "1")
        .env("DIM_CRASH_VAULT", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("çocuk süreç başlar");

    let stdout = child.stdout.take().expect("boru");
    let mut reader = BufReader::new(stdout);
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut job_id = None;
    while Instant::now() < deadline {
        let mut line = String::new();
        if reader.read_line(&mut line).expect("oku") == 0 {
            break;
        }
        if let Some(rest) = line.trim().strip_prefix(READY) {
            job_id = Some(rest.trim().to_string());
            break;
        }
    }
    let job_id = job_id.unwrap_or_else(|| {
        let _ = child.kill();
        panic!("çocuk sahiplenmeyi bildirmedi");
    });

    child.kill().expect("SIGKILL");
    let status = child.wait().expect("çocuk toplanır");
    assert!(
        !status.success(),
        "süreç gerçekten öldürülmüş olmalı: {status:?}"
    );

    // --- 1) Çökme izi: satır `running`, kira **gelecekte**.
    let db = Database::open(&path, &key()).expect("öldürmeden sonra kasa açılır");
    let job = db
        .get_job(&job_id)
        .expect("oku")
        .expect("iş satırı duruyor");
    assert_eq!(
        job.state, "running",
        "SIGKILL sonrası satır çalışıyor görünür"
    );
    let lease = job.lease_until.as_deref().expect("kira gelecekte kalır");
    let lease_at = time::OffsetDateTime::parse(lease, &Rfc3339).expect("kira zamanı");
    assert!(
        lease_at > time::OffsetDateTime::now_utc(),
        "kira süresi dolmamış olmalı (çökme anında 300 sn ileri)"
    );

    // --- 2) Orta oturum kurtarması kiraya saygı duyar: canlı işçi olabilir.
    let mid = db
        .recover_stale_jobs(time::OffsetDateTime::now_utc())
        .expect("orta oturum");
    assert_eq!(mid.interrupted, 0, "süresi dolmamış kira kesilmemeli");
    assert_eq!(
        db.get_job(&job_id).expect("oku").expect("var").state,
        "running"
    );

    // --- 3) Açılış kurtarması: bu süreçte işçi yok → iş sahipsiz.
    let report = db
        .recover_orphaned_jobs(time::OffsetDateTime::now_utc())
        .expect("açılış");
    assert_eq!(report.interrupted, 1);
    assert_eq!(report.requeued, 1);
    let row = db.get_job(&job_id).expect("oku").expect("var");
    assert_eq!(row.state, "queued", "iş yeniden sıraya alınmalı");
    assert!(row.lease_owner.is_none(), "sahipsiz iş kira taşımaz");
    assert_eq!(row.attempt_count, 1, "kesinti başarısız deneme değildir");

    // İş gerçekten yeniden sahiplenilebilir (numara sürer).
    let again = db
        .claim_job_by_key(
            "parse:sürüm-1",
            "parse-1000-1",
            DEFAULT_LEASE_SECONDS,
            at(2),
        )
        .expect("yeniden sahiplen")
        .expect("iş var");
    assert_eq!(again.id, job_id);
    assert_eq!(again.attempt_count, 2);
}
