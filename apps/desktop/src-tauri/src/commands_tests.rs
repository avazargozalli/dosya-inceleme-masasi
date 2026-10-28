
use super::*;

/// Makbuz hedefi **yalnız** ana makine + model taşır: yol, sorgu ve anahtar
/// makbuza sızmaz.
#[test]
fn transfer_receipt_destination_hides_path_and_secrets() {
    assert_eq!(
        destination_label("https://api.ornek.com/v1?key=GIZLI", "model-1"),
        "api.ornek.com · model-1"
    );
    assert_eq!(
        destination_label("  https://ornek.com  ", "m"),
        "ornek.com · m"
    );
    assert!(!destination_label("https://a.com/x?api_key=SIR", "m").contains("SIR"));
}

/// Kapsam dışı belgeler sessizce yutulmaz; nedeni tek cümlede yazılır.
#[test]
fn skipped_scope_is_reported_with_reasons() {
    let skipped = vec![
        dim_storage::ScopeSkip {
            document_id: "d1".into(),
            reason: "not_parsed".into(),
        },
        dim_storage::ScopeSkip {
            document_id: "d2".into(),
            reason: "not_parsed".into(),
        },
        dim_storage::ScopeSkip {
            document_id: "d3".into(),
            reason: "empty_text".into(),
        },
    ];
    let note = scope_skip_note(&skipped);
    assert!(note.contains("2 belge ayrıştırılmamış"), "{note}");
    assert!(note.contains("1 belgede metin yok"), "{note}");
    assert_eq!(scope_skip_note(&[]), "");
}

#[test]
fn empty_path_rejected() {
    assert_eq!(resolve_vault_path("   ").unwrap_err().code, "E_PATH");
}

/// **F04-T07 ilerleme olayı:** yalnız sürüm kimliği, aşama ve sayaç taşır;
/// belge metni, dosya adı veya yol yayımlanmaz.
#[test]
fn parse_progress_event_carries_only_stage_and_counts() {
    let event = ParseProgressEvent::new(
        "version-1",
        &ParseProgress {
            stage: "pages".to_string(),
            done: 7,
            total: Some(40),
        },
    );
    let value = serde_json::to_value(&event).expect("olay JSON");
    assert_eq!(value["versionId"], "version-1");
    assert_eq!(value["stage"], "pages");
    assert_eq!(value["done"], 7);
    assert_eq!(value["total"], 40);
    // Sözleşme: dört alan; metin/yol/ad alanı yok.
    assert_eq!(value.as_object().expect("nesne").len(), 4);
    for forbidden in ["text", "fileName", "path", "quote"] {
        assert!(value.get(forbidden).is_none(), "{forbidden} yayımlanmamalı");
    }
    // Toplam bilinmiyorsa `null` (uydurulmuş sayı yok).
    let open = ParseProgressEvent::new(
        "version-1",
        &ParseProgress {
            stage: "start".to_string(),
            done: 0,
            total: None,
        },
    );
    let value = serde_json::to_value(&open).expect("olay JSON");
    assert!(value["total"].is_null());
}

/// **§17.3 bütçe görünümü:** çekirdekten gelen rapor arayüz tipine **birebir**
/// çevrilir; sınır uydurulmaz, ihlal kodu çekirdeğin kodudur.
#[test]
fn worker_budget_view_mirrors_the_core_report() {
    let budget = WorkerBudget::for_kind(WorkerKind::Ocr);
    let report = BudgetReport {
        budget,
        elapsed: std::time::Duration::from_millis(1234),
        output_bytes: 2048,
        peak_memory_bytes: 96 * 1024 * 1024,
        violated: Some(dim_ingest::budget::BudgetDimension::Memory),
    };
    let view = WorkerBudgetView::from_report(&report);
    assert_eq!(view.kind, "ocr");
    assert_eq!(view.label, "OCR");
    assert_eq!(view.cpu_seconds, budget.cpu_seconds);
    assert_eq!(view.memory_bytes, budget.memory_bytes);
    assert_eq!(view.output_bytes, budget.output_bytes);
    assert_eq!(view.open_files, budget.open_files);
    assert_eq!(view.wall_clock_seconds, budget.wall_clock.as_secs());
    assert_eq!(view.elapsed_ms, 1234);
    assert_eq!(view.observed_output_bytes, 2048);
    assert_eq!(view.peak_memory_bytes, 96 * 1024 * 1024);
    assert_eq!(view.violated.as_deref(), Some("memory"));
    // İhlal yoksa `null` — "0" ya da boş metin değil.
    let clean = WorkerBudgetView::from_report(&BudgetReport {
        violated: None,
        ..report
    });
    assert!(clean.violated.is_none());
}

/// Profiller arayüze **çekirdekten** gelir (`WorkerKind::ALL` sırası korunur);
/// AI profilinde CPU sınırı yoktur (çağrı yerel süreç değil).
#[test]
fn worker_budgets_lists_all_profiles_from_the_core() {
    let profiles = worker_budgets();
    let codes: Vec<&str> = profiles
        .iter()
        .map(|profile| profile.kind.as_str())
        .collect();
    assert_eq!(codes, vec!["render", "extract", "ocr", "redaction", "ai"]);
    for profile in &profiles {
        assert!(!profile.label.is_empty(), "{}: etiket", profile.kind);
        assert!(profile.memory_bytes > 0, "{}: bellek", profile.kind);
        assert!(profile.output_bytes > 0, "{}: çıktı", profile.kind);
        assert!(profile.wall_clock_seconds > 0, "{}: süre", profile.kind);
    }
    let ai = profiles
        .iter()
        .find(|p| p.kind == "ai")
        .expect("ai profili");
    assert_eq!(ai.cpu_seconds, 0, "AI çağrısı yerel süreç değil");
}

/// **Kuyruk sayacı (F04-T07 → kart):** aynı kareler iş satırına yazılır;
/// olay her karede gider ama yazım kapıya uyar (aşama kenarları + aralık).
/// Gerçek kasa (bellek içi) + gerçek kuyruk satırı; worker yok.
#[test]
fn progress_sink_writes_the_job_row_and_respects_the_gate() {
    let db = Database::open_in_memory(&DatabaseKey::new([7u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Sayaç".into(),
            case_kind: dim_storage::CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: dim_storage::Sensitivity::Normal,
            default_ai_policy: dim_storage::AiPolicy::Off,
        })
        .expect("dosya");
    db.enqueue_job_with_priority("parse_document", Some(&case.id), "fp-1", "parse:v1", 0)
        .expect("iş");
    let job = db
        .claim_job_by_key(
            "parse:v1",
            "surec-1",
            dim_storage::DEFAULT_LEASE_SECONDS,
            time::OffsetDateTime::now_utc(),
        )
        .expect("sahiplen")
        .expect("iş var");

    let mut frames: Vec<ParseProgress> = Vec::new();
    let mut emit = |update: &ParseProgress| frames.push(update.clone());
    let mut store = ProgressStore::Direct(&db);
    let mut sink = JobProgressSink::new(Some(job.id.clone()), "surec-1".into(), &mut emit);
    for page in 1..=40u32 {
        sink.record(
            &ParseProgress {
                stage: "pages".into(),
                done: page,
                total: Some(40),
            },
            &mut store,
        );
    }
    sink.record(
        &ParseProgress {
            stage: "store".into(),
            done: 0,
            total: Some(40),
        },
        &mut store,
    );
    sink.record(
        &ParseProgress {
            stage: "store".into(),
            done: 40,
            total: Some(40),
        },
        &mut store,
    );

    let written = sink.written;
    let skipped = sink.skipped;
    let failed = sink.failed;
    let metadata = sink.audit_metadata();
    drop(sink);

    assert_eq!(frames.len(), 42, "olay her karede arayüze gitmeli");
    assert_eq!(failed, 0, "yazım düşmemeli");
    assert!(
        written >= 3,
        "aşama kenarları yazılmalı (ilk kare, store başı, store sonu): {written}"
    );
    assert!(
        written + skipped == 42,
        "her kare ya yazıldı ya atlandı ({written} + {skipped})"
    );
    assert!(
        written < 42,
        "sayfa başına commit yapılmamalı: {written} yazım"
    );

    let row = db.get_job(&job.id).expect("oku").expect("satır");
    assert_eq!(row.state, "running", "sayaç iş durumunu değiştirmez");
    assert_eq!(row.progress_stage.as_deref(), Some("store"));
    assert_eq!(row.progress_done, Some(40));
    assert_eq!(row.progress_total, Some(40));
    assert_eq!(row.progress, 1.0);

    // Denetim yükü **yalnız sayı** taşır; metin/ad/yol yok.
    let object = metadata.as_object().expect("nesne");
    assert_eq!(object.len(), 4);
    for forbidden in ["text", "fileName", "path"] {
        assert!(object.get(forbidden).is_none(), "{forbidden} yazılmamalı");
    }
}

/// **F04-T07 yeniden deneme denetimi — girdi çözümü.** Düğme yalnız satırı
/// `queued` yapmıyor; yürütülebilir işte sürüm çözülüp ayrıştırma
/// başlatılıyor. Maskeleme isteği (terimler/bölgeler) iş satırında
/// **saklanmadığı** için uydurulmuş bir istekle yeniden çalıştırılmaz.
#[test]
fn retry_runs_only_jobs_whose_input_is_known() {
    assert_eq!(
        runnable_parse_version("parse_document", Some("parse:sürüm-7")).as_deref(),
        Some("sürüm-7")
    );
    assert_eq!(
        runnable_parse_version("redaction", Some("redact:sürüm-7:abc")),
        None,
        "maskeleme isteği satırda yok; tahmin edilmez"
    );
    assert_eq!(
        runnable_parse_version("parse_document", Some("import:sürüm-7")),
        None,
        "başka anahtar biçimi sessizce yorumlanmaz"
    );
    assert_eq!(
        runnable_parse_version("parse_document", Some("parse:  ")),
        None
    );
    assert_eq!(runnable_parse_version("parse_document", None), None);
}

/// **Yeniden deneme planı (gerçek kasa, bellek içi, worker yok).**
/// `failed → queued`, deneme bütçesi sıfırlanır, girdi anahtarından sürüm
/// çözülür; yürütücüsü olmayan iş türünde sürüm **uydurulmaz**.
#[test]
fn retry_plan_reopens_failed_jobs_and_resolves_the_parse_version() {
    let db = Database::open_in_memory(&DatabaseKey::new([3u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Yeniden deneme".into(),
            case_kind: dim_storage::CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: dim_storage::Sensitivity::Normal,
            default_ai_policy: dim_storage::AiPolicy::Off,
        })
        .expect("dosya");
    let now = time::OffsetDateTime::now_utc();
    // Enqueue `next_run_at`ı kendi saatinden yazar; sahiplenme ondan **sonra**
    // olmalı (eşitlik yeterli değil).
    let claim_at = now + time::Duration::seconds(1);
    let parse_job = db
        .enqueue_job_with_priority(
            "parse_document",
            Some(&case.id),
            "fp-parse",
            "parse:sürüm-7",
            0,
        )
        .expect("iş");
    let claimed = db
        .claim_job_by_key(
            "parse:sürüm-7",
            "w1",
            dim_storage::DEFAULT_LEASE_SECONDS,
            claim_at,
        )
        .expect("sahiplen")
        .expect("iş var");
    db.fail_job(&claimed.id, "w1", "E_WORKER_TIMEOUT", false, claim_at)
        .expect("başarısız");

    let redact_job = db
        .enqueue_job_with_priority(
            "redaction",
            Some(&case.id),
            "fp-redact",
            "redact:sürüm-7:abc",
            0,
        )
        .expect("iş");
    let claimed_redact = db
        .claim_job_by_key(
            "redact:sürüm-7:abc",
            "w2",
            dim_storage::DEFAULT_LEASE_SECONDS,
            claim_at,
        )
        .expect("sahiplen")
        .expect("iş var");
    db.fail_job(
        &claimed_redact.id,
        "w2",
        "E_WORKER_DISCONNECTED",
        false,
        claim_at,
    )
    .expect("başarısız");

    // Anahtar okuması yürütücünün tek dayanağıdır: arayüze verilmez.
    assert_eq!(
        db.job_idempotency_key(&parse_job)
            .expect("anahtar")
            .as_deref(),
        Some("parse:sürüm-7")
    );

    let state = AppState::default();
    {
        let mut guard = state.lock();
        guard.db = Some(db);
    }
    let hub = state.vault_handle();

    let plan = plan_retry(&hub, &parse_job).expect("yeniden deneme");
    assert_eq!(plan.job.state, "queued", "iş yeniden sıraya alınmalı");
    assert_eq!(plan.job.attempt_count, 0, "deneme bütçesi sıfırlanır");
    assert_eq!(
        plan.version_id.as_deref(),
        Some("sürüm-7"),
        "sürüm anahtardan çözülür"
    );
    assert_eq!(plan.skipped, None);
    assert!(
        plan.options.is_none(),
        "ayar kaydı olmayan işte uydurulmuş ayar dönmez"
    );

    let redact_plan = plan_retry(&hub, &redact_job).expect("yeniden deneme");
    assert_eq!(redact_plan.job.state, "queued");
    assert_eq!(redact_plan.version_id, None, "maskeleme isteği uydurulmaz");
}

/// **F04-T07 ÖLÇÜMÜ — yeniden deneme gerçekten ayrıştırıyor.**
///
/// Gerçek kasa ve gerçek worker: iş `failed` iken yeniden deneme planı
/// sürümü çözer, `run_parse` sayfaları yazar, iş `completed` olur ve
/// hiçbir adım "Sırada"da asılı kalmaz.
///
/// Elle: `DIM_WORKER_PYTHON=workers/document/.venv/bin/python cargo test
/// --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
/// retry_parses -- --ignored --nocapture`
#[test]
#[ignore = "gerçek worker + gerçek kasa; ölçüm elle koşulur"]
fn retrying_a_failed_parse_job_really_parses_the_document() {
    if redaction_dev_python().is_none() {
        eprintln!("python+pymupdf yok; test atlandı");
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/synthetic/progress-pages.pdf");
    let data = std::fs::read(&fixture).expect("fikstür");
    let (state, _dir, version) = redaction_fixture_with(&data);

    // İş: içe aktarma gibi `parse:<sürüm>` anahtarıyla açılır, bir kez
    // sahiplenilip **başarısız** kapatılır (kullanıcının gördüğü durum).
    let job_id = {
        let guard = state.lock();
        let db = guard.db.as_ref().expect("db");
        let key = format!("parse:{}", version.id);
        db.enqueue_job_with_priority("parse_document", None, "fp", &key, 0)
            .expect("iş");
        let claimed = db
            .claim_job_by_key(
                &key,
                "w-once",
                dim_storage::DEFAULT_LEASE_SECONDS,
                time::OffsetDateTime::now_utc(),
            )
            .expect("sahiplen")
            .expect("iş var");
        db.fail_job(
            &claimed.id,
            "w-once",
            "E_WORKER_TIMEOUT",
            false,
            time::OffsetDateTime::now_utc(),
        )
        .expect("başarısız");
        claimed.id
    };

    let hub = state.vault_handle();
    let slot = state.workers_handle();
    let pool = state.parse_pool_handle();
    let plan = plan_retry(&hub, &job_id).expect("plan");
    assert_eq!(plan.job.state, "queued");
    let version_id = plan.version_id.expect("sürüm çözülmeli");
    assert_eq!(version_id, version.id);

    let frames = Arc::new(Mutex::new(Vec::<ParseProgress>::new()));
    let collected = Arc::clone(&frames);
    let mut emit = |update: &ParseProgress| {
        collected.lock().expect("kare").push(update.clone());
    };
    let started = Instant::now();
    let view = run_parse(
        &hub,
        &slot,
        &pool,
        &version_id,
        ParseOptions {
            ocr: None,
            ocr_lang: None,
            text_encoding: None,
            password: None,
        },
        &mut emit,
    )
    .expect("yeniden deneme ayrıştırması");
    let elapsed = started.elapsed();
    let seen = frames.lock().expect("kare").clone();
    println!(
        "yeniden deneme: {} sayfa, {} segment, {:?}, kareler {:?}",
        view.pages,
        view.stored_segments,
        elapsed,
        seen.iter().map(|f| f.stage.clone()).collect::<Vec<_>>()
    );
    assert_eq!(view.pages, 4, "fikstür 4 sayfa");
    assert!(view.stored_segments > 0, "segment yazılmalı");
    assert!(
        seen.iter().any(|f| f.stage == "pages" && f.done > 0),
        "canlı sayaç gelmeli: {seen:?}"
    );
    let guard = state.lock();
    let job = guard
        .db
        .as_ref()
        .expect("db")
        .get_job(&job_id)
        .expect("oku")
        .expect("satır");
    assert_eq!(
        job.state, "completed",
        "iş terminal olmalı, asılı kalmamalı"
    );
    assert_eq!(
        job.progress_stage.as_deref(),
        Some("store"),
        "son sayaç aşaması kasaya yazma olmalı (ayrıştırma yolunda `scan` yok)"
    );
    assert_eq!(job.attempt_count, 1, "yeniden deneme yeni bir denemedir");
}

/// **Ayarların geri yüklenmesi (saf).** Kayıtlı ayar isteğe birebir geçer;
/// kayıt yoksa varsayılan + `restored=false`. Parola hiçbir yolda taşınmaz.
#[test]
fn retry_options_come_from_the_recorded_job_row() {
    let stored = dim_storage::queue::JobParseOptions {
        ocr: true,
        ocr_lang: "tur+eng".into(),
        text_encoding: "windows-1254".into(),
        requires_password: true,
    };
    let (options, applied) = retry_parse_options(Some(&stored));
    assert_eq!(options.ocr, Some(true));
    assert_eq!(options.ocr_lang.as_deref(), Some("tur+eng"));
    assert_eq!(options.text_encoding.as_deref(), Some("windows-1254"));
    assert!(options.password.is_none(), "parola isteğe taşınmaz");
    assert!(applied.restored, "kayıt geri yüklendi");
    assert!(applied.ocr);
    assert_eq!(applied.ocr_lang, "tur+eng");
    assert_eq!(applied.text_encoding, "windows-1254");

    let (defaults, applied) = retry_parse_options(None);
    assert_eq!(defaults.ocr, None, "kayıt yok → OCR kapalı varsayılanı");
    assert!(defaults.password.is_none());
    assert!(!applied.restored, "kayıt yokluğu dürüstçe bildirilir");
    assert!(!applied.ocr);
    assert_eq!(applied.ocr_lang, "eng");
    assert_eq!(applied.text_encoding, "auto");
}

/// Kaydedilmiş ayar planla birlikte gelir; **parola ile açılmış** iş
/// kendiliğinden yeniden başlatılmaz (satır `failed` kalır, sahte "başladı"
/// denmez).
#[test]
fn retry_plan_restores_recorded_options_and_holds_password_jobs() {
    use dim_storage::queue::JobParseOptions;

    let db = Database::open_in_memory(&DatabaseKey::new([5u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Ayarlar".into(),
            case_kind: dim_storage::CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: dim_storage::Sensitivity::Normal,
            default_ai_policy: dim_storage::AiPolicy::Off,
        })
        .expect("dosya");
    let now = time::OffsetDateTime::now_utc();
    let claim_at = now + time::Duration::seconds(1);

    let failed_parse = |key: &str| -> (String, String) {
        let id = db
            .enqueue_job_with_priority("parse_document", Some(&case.id), "fp", key, 0)
            .expect("iş");
        let claimed = db
            .claim_job_by_key(key, "w1", dim_storage::DEFAULT_LEASE_SECONDS, claim_at)
            .expect("sahiplen")
            .expect("iş var");
        db.fail_job(&claimed.id, "w1", "E_WORKER_TIMEOUT", false, claim_at)
            .expect("başarısız");
        (id, claimed.id)
    };
    // Anahtar biçimi `parse:<sürüm>`: sürüm kimliği anahtardan gelir.
    let (with_settings, with_settings_id) = failed_parse("parse:v-tur");
    db.set_job_parse_options(
        &with_settings,
        &JobParseOptions {
            ocr: true,
            ocr_lang: "tur".into(),
            text_encoding: "auto".into(),
            requires_password: false,
        },
        claim_at,
    )
    .expect("ayar");
    let (with_password, with_password_id) = failed_parse("parse:v-parola");
    db.set_job_parse_options(
        &with_password,
        &JobParseOptions {
            ocr: false,
            ocr_lang: "eng".into(),
            text_encoding: "auto".into(),
            requires_password: true,
        },
        claim_at,
    )
    .expect("ayar");

    let state = AppState::default();
    {
        let mut guard = state.lock();
        guard.db = Some(db);
    }
    let hub = state.vault_handle();

    let plan = plan_retry(&hub, &with_settings).expect("plan");
    assert_eq!(plan.job.state, "queued");
    assert_eq!(plan.job.id, with_settings_id);
    assert_eq!(plan.skipped, None);
    let stored = plan.options.as_ref().expect("ayar kaydı");
    assert!(stored.ocr);
    assert_eq!(stored.ocr_lang, "tur");
    let (options, applied) = retry_parse_options(plan.options.as_ref());
    assert_eq!(options.ocr, Some(true), "OCR ayarı isteğe geçer");
    assert_eq!(options.ocr_lang.as_deref(), Some("tur"));
    assert!(applied.restored);

    let plan = plan_retry(&hub, &with_password).expect("plan");
    assert_eq!(plan.job.id, with_password_id);
    assert_eq!(
        plan.skipped,
        Some("password_required"),
        "parola saklanmaz; parolasız deneme başlatılmaz"
    );
    assert_eq!(
        plan.job.state, "failed",
        "satır yeniden sıraya alınmaz (asılı `queued` yok)"
    );
    assert_eq!(plan.version_id, None);
}

/// Kuyruk satırı açılır, **sahiplenilmez**: `run_parse` kendisi sahiplenir.
fn enqueue_parse_job(state: &AppState, version_id: &str) -> String {
    let guard = state.lock();
    let db = guard.db.as_ref().expect("db");
    db.enqueue_job_with_priority(
        "parse_document",
        None,
        "fp",
        &format!("parse:{version_id}"),
        0,
    )
    .expect("iş")
}

/// İş sahiplenilip **başarısız** kapatılır (kullanıcının gördüğü durum).
fn failed_parse_job(state: &AppState, version_id: &str) -> String {
    let key = format!("parse:{version_id}");
    let guard = state.lock();
    let db = guard.db.as_ref().expect("db");
    db.enqueue_job_with_priority("parse_document", None, "fp", &key, 0)
        .expect("iş");
    // `enqueue` `next_run_at`ı kendi saatinden yazar; sahiplenme **sonra**
    // olmalı (eşitlik yeterli değil).
    let claim_at = time::OffsetDateTime::now_utc() + time::Duration::seconds(1);
    let claimed = db
        .claim_job_by_key(&key, "w-once", dim_storage::DEFAULT_LEASE_SECONDS, claim_at)
        .expect("sahiplen")
        .expect("iş var");
    db.fail_job(&claimed.id, "w-once", "E_WORKER_TIMEOUT", false, claim_at)
        .expect("başarısız");
    claimed.id
}

/// **F04-T07 ÖLÇÜMÜ (kusur #58) — yeniden deneme kaydedilmiş ayarla koşar.**
///
/// Gerçek kasa + gerçek worker + **metin katmanı olmayan** fikstür
/// (`fixtures/synthetic/scanned-page.pdf`, 1 sayfa, 33.380 bayt; sayfada
/// yalnız görüntü var, `get_text()` boş).
///
/// 1. OCR **açık** koşu metni çıkarır ve ayarı iş satırına yazar.
/// 2. Ayar kaydı olmayan iş varsayılanlarla (OCR kapalı) koşarsa **metin
///    çıkmaz** — eski davranışın sessiz farkı budur.
/// 3. Kayıtlı ayarla yeniden deneme metni üretir; segment sayısı 1'e eşittir
///    (aynı belge + aynı ayar = aynı çıktı).
///
/// Elle: `DIM_WORKER_PYTHON=workers/document/.venv/bin/python cargo test
/// --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
/// retry_reuses -- --ignored --nocapture`
#[test]
#[ignore = "gerçek worker + gerçek kasa + tesseract; ölçüm elle koşulur"]
fn retry_reuses_the_recorded_ocr_setting_on_a_scanned_document() {
    if redaction_dev_python().is_none() {
        eprintln!("python+pymupdf yok; test atlandı");
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/synthetic/scanned-page.pdf");
    let data = std::fs::read(&fixture).expect("fikstür");
    assert!(data.starts_with(b"%PDF-"), "fikstür PDF olmalı");

    let ocr_on = || ParseOptions {
        ocr: Some(true),
        ocr_lang: Some("eng".to_string()),
        text_encoding: None,
        password: None,
    };
    let text_of = |state: &AppState, run_id: &str| -> String {
        let guard = state.lock();
        let db = guard.db.as_ref().expect("db");
        db.pages_for_run(run_id)
            .expect("sayfalar")
            .into_iter()
            .map(|page| page.text)
            .collect::<Vec<_>>()
            .join("\n")
    };

    // 1 — ayarın kaydı: OCR açık koşu iş satırına yazılır.
    let (state, _dir, version) = redaction_fixture_with(&data);
    let job = enqueue_parse_job(&state, &version.id);
    let mut noop = |_: &ParseProgress| {};
    let first = run_parse(
        &state.vault_handle(),
        &state.workers_handle(),
        &state.parse_pool_handle(),
        &version.id,
        ocr_on(),
        &mut noop,
    )
    .expect("OCR koşusu");
    let first_text = text_of(&state, &first.parse_run.id);
    let recorded = {
        let guard = state.lock();
        guard
            .db
            .as_ref()
            .expect("db")
            .job_parse_options(&job)
            .expect("oku")
    };
    let recorded = recorded.expect("ayar kaydı yazılmalı");
    assert!(recorded.ocr, "OCR açık ayarı iş satırına yazılır");
    assert_eq!(recorded.ocr_lang, "eng");
    assert!(!recorded.requires_password, "parola kullanılmadı");

    // 2 — kayıt yok: varsayılan (OCR kapalı) → metin çıkmaz.
    let (state_default, _dir2, version2) = redaction_fixture_with(&data);
    let job2 = failed_parse_job(&state_default, &version2.id);
    let plan = plan_retry(&state_default.vault_handle(), &job2).expect("plan");
    assert!(plan.options.is_none(), "0023 öncesi iş: ayar kaydı yok");
    let (defaults, applied) = retry_parse_options(plan.options.as_ref());
    assert!(!applied.restored, "varsayılan koşu dürüstçe bildirilir");
    let mut noop2 = |_: &ParseProgress| {};
    let without = run_parse(
        &state_default.vault_handle(),
        &state_default.workers_handle(),
        &state_default.parse_pool_handle(),
        &version2.id,
        defaults,
        &mut noop2,
    )
    .expect("varsayılan koşu");
    let without_text = text_of(&state_default, &without.parse_run.id);

    // 3 — kayıtlı ayar: aynı metin çıkar.
    let (state_recorded, _dir3, version3) = redaction_fixture_with(&data);
    let job3 = failed_parse_job(&state_recorded, &version3.id);
    {
        let guard = state_recorded.lock();
        guard
            .db
            .as_ref()
            .expect("db")
            .set_job_parse_options(
                &job3,
                &dim_storage::queue::JobParseOptions {
                    ocr: true,
                    ocr_lang: "eng".into(),
                    text_encoding: "auto".into(),
                    requires_password: false,
                },
                time::OffsetDateTime::now_utc(),
            )
            .expect("ayar");
    }
    let plan = plan_retry(&state_recorded.vault_handle(), &job3).expect("plan");
    let (options, applied) = retry_parse_options(plan.options.as_ref());
    assert!(applied.restored, "ayar iş satırından geri yüklendi");
    let mut noop3 = |_: &ParseProgress| {};
    let with = run_parse(
        &state_recorded.vault_handle(),
        &state_recorded.workers_handle(),
        &state_recorded.parse_pool_handle(),
        &version3.id,
        options,
        &mut noop3,
    )
    .expect("kayıtlı ayarla koşu");
    let with_text = text_of(&state_recorded, &with.parse_run.id);

    println!(
        "1) OCR açık: {} sayfa / {} segment, metin {:?}\n\
             2) varsayılan (eski davranış): {} sayfa / {} segment, metin {:?}\n\
             3) kayıtlı ayar (yeni davranış): {} sayfa / {} segment, metin {:?}",
        first.pages,
        first.stored_segments,
        first_text.trim(),
        without.pages,
        without.stored_segments,
        without_text.trim(),
        with.pages,
        with.stored_segments,
        with_text.trim()
    );
    assert!(
        first_text.contains("MADDE 1"),
        "OCR metni okunmalı: {first_text:?}"
    );
    assert_eq!(
        without_text.trim(),
        "",
        "varsayılan (OCR kapalı) koşuda metin çıkmaz — ölçülen fark"
    );
    assert!(
        with_text.contains("MADDE 1"),
        "kayıtlı ayarla metin çıkar: {with_text:?}"
    );
    assert_eq!(
        with.stored_segments, first.stored_segments,
        "aynı belge + aynı ayar = aynı segment sayısı"
    );
}

/// **F04-T07 ortamı:** gerçek kasa dosyası + bir belge sürümü; worker yok.
/// Şifreli blob gerçekten yazılır (maskeleme girdisi oradan okunur).
fn redaction_fixture() -> (AppState, PathBuf, DocumentVersion) {
    redaction_fixture_with(b"%PDF-1.7 maskeleme girdisi")
}

fn redaction_fixture_with(data: &[u8]) -> (AppState, PathBuf, DocumentVersion) {
    use dim_storage::{AiPolicy, CaseKind, NewCase, Sensitivity};

    let dir = std::env::temp_dir().join(format!(
        "dim-redact-job-{}-{}",
        std::process::id(),
        PARSE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dizin");
    let vault_path = dir.join("vault.dimvault");
    let (file, _recovery, unlocked) =
        VaultFile::create(&vault_path, "maskeleme-parola").expect("kasa");
    let vault_id = vault_id_from_master(unlocked.master_key());
    let db = open_database(&file.path, &unlocked, &vault_id).expect("db");
    let case = db
        .create_case(&NewCase {
            title: "Maskeleme".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");
    let sha = "a".repeat(64);
    let aad = format!("{}:original:{sha}", case.id);
    let blobs = dir.join("blobs");
    let path = unlocked
        .write_blob(&blobs, data, aad.as_bytes())
        .expect("blob");
    let storage_id = path
        .file_stem()
        .and_then(|name| name.to_str())
        .expect("ad")
        .to_string();
    let size = i64::try_from(std::fs::metadata(&path).expect("boyut").len()).unwrap_or(i64::MAX);
    let blob_id = db
        .create_blob_record(&storage_id, size, &aad)
        .expect("blob kaydı");
    let doc = db
        .create_document(&case.id, "sozlesme.pdf", "pdf", None, "unknown")
        .expect("belge");
    let version = db
        .add_version_with_parent(
            &doc.id,
            &case.id,
            &blob_id,
            &sha,
            size,
            "application/pdf",
            None,
        )
        .expect("sürüm");
    let state = AppState::default();
    {
        let mut guard = state.lock();
        guard.unlocked = Some(unlocked);
        guard.db = Some(db);
        guard.path = Some(file.path.clone());
        guard.vault_id = Some(vault_id);
    }
    (state, dir, version)
}

/// Maskeleme artık **kendi iş türünü** açar (`redaction`) ve sahiplenir;
/// yürüyen iş varken ikinci worker başlatılmaz (`E_REDACT_BUSY`).
#[test]
fn redaction_opens_its_own_job_and_refuses_a_second_run() {
    let (state, _dir, version) = redaction_fixture();
    let request = RedactRequest::Terms(vec!["Gizli".into()]);
    let prep = {
        let guard = state.lock();
        prepare_redaction(&guard, &version.id, &request).expect("hazırlık")
    };
    let job = prep.job.as_ref().expect("iş sahiplenilmeli");
    assert_eq!(job.job_type, "redaction");
    assert_eq!(job.state, "running");
    assert_eq!(job.lease_owner.as_deref(), Some(prep.owner.as_str()));
    assert!(
        !prep.data.is_empty(),
        "şifreli blob çözülmeli (maskeleme girdisi)"
    );

    let err = {
        let guard = state.lock();
        prepare_redaction(&guard, &version.id, &request)
            .map(|_| ())
            .expect_err("ikinci koşu reddedilmeli")
    };
    assert_eq!(err.code, "E_REDACT_BUSY");

    let regions = RedactRequest::Regions(vec![(0, [10.0, 10.0, 40.0, 40.0])]);
    let other = {
        let guard = state.lock();
        prepare_redaction(&guard, &version.id, &regions).expect("bölge işi")
    };
    assert_ne!(
        other.job.as_ref().expect("bölge işi").id,
        job.id,
        "farklı maskeleme isteği ayrı iş satırı"
    );
}

/// Bitmiş maskeleme yeniden istenirse **yeni** iş satırı açılır; terminal
/// satır yeniden yazılmaz (§17.2).
#[test]
fn a_completed_redaction_opens_a_fresh_job_row() {
    let (state, _dir, version) = redaction_fixture();
    let request = RedactRequest::Terms(vec!["Gizli".into()]);
    let first = {
        let guard = state.lock();
        prepare_redaction(&guard, &version.id, &request).expect("hazırlık")
    };
    let first_job = first.job.clone().expect("iş");
    {
        let guard = state.lock();
        let db = guard.db.as_ref().expect("db");
        db.complete_job(&first_job.id, &first.owner, time::OffsetDateTime::now_utc())
            .expect("bitir");
    }
    let second = {
        let guard = state.lock();
        prepare_redaction(&guard, &version.id, &request).expect("ikinci hazırlık")
    };
    let second_job = second.job.expect("yeni iş");
    assert_ne!(second_job.id, first_job.id, "yeni niyet, yeni satır");
    assert_eq!(second_job.state, "running");
    let guard = state.lock();
    let row = guard
        .db
        .as_ref()
        .expect("db")
        .get_job(&first_job.id)
        .expect("oku")
        .expect("satır");
    assert_eq!(row.state, "completed", "eski satır terminal kalır");
}

/// Gerçek worker'lı ölçümler için python + worker klasörü.
fn redaction_dev_python() -> Option<PathBuf> {
    let dir = dim_ingest::worker::dev_worker_dir()?;
    dim_ingest::worker::dev_python_with_module(&dir, "pymupdf")
}

/// **F04-T07 ÖLÇÜMÜ — kasa kilidi worker boyunca tutulmaz.** Eski kod
/// `state.lock()` altında worker'ı çalıştırıyordu: uzun maskede bütün arayüz
/// kilitleniyor, iptal düğmesine basılamıyordu. Bu test maskeleme sürerken
/// kilidi örnekler ve **en uzun bekleme**yi yazar.
///
/// Elle: `DIM_WORKER_PYTHON=workers/document/.venv/bin/python cargo test
/// --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
/// redaction_runs_off_the_vault_lock -- --ignored --nocapture`
#[test]
#[ignore = "gerçek worker + gerçek kasa; ölçüm elle koşulur"]
fn redaction_runs_off_the_vault_lock_and_reports_pages() {
    if redaction_dev_python().is_none() {
        eprintln!("python+pymupdf yok; test atlandı");
        return;
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/synthetic/progress-pages.pdf");
    let data = std::fs::read(&fixture).expect("fikstür");
    let (state, _dir, version) = redaction_fixture_with(&data);
    let hub = state.vault_handle();
    let slot = state.workers_handle();
    let pool = state.parse_pool_handle();
    let frames = Arc::new(Mutex::new(Vec::<ParseProgress>::new()));
    let collected = Arc::clone(&frames);
    let vid = version.id.clone();
    let started = Instant::now();
    let handle = std::thread::spawn(move || {
        let mut emit = |update: &ParseProgress| {
            collected.lock().expect("kare").push(update.clone());
        };
        run_redaction(
            &hub,
            &slot,
            &pool,
            &vid,
            RedactRequest::Terms(vec!["Sayfa 2".to_string()]),
            &mut emit,
        )
    });

    // Worker gerçekten koşmaya başlasın (ilk `redact` karesi).
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if frames
            .lock()
            .expect("kare")
            .iter()
            .any(|frame| frame.stage == "redact")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Kilit beklemesini örnekle: worker sürerken başka komutlar beklemez.
    let mut worst = Duration::ZERO;
    let sampling = Instant::now();
    while !handle.is_finished() && sampling.elapsed() < Duration::from_secs(60) {
        let probe = Instant::now();
        drop(state.lock());
        worst = worst.max(probe.elapsed());
        std::thread::sleep(Duration::from_millis(2));
    }
    let view = handle.join().expect("iş parçacığı").expect("maskeleme");
    let seen = frames.lock().expect("kare").clone();
    println!(
        "maskeleme: {:?} · en uzun kilit beklemesi {:?} · kareler {:?}",
        started.elapsed(),
        worst,
        seen
    );

    assert!(
        seen.iter()
            .any(|frame| frame.stage == "redact" && frame.done > 0),
        "gerçek sayfa sayacı gelmeli: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|frame| frame.stage == "verify" && frame.done > 0),
        "doğrulama aşaması da bildirilmeli: {seen:?}"
    );
    // Aşama sırası gerçek iş sırasıdır (F04-T07): kuyruk satırına en son
    // yazılan aşama `scan` olur; `pack`/`verify` ondan önce gelmeli.
    let stages: Vec<&str> = {
        let mut out: Vec<&str> = Vec::new();
        for frame in &seen {
            if out.last().copied() != Some(frame.stage.as_str()) {
                out.push(frame.stage.as_str());
            }
        }
        out
    };
    assert_eq!(
        stages,
        vec!["start", "redact", "pack", "verify", "scan"],
        "aşama sırası: {seen:?}"
    );
    assert!(
        worst < Duration::from_millis(150),
        "kasa kilidi worker boyunca tutulmamalı; en uzun bekleme {worst:?}"
    );
    assert!(view.hits > 0, "terim fikstürde bulunmalı");
    assert!(view.remaining.is_empty());
    let guard = state.lock();
    let row = guard
        .db
        .as_ref()
        .expect("db")
        .job_by_key(&format!(
            "redact:{}:{}",
            version.id,
            RedactRequest::Terms(vec!["Sayfa 2".to_string()]).fingerprint()
        ))
        .expect("oku")
        .expect("iş satırı");
    assert_eq!(row.state, "completed", "iş kuyrukta kapanmalı");
    assert_eq!(
        row.progress_stage.as_deref(),
        Some("scan"),
        "kuyruk satırındaki son aşama gerçek iş sırasını yansıtır"
    );
}

/// **F04-T07 ÖLÇÜMÜ — gerçek iptal.** Maskeleme sürerken iptal, süreci
/// durdurur; **hiçbir çıktı** yazılmaz ve iş `canceled` kapanır (§17.2).
///
/// Elle: `DIM_WORKER_PYTHON=… cargo test --manifest-path
/// apps/desktop/src-tauri/Cargo.toml --lib redaction_cancel -- --ignored --nocapture`
#[test]
#[ignore = "gerçek worker + gerçek kasa; ölçüm elle koşulur"]
fn redaction_cancel_stops_the_worker_and_writes_nothing() {
    if redaction_dev_python().is_none() {
        eprintln!("python+pymupdf yok; test atlandı");
        return;
    }
    // 120 sayfa: iptal için gerçek zaman penceresi. Fikstür **depoda**
    // durur (`fixtures/synthetic/cancel-pages.pdf`, generate.py üretir):
    // test süreç başlatmaz (ADR-05) ve ölçüm her koşuda aynı belgeyi kullanır.
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/synthetic/cancel-pages.pdf");
    let data = std::fs::read(&fixture).expect("cancel-pages.pdf fikstürü");
    let (state, _vault_dir, version) = redaction_fixture_with(&data);
    let before = state
        .lock()
        .db
        .as_ref()
        .expect("db")
        .list_documents(&version.case_id)
        .expect("belgeler")
        .len();

    let hub = state.vault_handle();
    let slot = state.workers_handle();
    let pool = state.parse_pool_handle();
    let frames = Arc::new(Mutex::new(Vec::<ParseProgress>::new()));
    let collected = Arc::clone(&frames);
    let vid = version.id.clone();
    let started = Instant::now();
    let handle = std::thread::spawn(move || {
        let mut emit = |update: &ParseProgress| {
            collected.lock().expect("kare").push(update.clone());
        };
        run_redaction(
            &hub,
            &slot,
            &pool,
            &vid,
            RedactRequest::Terms(vec!["Gizli".to_string()]),
            &mut emit,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut stopped = 0usize;
    while Instant::now() < deadline {
        let progress_read = frames.lock().expect("kare").iter().any(|frame| {
            frame.stage == "redact" && frame.done >= 1 && frame.done < frame.total.unwrap_or(0)
        });
        if progress_read {
            stopped = state.cancel_all_workers();
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(stopped >= 1, "iptal, yürüyen worker'ı durdurmalı");
    let outcome = handle.join().expect("iş parçacığı");
    println!("iptal {:?} sonra döndü", started.elapsed());
    let err = outcome
        .err()
        .expect("iptal edilen maskeleme çıktı üretmemeli");
    assert_eq!(err.code, "E_WORKER_CANCELED");

    let guard = state.lock();
    let db = guard.db.as_ref().expect("db");
    let after = db.list_documents(&version.case_id).expect("belgeler").len();
    assert_eq!(after, before, "yarım/ maskeli belge yazılmamalı");
    let job = db
        .job_by_key(&format!(
            "redact:{}:{}",
            version.id,
            RedactRequest::Terms(vec!["Gizli".to_string()]).fingerprint()
        ))
        .expect("oku")
        .expect("iş satırı");
    assert_eq!(job.state, "canceled", "iş canceled kapanmalı");
}

/// **§17.3 ölçümü — gerçek kasa + gerçek worker.** İki belge paralel
/// ayrıştırıldığında duvar saati, seri toplamdan **kısadır** ve iki
/// ayrıştırma gerçekten **çakışır**; veritabanı yazımları kasa kilidi
/// altında seri kalır (kasa başına tek yazıcı).
///
/// Elle koşulur (gerçek Python worker gerekir):
/// `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
/// two_parses -- --ignored --nocapture`
#[test]
#[ignore = "gerçek worker + gerçek kasa; F15 ölçümü elle koşulur"]
fn two_parses_run_concurrently_on_a_real_vault() {
    use dim_storage::{AiPolicy, CaseKind, NewCase, Sensitivity};

    let dir = std::env::temp_dir().join(format!("dim-parse-parallel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dizin");
    let vault_path = dir.join("vault.dimvault");
    let (file, _recovery, unlocked) = VaultFile::create(&vault_path, "olcum-parola").expect("kasa");
    let vault_id = vault_id_from_master(unlocked.master_key());
    let db = open_database(&file.path, &unlocked, &vault_id).expect("db");
    let state = AppState::default();
    {
        let mut guard = state.lock();
        guard.unlocked = Some(unlocked);
        guard.db = Some(db);
        guard.path = Some(file.path.clone());
        guard.vault_id = Some(vault_id);
    }

    let case = {
        let guard = state.lock();
        guard
            .db
            .as_ref()
            .expect("db")
            .create_case(&NewCase {
                title: "Paralel ölçüm".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: AiPolicy::Off,
            })
            .expect("dosya")
    };
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/synthetic");
    let blob_dir = file.path.parent().expect("dizin").join("blobs");
    let mut versions = Vec::new();
    for name in ["progress-pages.pdf", "redaction-hidden.pdf"] {
        let guard = state.lock();
        let db = guard.db.as_ref().expect("db");
        let vault = guard.unlocked.as_ref().expect("kasa");
        let result = import_path(db, vault, &blob_dir, &case.id, &fixtures.join(name), false)
            .expect("içe aktarma");
        let version = match result {
            ImportResult::Imported { version, .. } => version,
            ImportResult::Duplicate { version_id, .. } => {
                db.get_version(&version_id).expect("oku").expect("sürüm")
            }
        };
        drop(guard);
        versions.push(version.id);
    }

    let hub = state.vault_handle();
    let slot = state.workers_handle();
    let pool = state.parse_pool_handle();

    // Isınma: ilk süreç başlatma disk/önbellek maliyetini ölçüme katmaz.
    for version in &versions {
        let mut emit = |_p: &ParseProgress| {};
        run_parse(
            &hub,
            &slot,
            &pool,
            version,
            ParseOptions::default(),
            &mut emit,
        )
        .expect("ısınma ayrıştırması");
    }

    // Seri taban: aynı iki belge sırayla.
    let serial_started = Instant::now();
    for version in &versions {
        let mut emit = |_p: &ParseProgress| {};
        run_parse(
            &hub,
            &slot,
            &pool,
            version,
            ParseOptions::default(),
            &mut emit,
        )
        .expect("seri ayrıştırma");
    }
    let serial_wall = serial_started.elapsed().as_secs_f64();

    // Paralel: iki belge aynı anda (havuz kapasitesi 2).
    let started = Instant::now();
    let marks = Arc::new(Mutex::new(Vec::<(usize, f64, f64)>::new()));
    let mut handles = Vec::new();
    for (index, version) in versions.iter().enumerate() {
        let hub = Arc::clone(&hub);
        let slot = Arc::clone(&slot);
        let pool = Arc::clone(&pool);
        let marks = Arc::clone(&marks);
        let version = version.clone();
        handles.push(std::thread::spawn(move || {
            let begin = started.elapsed().as_secs_f64();
            let mut emit = |_p: &ParseProgress| {};
            let outcome = run_parse(
                &hub,
                &slot,
                &pool,
                &version,
                ParseOptions::default(),
                &mut emit,
            );
            let end = started.elapsed().as_secs_f64();
            marks.lock().expect("ölçüm").push((index, begin, end));
            outcome.map(|view| view.pages)
        }));
    }
    let parallel: Vec<Result<usize, ApiError>> = handles
        .into_iter()
        .map(|handle| handle.join().expect("iş parçacığı"))
        .collect();
    let parallel_wall = started.elapsed().as_secs_f64();
    assert!(
        parallel.iter().all(Result::is_ok),
        "paralel ayrıştırma düştü: {parallel:?}"
    );

    // Çakışma kanıtı: iki aralık kesişmeli (gerçekten aynı anda koşuyorlar).
    let samples = marks.lock().expect("ölçüm").clone();
    let overlap = (samples[0].2.min(samples[1].2) - samples[0].1.max(samples[1].1)).max(0.0);
    assert!(overlap > 0.02, "ayrıştırmalar çakışmadı ({overlap:.3} sn)");

    // Veri bütünlüğü: her sürümün başarılı koşusu var, yuva boşaldı.
    {
        let guard = state.lock();
        let db = guard.db.as_ref().expect("db");
        for version in &versions {
            let runs = db.list_parse_runs(version).expect("koşular");
            assert!(
                runs.iter().any(|run| run.status == "succeeded"),
                "başarılı koşu yok"
            );
        }
    }
    assert!(state.workers_lock().is_empty(), "yuva temizlenmeli");

    println!(
            "parallel_wall={parallel_wall:.3}s serial_wall={serial_wall:.3}s overlap={overlap:.3}s marks={samples:?}"
        );
    assert!(
        parallel_wall < serial_wall * 0.9,
        "paralel koşu seri toplamdan belirgin hızlı olmalı: {parallel_wall:.3} vs {serial_wall:.3}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Arama kipi adı sunucuda çözülür: boş → `auto`, bilinmeyen reddedilir.
/// İki komut (`search_segments_page`, `search_segments_total`) aynı kuralı
/// kullanır; ayrışırsa sayfa ile sayı farklı kipe ait olurdu.
#[test]
fn search_mode_names_are_validated_and_empty_means_auto() {
    assert!(matches!(parse_search_mode(None), Ok(SearchMode::Auto)));
    assert!(matches!(
        parse_search_mode(Some("   ")),
        Ok(SearchMode::Auto)
    ));
    assert!(matches!(
        parse_search_mode(Some("phrase")),
        Ok(SearchMode::Phrase)
    ));
    assert!(matches!(
        parse_search_mode(Some("Akıllı")),
        Err(ApiError { .. })
    ));
    assert_eq!(
        parse_search_mode(Some("fuzzy")).unwrap_err().code,
        "E_SEARCH_QUERY"
    );
}

/// Ayrıştırma hatasının tekrar sınıfı: yalnız geçici altyapı hataları
/// yeniden denenir. Belgeye özgü hatalar tekrar denenirse aynı belge için
/// üç deneme harcanır ve kullanıcıya yanlış "geçici hata" izlenimi verilir.
/// **Disk dolu ve kullanıcı iptali** de tekrar denenmez (§17.4).
#[test]
fn only_transport_failures_are_retried() {
    for code in [
        "E_WORKER_DISCONNECTED",
        "E_WORKER_TIMEOUT",
        "E_WORKER_FRAME",
        "E_WORKER_RESPONSE",
        "E_IO",
    ] {
        assert!(parse_failure_is_retryable(code), "{code} tekrar denenmeli");
    }
    for code in [
        "E_WORKER_MISSING",
        "E_PARSE",
        "E_PDF_PASSWORD",
        "E_UNSUPPORTED",
        "E_NOT_FOUND",
        "E_DISK_FULL",
        "E_WORKER_CANCELED",
    ] {
        assert!(
            !parse_failure_is_retryable(code),
            "{code} tekrar denenmemeli"
        );
    }
}

#[test]
fn rollback_accepts_only_a_bare_backup_file_name() {
    assert_eq!(
        rollback_backup_name("vault.db.v12-pre-update.bak").expect("ad"),
        "vault.db.v12-pre-update.bak"
    );
    for bad in [
        "",
        "   ",
        " vault.db.v12-pre-update.bak",
        "../vault.db.v12-pre-update.bak",
        "/tmp/vault.db.v12-pre-update.bak",
        "..\\vault.db.v12-pre-update.bak",
        "alt/vault.db.v12-pre-update.bak",
    ] {
        assert_eq!(
            rollback_backup_name(bad).unwrap_err().code,
            "E_ROLLBACK_BACKUP",
            "reddedilmeli: {bad:?}"
        );
    }
}

#[test]
fn rollback_needs_the_exact_confirmation() {
    assert!(rollback_confirmed("GERİ DÖN"));
    assert!(rollback_confirmed("  GERİ DÖN  "), "kırpma hoş görülür");
    for bad in ["", "GERI DON", "geri dön", "GERİ DÖN!", "onayla"] {
        assert!(!rollback_confirmed(bad), "reddedilmeli: {bad:?}");
    }
}

#[test]
fn inventory_gate_accepts_only_docx_and_udf() {
    assert!(inventory_mime_supported(INVENTORY_MIMES[0]));
    assert!(inventory_mime_supported("application/udf"));
    for mime in [
        "application/pdf",
        "image/jpeg",
        "text/plain",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "",
    ] {
        assert!(!inventory_mime_supported(mime), "reddedilmeli: {mime:?}");
    }
}

#[test]
fn package_report_explains_a_missing_package() {
    let view = read_worker_package(Path::new("/olmayan/worker/paketi")).expect("rapor");
    assert!(!view.found);
    assert!(view.manifest.is_none() && view.verification.is_none());
    assert!(
        view.note
            .as_deref()
            .unwrap_or_default()
            .contains("geliştirme"),
        "gerekçe yazılmalı: {:?}",
        view.note
    );
}

fn package_fixture() -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("dim-worker-pkg-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(dir.join("_internal")).expect("dizin");
    std::fs::write(dir.join("dim-worker"), b"ikili").expect("ikili");
    std::fs::write(dir.join("_internal").join("base_library.zip"), b"arsiv").expect("arsiv");
    dir
}

fn write_manifest(dir: &Path, tamper: bool) {
    let binary = if tamper { "baska" } else { "ikili" };
    let manifest = serde_json::json!({
        "manifestVersion": 1,
        "workerVersion": "0.1.0",
        "mode": "onedir",
        "buildPlatform": "test-arm64",
        "pythonVersion": "3.12.0",
        "pyinstallerVersion": "6.11.1",
        "entryPoint": "dim-worker",
        "totals": {"files": 2, "symlinks": 0, "bytes": 10, "truncated": false, "unsafeLinks": 0},
        "roles": {"bootloader": 1, "stdlib_archive": 1},
        "tesseract": {"bundled": false, "note": "sistemde kurulu olmalı"},
        "files": [
            {"path": "dim-worker", "role": "bootloader", "kind": "file",
             "bytes": binary.len(), "sha256": hex::encode(Sha256::digest(binary.as_bytes()))},
            {"path": "_internal/base_library.zip", "role": "stdlib_archive", "kind": "file",
             "bytes": 5, "sha256": hex::encode(Sha256::digest(b"arsiv"))}
        ]
    });
    std::fs::write(
        dir.join(MANIFEST_FILE),
        serde_json::to_string(&manifest).expect("json"),
    )
    .expect("manifest");
}

#[test]
fn package_report_verifies_a_synthetic_package() {
    let dir = package_fixture();
    write_manifest(&dir, false);
    let view = read_worker_package(&dir).expect("rapor");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(view.found, "{:?}", view.note);
    let manifest = view.manifest.expect("manifest");
    assert_eq!(manifest.worker_version, "0.1.0");
    assert_eq!(manifest.totals.files, 2);
    assert_eq!(manifest.roles.get("bootloader"), Some(&1));
    let verification = view.verification.expect("doğrulama");
    assert!(verification.ok, "{verification:?}");
    assert_eq!(verification.verified, 2);
    assert!(verification.entry_point_present);
}

#[test]
fn package_report_reports_tampering_instead_of_passing() {
    let dir = package_fixture();
    write_manifest(&dir, true);
    let view = read_worker_package(&dir).expect("rapor");
    let _ = std::fs::remove_dir_all(&dir);
    let verification = view.verification.expect("doğrulama");
    assert!(!verification.ok, "kurcalama geçmemeli");
    assert!(verification.problems.iter().any(|p| p.path == "dim-worker"
        && p.problem == dim_ingest::packaging::PackageProblemKind::HashChanged));
}

#[test]
fn package_report_flags_a_package_without_manifest() {
    let dir = package_fixture();
    let view = read_worker_package(&dir).expect("rapor");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!view.found);
    assert!(
        view.note
            .as_deref()
            .unwrap_or_default()
            .contains("manifest"),
        "gerekçe yazılmalı: {:?}",
        view.note
    );
}

#[test]
fn directory_gets_vault_file_name() {
    let p = resolve_vault_path("DimTest").expect("yol");
    assert!(p.ends_with("DimTest/vault.dimvault") || p.ends_with("DimTest\\vault.dimvault"));
}

#[test]
fn explicit_extension_is_kept() {
    let p = resolve_vault_path("~/DimTest/ozel.dimvault").expect("yol");
    assert!(p.ends_with("ozel.dimvault"));
}

#[test]
fn case_payload_embeds_coverage_block() {
    use dim_storage::{AiPolicy, CaseKind, NewCase, Sensitivity};
    let db = Database::open_in_memory(&DatabaseKey::new([90u8; 32])).expect("db");
    let case = db
        .create_case(&NewCase {
            title: "Kapsam".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("case");
    let coverage = ExportCoverage::from_report(&db.coverage_report(&case.id).expect("rapor"));
    let payload = case_payload(&db, &case.id, &coverage).expect("yük");
    assert_eq!(
        payload["coverage"]["statement"],
        dim_storage::COVERAGE_STATEMENT
    );
    assert_eq!(payload["coverage"]["documents"], 0);
    assert_eq!(payload["coverage"]["complete"], false);
    assert_eq!(payload["coverage"]["textPages"], 0);
    // Rapor bölümü tablo başlığıyla başlar ve tüm sayımları taşır.
    let section = coverage_section(&coverage);
    assert_eq!(section["heading"], "İncelenen kapsam");
    assert_eq!(section["rows"][0][0], "Ölçü");
    assert_eq!(
        section["rows"].as_array().map(Vec::len),
        Some(coverage.rows().len() + 1)
    );
    assert_eq!(
        section["lines"].as_array().map(Vec::len),
        Some(coverage.warnings.len() + 1)
    );
    assert_eq!(
        section["lines"]
            .as_array()
            .and_then(|lines| lines.last())
            .cloned(),
        Some(serde_json::json!(dim_storage::COVERAGE_STATEMENT))
    );
    // CSV yorumları tablo başlığını kirletmez.
    assert!(coverage
        .csv_comment_block()
        .lines()
        .all(|l| l.starts_with('#')));
}
