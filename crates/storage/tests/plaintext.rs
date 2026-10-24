//! F01-T03 — SQLCipher, FTS5, WAL ve geçici depolamada açık metin kalmadığının
//! **işaretleyicilerle** incelenmesi.
//!
//! Her test iki şeyi birlikte kanıtlar: (1) işaretçi gerçekten kasanın içine
//! yazıldı (sorguyla geri okunur, FTS indeksinde bulunur), (2) aynı işaretçi
//! diskteki baytlarda bulunamaz. Yalnız (2) ölçülseydi "hiç yazılmamış" bir
//! içerik de testi geçerdi; bu yüzden (1) her testte ayrıca doğrulanır.
//!
//! Kapsam sınırları (dürüst):
//! - İnceleme **bayt araması**dır: şifreli sayfaların çözülememesi kanıtlanır,
//!   kriptografik analiz yapılmaz.
//! - Geçici depolama, `PRAGMA temp_store` değeri + gözlemlenen dosya izi ile
//!   ölçülür; SQLite'ın her sürümde/derlemede aynı davranacağı kanıtlanmaz.
//! - `-wal`/`-shm` yan dosyaları yalnız bağlantı **açıkken** anlamlıdır; kapanış
//!   WAL'ı checkpoint edip sıfırlar, bu yüzden tarama kapanıştan önce yapılır.
//! - Python worker'ının geçici dizini bu dosyanın kapsamı dışındadır; ayrı test
//!   `crates/ingest/tests/worker_temp_plaintext.rs` içinde koşar.

use std::path::{Path, PathBuf};

use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, NewCase, NewPage, SegmentBlock, Sensitivity,
};

/// Bayt aramasında kullanılan işaretçi. Yeterince uzun ve benzersiz: rastgele
/// bir şifreli sayfada bu dizinin tesadüfen oluşma olasılığı ihmal edilebilir.
const MARKER: &str = "ZZ-KIRA-MARKER-7F3A9C";
/// FTS indeksinde tek belirteç olarak bulunacak biçim (ayırıcısız), böylece
/// arama sonucu "işaretçi indekste" kanıtı olur.
const FTS_TOKEN: &str = "ZZKIRAMARKERZZ";

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

fn marker_text() -> String {
    format!("Kira bedeli ödenmedi. {MARKER} {FTS_TOKEN} ihtar 12.03.2026 tarihinde tebliğ edildi.")
}

fn table_count(db: &Database, sql: &str) -> i64 {
    db.conn()
        .query_row(sql, [], |row| row.get(0))
        .expect("sayım")
}

fn table_exists(db: &Database, name: &str) -> bool {
    table_count(
        db,
        &format!("SELECT count(*) FROM sqlite_master WHERE name='{name}'"),
    ) > 0
}

fn read_optional(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Kasanın içine işaretçili bir belge yazar: belge → sürüm → ayrıştırma koşusu
/// → sayfalar (FTS tetikleyicisi indeksi doldurur). Dönen kimlikler testin
/// işaretçiyi sorguyla geri okumasını sağlar.
fn seed_indexed_text(db: &Database, case_id: &str) -> (String, String) {
    let document = db
        .create_document(case_id, "kurgu-kira.txt", "text", None, "unknown")
        .expect("belge");
    let blob_id = db
        .create_blob_record(&uuid::Uuid::now_v7().to_string(), 128, "test")
        .expect("blob");
    let version = db
        .add_version(
            &document.id,
            case_id,
            &blob_id,
            &"a".repeat(64),
            128,
            "text/plain",
        )
        .expect("sürüm");
    let run = db
        .start_parse_run(&version.id, case_id, "plain", "2", "text/plain|ocr=false")
        .expect("koşu");
    let text = marker_text();
    let stored = db
        .store_pages(
            &run.id,
            case_id,
            &[NewPage {
                index: 0,
                text: text.clone(),
                width: None,
                height: None,
                origin: "native".into(),
                warnings: Vec::new(),
                rotation: 0,
                coordinate_space: None,
                page_label: None,
                blocks: vec![SegmentBlock {
                    start: 0,
                    end: i64::try_from(text.chars().count()).expect("uzunluk"),
                    bbox: None,
                    lines: Vec::new(),
                    path: None,
                    kind: None,
                }],
            }],
        )
        .expect("sayfa");
    assert_eq!(stored, 1, "metinli sayfa yazılmalı");
    db.finish_parse_run(&run.id, "succeeded", &[])
        .expect("bitir");
    (version.id, run.id)
}

/// Dizin ağacındaki dosyalardan işaretçiyi taşıyanları döndürür.
fn files_containing(dir: &Path, needle: &[u8]) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => stack.push(path),
                Ok(kind) if kind.is_file() => {
                    if let Ok(bytes) = std::fs::read(&path) {
                        if contains(&bytes, needle) {
                            hits.push(path);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    hits
}

fn assert_clean(bytes: &[u8], where_: &str) {
    assert!(
        !contains(bytes, MARKER.as_bytes()),
        "{where_}: işaretçi düz metin olarak sızmamalı"
    );
    assert!(
        !contains(bytes, FTS_TOKEN.as_bytes()),
        "{where_}: FTS belirteci düz metin olarak sızmamalı"
    );
    assert!(
        !bytes.starts_with(b"SQLite format 3\0"),
        "{where_}: SQLCipher başlığı açık olmamalı"
    );
    assert!(
        !contains(bytes, b"sqlite_master"),
        "{where_}: şema adları düz metin olmamalı"
    );
    assert!(
        !contains(bytes, b"text_segments"),
        "{where_}: FTS/gölge tablo adları düz metin olmamalı"
    );
}

#[test]
fn database_file_leaks_no_plaintext_and_rejects_wrong_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("vault.db");
    let key = DatabaseKey::new([9u8; 32]);
    let title = "Çok Gizli Kira Dosyası";

    let bytes = {
        let db = Database::open(&path, &key).expect("aç");
        let case = db.create_case(&new_case(title)).expect("dosya");
        seed_indexed_text(&db, &case.id);
        std::fs::read(&path).expect("oku")
    };

    assert_clean(&bytes, "kasa dosyası");
    assert!(
        !contains(&bytes, title.as_bytes()),
        "dosya adı düz metin olarak sızmamalı"
    );

    let wrong = DatabaseKey::new([10u8; 32]);
    assert!(
        Database::open(&path, &wrong).is_err(),
        "yanlış anahtar açmamalı"
    );
}

#[test]
fn fts5_index_holds_marker_but_file_bytes_do_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("vault.db");
    let key = DatabaseKey::new([7u8; 32]);
    let db = Database::open(&path, &key).expect("aç");
    let case = db.create_case(&new_case("FTS kapsamı")).expect("dosya");
    seed_indexed_text(&db, &case.id);

    // (1) İşaretçi gerçekten indekste: FTS üzerinden bulunur.
    let hits = db.search_segments(&case.id, FTS_TOKEN, 10).expect("arama");
    assert_eq!(hits.len(), 1, "FTS işaretçiyi bulmalı (indeks dolu)");
    assert!(hits[0].raw_text.contains(MARKER));

    // Gölge tablolar var ve dolu — yani taranan şey boş bir indeks değil.
    for shadow in [
        "text_segments_fts_data",
        "text_segments_fts_idx",
        "text_segments_fts_docsize",
        "text_segments_fts_config",
    ] {
        assert!(table_exists(&db, shadow), "{shadow} bulunmalı");
    }
    assert!(
        table_count(&db, "SELECT count(*) FROM text_segments_fts_data") > 0,
        "FTS gölge tablosu boş olmamalı"
    );
    assert!(
        table_count(&db, "SELECT count(*) FROM text_segments_fts_idx") > 0,
        "FTS terim indeksi boş olmamalı"
    );
    assert!(
        table_count(
            &db,
            "SELECT count(*) FROM text_segments_fts WHERE text_segments_fts MATCH 'ZZKIRAMARKERZZ'"
        ) == 1,
        "FTS MATCH doğrudan indekste eşleşmeli"
    );

    // (2) Aynı işaretçi dosya baytlarında yok.
    let bytes = std::fs::read(&path).expect("oku");
    assert_clean(&bytes, "kasa dosyası (FTS dolu)");
}

#[test]
fn wal_and_shm_sidecars_hold_no_plaintext() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("vault.db");
    let key = DatabaseKey::new([5u8; 32]);
    let db = Database::open(&path, &key).expect("aç");

    // Yapılandırma kanıtı: WAL gerçekten açık, yoksa yan dosya taraması tiyatro olur.
    let journal: String = db
        .conn()
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal_mode");
    assert_eq!(journal.to_lowercase(), "wal", "WAL kipi açık olmalı");

    let case = db.create_case(&new_case("WAL kapsamı")).expect("dosya");
    seed_indexed_text(&db, &case.id);

    // (1) İşaretçi mantıksal olarak yazılmış: WAL'da taşınan commit'ler olmasa
    // sorgu sonuç vermezdi.
    let hits = db.search_segments(&case.id, FTS_TOKEN, 10).expect("arama");
    assert_eq!(hits.len(), 1);

    // (2) Bağlantı **açıkken** yan dosyalar taranır (kapanış WAL'ı sıfırlar).
    let wal_path = dir.path().join("vault.db-wal");
    let shm_path = dir.path().join("vault.db-shm");
    let wal = read_optional(&wal_path).expect("WAL dosyası bağlantı açıkken bulunmalı");
    assert!(
        wal.len() >= 4096,
        "WAL boş olmamalı (aksi hâlde tarama kanıt değil); {} bayt",
        wal.len()
    );
    assert_clean(&wal, "vault.db-wal");

    let shm = read_optional(&shm_path).expect("SHM dosyası bağlantı açıkken bulunmalı");
    assert_clean(&shm, "vault.db-shm");

    // (3) Açık bağlantının yanında kasa dizininde başka dosya varsa o da temiz olmalı.
    let leaked = files_containing(dir.path(), MARKER.as_bytes());
    assert!(
        leaked.is_empty(),
        "kasa dizininde düz metin sızıntısı: {leaked:?}"
    );
}

#[test]
fn deleted_content_is_scrubbed_from_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("vault.db");
    let key = DatabaseKey::new([3u8; 32]);
    let db = Database::open(&path, &key).expect("aç");
    let case = db.create_case(&new_case("Silme kapsamı")).expect("dosya");
    seed_indexed_text(&db, &case.id);

    assert_eq!(
        db.search_segments(&case.id, FTS_TOKEN, 10)
            .expect("arama")
            .len(),
        1,
        "silmeden önce bulunmalı"
    );

    // Kaskad silme: parse_runs → pages/text_segments → FTS tetikleyicisi.
    db.conn()
        .execute("DELETE FROM cases WHERE id=?1", [&case.id])
        .expect("dosya sil");

    assert!(
        db.search_segments(&case.id, FTS_TOKEN, 10)
            .expect("arama")
            .is_empty(),
        "silinen içerik aramada görünmemeli"
    );

    // FTS5 harici içerikli tabloda silme, bölütü hemen yok etmez: indekse
    // "silindi" işareti yazılır, fiziksel birleştirme (`optimize`/`rebuild`)
    // sonraya kalır. Bu yüzden satır sayısı sıfır **beklenmez**; beklenen şey
    // kalıntının aramada görünmemesi ve baytların şifreli kalmasıdır. Ölçülen
    // sayı, kalıntının ne kadar sürdüğünü görünür kılmak için raporlanır.
    let residual_before = table_count(&db, "SELECT count(*) FROM text_segments_fts_data");
    db.conn()
        .execute(
            "INSERT INTO text_segments_fts(text_segments_fts) VALUES('optimize')",
            [],
        )
        .expect("indeks birleştirme");
    let residual_after = table_count(&db, "SELECT count(*) FROM text_segments_fts_data");
    eprintln!(
        "FTS kalıntı bölütü: silme sonrası {residual_before}, optimize sonrası {residual_after}"
    );
    assert!(
        residual_after <= residual_before,
        "birleştirme kalıntıyı büyütmemeli"
    );
    assert!(
        db.search_segments(&case.id, FTS_TOKEN, 10)
            .expect("arama")
            .is_empty(),
        "birleştirme sonrası da silinen içerik bulunmamalı"
    );

    // `PRAGMA secure_delete = ON` beklenir; silinen içerik dosyada kalmamalı.
    let secure_delete: i64 = db
        .conn()
        .query_row("PRAGMA secure_delete", [], |row| row.get(0))
        .expect("secure_delete");
    assert_eq!(secure_delete, 1, "secure_delete açık olmalı");

    let bytes = std::fs::read(&path).expect("oku");
    assert_clean(&bytes, "kasa dosyası (silme sonrası)");
}

#[test]
fn planted_marker_is_detected_by_the_scan() {
    // Dedektör kanıtı: yukarıdaki "sızıntı yok" sonucu körlük değil. Aynı
    // tarama, kasıtlı olarak bırakılan işaretçiyi bulmalı.
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("temiz.txt"), b"ilgisiz icerik").expect("temiz");
    let nested = dir.path().join("alt");
    std::fs::create_dir(&nested).expect("alt dizin");
    std::fs::write(nested.join("sizinti.txt"), format!("{MARKER} metin")).expect("sizinti");

    let found = files_containing(dir.path(), MARKER.as_bytes());
    assert_eq!(found.len(), 1, "tam olarak bir sızıntı bulunmalı");
    assert!(found[0].ends_with("sizinti.txt"));
    assert!(
        files_containing(dir.path(), b"SQLite format 3\0").is_empty(),
        "olmayan bir imza bulunmamalı"
    );
}

#[test]
fn temp_storage_is_memory_and_leaves_no_marker_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("vault.db");
    let key = DatabaseKey::new([2u8; 32]);
    let db = Database::open(&path, &key).expect("aç");
    let case = db
        .create_case(&new_case("Geçici depo kapsamı"))
        .expect("dosya");
    seed_indexed_text(&db, &case.id);

    let temp_store: i64 = db
        .conn()
        .query_row("PRAGMA temp_store", [], |row| row.get(0))
        .expect("temp_store");
    assert_eq!(
        temp_store, 2,
        "geçici tablolar bellekte tutulmalı (temp_store = MEMORY)"
    );

    let temp_dir = std::env::temp_dir();
    let sqlite_temp_files = |dir: &Path| -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("etilqs_"))
            })
            .collect()
    };
    let before = sqlite_temp_files(&temp_dir);

    // Bellek içi geçici tablo + büyük sıralama: temp_store = MEMORY iken disk
    // dosyası doğmamalı. İşaretçi geçici tabloya da yazılır.
    db.conn()
        .execute_batch(&format!(
            "CREATE TEMP TABLE probe(x TEXT);
             INSERT INTO probe SELECT '{MARKER}' FROM (
               WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM c WHERE i<5000)
               SELECT i FROM c);
             SELECT count(*) FROM (SELECT x FROM probe ORDER BY random());"
        ))
        .expect("geçici tablo");
    db.conn()
        .execute("DROP TABLE temp.probe", [])
        .expect("geçici tablo sil");

    let after = sqlite_temp_files(&temp_dir);
    let created: Vec<_> = after.iter().filter(|path| !before.contains(path)).collect();
    assert!(
        created.is_empty(),
        "temp_store=MEMORY iken SQLite geçici dosyası doğmamalı: {created:?}"
    );

    // Kasa dizininde işaretçi taşıyan tek dosya bile kalmamalı.
    let leaked = files_containing(dir.path(), MARKER.as_bytes());
    assert!(
        leaked.is_empty(),
        "geçici iş sırasında düz metin sızıntısı: {leaked:?}"
    );
}
