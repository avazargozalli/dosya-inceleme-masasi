//! F01-T06 — DOCX/UDF envanterinin **gerçek worker** ile çıkarılması.
//!
//! Fikstürler (`fixtures/synthetic/docx-inventory.docx`, `udf-inventory.udf`)
//! bağımlılıksız üretilir (`fixtures/synthetic/generate.py`) ve kurgu
//! işaretçiler taşır. Kanıt iki yönlüdür:
//!
//! 1. İşaretçiler **kaynakta gerçekten var** — dosya boş değil, envanterin
//!    saydığı şeyler paket içinde duruyor;
//! 2. envanter yanıtı bu işaretçilerin **hiçbirini** taşımıyor (içerik dışarı
//!    verilmiyor), buna karşın öğe sayıları ve ek listesi geliyor.
//!
//! Yalnız (2) ölçülseydi "worker hiç çalışmadı" durumu da testi geçerdi.

use std::path::PathBuf;

use dim_ingest::worker::{dev_python_with_module, dev_worker_dir, WorkerClient};

const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const UDF_MIME: &str = "application/udf";

/// Fikstürlerdeki kurgu işaretçiler: yanıtta görünmemeleri gerekir.
const MARKERS: [&str; 8] = [
    "ZZ-OLE-ICERIK-GIZLI",
    "ZZ-GORSEL-ICERIK",
    "ZZ-ALTCHUNK-ICERIK",
    "ZZ-IMZA-PARCASI",
    "ZZ-UDF-GORSEL",
    "ZZ-UDF-IMZA",
    "ZZ-UDF-PDF",
    "Kira sozlesmesi 2026",
];

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/synthetic")
        .join(name)
}

/// Envanter python-docx/PyMuPDF gerektirmez; yalnız worker klasörü ve
/// `dim_worker` modülünü içe aktarabilen yorumlayıcı gerekir.
fn worker_context() -> Option<(PathBuf, PathBuf)> {
    let worker_dir = dev_worker_dir()?;
    let python = dev_python_with_module(&worker_dir, "dim_worker")?;
    Some((python, worker_dir))
}

fn contains(haystack: &str, needle: &str) -> bool {
    haystack.contains(needle)
}

#[test]
fn docx_inventory_counts_structure_attachments_and_changes() {
    let Some((python, worker_dir)) = worker_context() else {
        eprintln!("worker klasörü veya python yok; test atlandı");
        return;
    };
    let data = std::fs::read(fixture("docx-inventory.docx")).expect("fikstür okunmalı");
    let raw = String::from_utf8_lossy(&data).to_string();

    // (1) Paket gerçekten bu içeriği taşıyor mu? Zip sıkıştırması yüzünden
    // işaretçiler ham baytta görünmeyebilir; parçaları worker'a soruyoruz.
    let mut worker = WorkerClient::spawn_dev(&python, &worker_dir).expect("worker başlamalı");
    let inventory = worker
        .inventory(&data, DOCX_MIME)
        .expect("DOCX envanteri çıkarılmalı");
    worker.shutdown();

    assert_eq!(inventory.kind, "docx");
    assert_eq!(inventory.structure["tables"], 1);
    assert_eq!(inventory.structure["cells"], 2);
    assert_eq!(inventory.structure["textBoxes"], 1);
    assert_eq!(inventory.structure["contentControls"], 1);
    assert_eq!(inventory.structure["footnotes"], 1);
    assert_eq!(inventory.structure["comments"], 1);
    assert_eq!(inventory.structure["mediaParts"], 1);

    assert_eq!(inventory.changes.insertions, 1);
    assert_eq!(inventory.changes.deletions, 1);
    assert_eq!(inventory.changes.comments, 1);
    assert_eq!(inventory.changes.track_revisions, Some(true));
    assert_eq!(inventory.changes.revision.as_deref(), Some("12"));
    assert_eq!(inventory.changes.signatures, 1);
    assert_eq!(inventory.external_links, 1);

    let kinds: Vec<(&str, &str)> = inventory
        .attachments
        .iter()
        .map(|item| (item.name.as_str(), item.kind.as_str()))
        .collect();
    assert!(kinds.contains(&("word/embeddings/oleObject1.bin", "ole_object")));
    assert!(kinds.contains(&("word/media/image1.png", "image")));
    assert!(kinds.contains(&("word/afchunk1.mht", "alt_chunk")));
    assert!(inventory.attachment_bytes > 0);
    assert!(!inventory.notes.is_empty());

    // (2) İçerik sızmıyor: yanıtın hiçbir alanında işaretçi yok.
    let serialized = serde_json::to_string(&inventory).expect("serileşmeli");
    for marker in MARKERS {
        assert!(
            !contains(&serialized, marker),
            "envanter {marker} içermemeli (içerik dışarı verilmemeli)"
        );
    }
    // Ham dosya işaretçiyi taşıyor mu? (sıkıştırılmış zip'te görünmese de
    // bayt düzeyinde arayalım; bulunamazsa bu bir kanıt değil, bilgi.)
    let raw_has_marker = MARKERS.iter().any(|marker| contains(&raw, marker));
    eprintln!("ham zip baytında işaretçi görünür mü: {raw_has_marker}");
}

#[test]
fn udf_inventory_counts_elements_and_marks_signature_as_unverified() {
    let Some((python, worker_dir)) = worker_context() else {
        eprintln!("worker klasörü veya python yok; test atlandı");
        return;
    };
    let data = std::fs::read(fixture("udf-inventory.udf")).expect("fikstür okunmalı");
    let mut worker = WorkerClient::spawn_dev(&python, &worker_dir).expect("worker başlamalı");
    let inventory = worker
        .inventory(&data, UDF_MIME)
        .expect("UDF envanteri çıkarılmalı");
    worker.shutdown();

    assert_eq!(inventory.kind, "udf");
    assert_eq!(inventory.structure["pages"], 1);
    assert_eq!(inventory.structure["tables"], 1);
    assert_eq!(inventory.structure["images"], 1);
    assert_eq!(inventory.structure["unknown"], 1);
    assert_eq!(inventory.attachment_count, 3);
    assert_eq!(inventory.changes.signatures, 1);
    // UDF'te izlenen değişiklik yok: alan uydurulmuyor.
    assert_eq!(inventory.changes.insertions, 0);
    assert_eq!(inventory.changes.track_revisions, None);
    assert!(inventory
        .notes
        .iter()
        .any(|note| note.contains("kriptografik doğrulama yapılmadı")));

    let serialized = serde_json::to_string(&inventory).expect("serileşmeli");
    for marker in MARKERS {
        assert!(
            !contains(&serialized, marker),
            "envanter {marker} içermemeli"
        );
    }
}

#[test]
fn unsupported_kind_is_refused_with_a_code() {
    let Some((python, worker_dir)) = worker_context() else {
        eprintln!("worker klasörü veya python yok; test atlandı");
        return;
    };
    let mut worker = WorkerClient::spawn_dev(&python, &worker_dir).expect("worker başlamalı");
    let error = worker
        .inventory(b"%PDF-1.4", "application/pdf")
        .expect_err("PDF için envanter reddedilmeli");
    worker.shutdown();
    match error {
        dim_ingest::worker::WorkerError::Remote { code, .. } => {
            assert_eq!(code, "E_INVENTORY_UNSUPPORTED");
        }
        other => panic!("beklenmeyen hata: {other:?}"),
    }
}
