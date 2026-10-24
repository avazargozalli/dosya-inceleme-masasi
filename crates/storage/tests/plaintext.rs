//! SQLCipher kanıtı: kasa veritabanı dosyasında ne SQLite başlığı ne de düz
//! metin dosya adı bulunur; yanlış anahtar açamaz. Bu test, güvenlik
//! matrisindeki T3 iddiasının kanıtıdır.

use dim_storage::{AiPolicy, CaseKind, Database, DatabaseKey, NewCase, Sensitivity};

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn new_case(title: &str) -> NewCase {
    NewCase {
        title: title.into(),
        case_kind: CaseKind::Review,
        reference_code: None,
        jurisdiction_label: None,
        practice_area: None,
        sensitivity: Sensitivity::Normal,
        default_ai_policy: AiPolicy::Off,
    }
}

#[test]
fn database_file_leaks_no_plaintext_and_rejects_wrong_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("vault.db");
    let key = DatabaseKey::new([9u8; 32]);
    let title = "Çok Gizli Kira Dosyası";

    {
        let db = Database::open(&path, &key).expect("aç");
        db.create_case(&new_case(title)).expect("dosya");
    }

    let bytes = std::fs::read(&path).expect("oku");
    assert!(
        !bytes.starts_with(b"SQLite format 3\0"),
        "SQLCipher başlığı açık olmamalı"
    );
    assert!(
        !contains(&bytes, title.as_bytes()),
        "dosya adı düz metin olarak sızmamalı"
    );
    assert!(
        !contains(&bytes, b"sqlite_master"),
        "şema adları düz metin olmamalı"
    );

    let wrong = DatabaseKey::new([10u8; 32]);
    assert!(
        Database::open(&path, &wrong).is_err(),
        "yanlış anahtar açmamalı"
    );
}
