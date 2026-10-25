//! F16-T03 — atomik güncelleme / geri dönüş tatbikatı.
//!
//! Tatbikat gerçek dosyalarla çalışır: eski şemada (v12) bir kasa kurulur,
//! güncelleme uygulanır, migrasyon yarıda bırakılır, yedek bozulur ve geri
//! dönüş denenir. Hiçbir adım taklit edilmez; her adım gerçek SQLCipher
//! dosyasını açar ve okur.

use super::*;

fn key() -> DatabaseKey {
    DatabaseKey::new([9u8; 32])
}

/// Güncelleme öncesi durumu kurar: `CURRENT_SCHEMA_VERSION - 1` (v12) şeması,
/// gerçek migrasyon dosyaları ve bir dosya kaydıyla.
fn create_v12(path: &Path) {
    let conn = Connection::open(path).expect("dosya açılır");
    apply_key_and_pragmas(&conn, &key()).expect("anahtar");
    parsing::register_search_functions(&conn).expect("arama fonksiyonları");
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
           version INTEGER PRIMARY KEY,
           name TEXT NOT NULL,
           checksum TEXT NOT NULL,
           applied_at TEXT NOT NULL
         );",
    )
    .expect("sürüm tablosu");
    for migration in MIGRATIONS.iter().filter(|m| m.version <= 12) {
        conn.execute_batch(migration.sql).expect("eski migrasyon");
        conn.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                migration.version,
                migration.name,
                hex::encode(Sha256::digest(migration.sql.as_bytes())),
                now_iso()
            ],
        )
        .expect("sürüm kaydı");
    }
    conn.execute(
        "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
         VALUES ('c-eski','Eski dosya','review','active','normal','off','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    )
    .expect("dosya kaydı");
    // WAL kalıntısı bırakma: sonraki adımlar dosyayı tek başına okuyabilsin.
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;");
}

fn case_titles(path: &Path) -> Vec<String> {
    let version = Database::verify_database_file(path, &key()).expect("doğrula");
    assert!(version <= CURRENT_SCHEMA_VERSION);
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("aç");
    apply_key_and_pragmas(&conn, &key()).expect("anahtar");
    let mut stmt = conn
        .prepare("SELECT title FROM cases ORDER BY title")
        .expect("hazırla");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("sorgu");
    rows.map(|r| r.expect("satır")).collect()
}

#[test]
fn upgrade_takes_verified_backup_before_migrating() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    let backup = pre_migration_backup_path(&path, 12);
    assert!(!backup.exists(), "güncellemeden önce yedek olmamalı");

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    assert_eq!(db.pre_migration_backup(), Some(backup.as_path()));
    assert!(backup.is_file(), "yedek dosyası yazılmalı");
    assert_eq!(
        Database::verify_database_file(&backup, &key()).expect("yedek doğrulanır"),
        12,
        "yedek eski şemayı taşımalı"
    );
    drop(db);

    // Güncelleme veriyi korur, yedek de eski veriyi tek başına taşır.
    assert_eq!(case_titles(&path), vec!["Eski dosya".to_string()]);
    assert_eq!(case_titles(&backup), vec!["Eski dosya".to_string()]);
}

#[test]
fn upgrade_does_not_start_when_backup_cannot_be_written() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    // Yedek yolunda bir klasör var: yedek yazılamaz.
    std::fs::create_dir_all(pre_migration_backup_path(&path, 12)).expect("engel");

    let err = Database::open(&path, &key()).expect_err("yedeksiz güncelleme yok");
    assert_eq!(err.code(), "E_DB_MIGRATION_BACKUP");
    assert_eq!(
        Database::verify_database_file(&path, &key()).expect("dosya sağlam"),
        12,
        "güncelleme başlamamalı"
    );
}

#[test]
fn failed_migration_keeps_old_version_and_backup_stays_usable() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    {
        let conn = Connection::open(&path).expect("aç");
        apply_key_and_pragmas(&conn, &key()).expect("anahtar");
        // v13'ün oluşturacağı indeks adını önceden kap: ALTER'lar geçer,
        // CREATE INDEX patlar → migrasyon yarıda kalır.
        conn.execute_batch("CREATE INDEX precedent_versions_by_precedent ON cases(id);")
            .expect("çakışma");
    }

    let err = Database::open(&path, &key()).expect_err("migrasyon düşer");
    assert_eq!(err.code(), "E_DB_MIGRATION");
    assert_eq!(
        Database::verify_database_file(&path, &key()).expect("dosya sağlam"),
        12,
        "yarım migrasyon dosyada iz bırakmamalı"
    );
    let backup = pre_migration_backup_path(&path, 12);
    assert_eq!(
        Database::verify_database_file(&backup, &key()).expect("yedek doğrulanır"),
        12
    );
    assert_eq!(case_titles(&backup), vec!["Eski dosya".to_string()]);
}

#[test]
fn newer_schema_is_refused_before_any_write() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    {
        let conn = Connection::open(&path).expect("aç");
        apply_key_and_pragmas(&conn, &key()).expect("anahtar");
        // “Gelecek” sürüm daima mevcut şemanın bir üstüdür; sabit sayı
        // yazmak her yeni migrasyonda testi yanlış yönde kırıyordu.
        let future = crate::CURRENT_SCHEMA_VERSION + 1;
        conn.execute(
            &format!(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES ({future}, '00{future}_future', 'x', '2026-01-01T00:00:00Z')"
            ),
            [],
        )
        .expect("gelecek sürüm");
    }

    let err = Database::open(&path, &key()).expect_err("yeni şema reddedilir");
    assert_eq!(err.code(), "E_DB_SCHEMA_NEWER");

    // Ne v13 uygulanmış ne de v13 sütunları eklenmiş olmalı.
    let conn = Connection::open(&path).expect("aç");
    apply_key_and_pragmas(&conn, &key()).expect("anahtar");
    let has_13: i64 = conn
        .query_row(
            "SELECT count(*) FROM schema_migrations WHERE version = 13",
            [],
            |row| row.get(0),
        )
        .expect("say");
    assert_eq!(has_13, 0);
    let columns: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('precedent_versions') WHERE name = 'full_text'",
            [],
            |row| row.get(0),
        )
        .expect("sütunlar");
    assert_eq!(columns, 0, "yeni şema yazılmamalı");
    let conflict_columns: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_table_info('precedent_versions') WHERE name = 'identity_conflict'",
            [],
            |row| row.get(0),
        )
        .expect("sütunlar");
    assert_eq!(conflict_columns, 0, "v14 de yazılmamalı");
}

#[test]
fn rollback_restores_previous_version_and_keeps_safety_copy() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);

    let db = Database::open(&path, &key()).expect("güncelle");
    db.conn()
        .execute(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c-yeni','Yeni dosya','review','active','normal','off','2026-01-02T00:00:00Z','2026-01-02T00:00:00Z')",
            [],
        )
        .expect("yeni kayıt");
    let backup = db.pre_migration_backup().expect("yedek").to_path_buf();
    drop(db);

    let outcome = rollback_vault(&path, &backup, &key()).expect("geri dönüş");
    assert_eq!(outcome.restored_version, 12);
    assert!(outcome.safety_copy.is_file(), "güvenlik kopyası kalmalı");

    assert_eq!(case_titles(&path), vec!["Eski dosya".to_string()]);
    assert_eq!(
        case_titles(&outcome.safety_copy),
        vec!["Eski dosya".to_string(), "Yeni dosya".to_string()],
        "güvenlik kopyası güncelleme sonrası veriyi korur"
    );
}

#[test]
fn in_app_rollback_restores_old_data_and_resumes_current_schema() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);

    let db = Database::open(&path, &key()).expect("güncelle");
    assert_eq!(db.schema_version(), CURRENT_SCHEMA_VERSION);
    db.conn()
        .execute(
            "INSERT INTO cases (id,title,case_kind,status,sensitivity,default_ai_policy,created_at,updated_at)
             VALUES ('c-yeni','Yeni dosya','review','active','normal','off','2026-01-02T00:00:00Z','2026-01-02T00:00:00Z')",
            [],
        )
        .expect("yeni kayıt");
    drop(db);

    // Uygulamanın gördüğü liste: kasanın yanındaki yedek, doğrulanmış sürümüyle.
    let list = pre_update_backups(&path, &key()).expect("liste");
    assert_eq!(list.len(), 1, "güncelleme öncesi yedek listelenir");
    assert_eq!(list[0].claimed_version, 12);
    assert_eq!(list[0].verified_version, Some(12));
    assert!(list[0].usable());
    assert!(!list[0].name_mismatch());

    let resumed = rollback_and_resume(&path, &list[0].path, &key()).expect("geri dönüş");
    assert_eq!(resumed.restored_version, 12);
    assert!(resumed.safety_copy.is_file(), "güvenlik kopyası kalır");
    assert_eq!(
        resumed.database.schema_version(),
        CURRENT_SCHEMA_VERSION,
        "kasa güncel şemaya ilerletilir"
    );

    // Geri dönen kasa yalnız güncelleme öncesi veriyi taşır; güncelleme
    // sonrası kayıt güvenlik kopyasında durur.
    let titles: Vec<String> = {
        let mut stmt = resumed
            .database
            .conn()
            .prepare("SELECT title FROM cases ORDER BY title")
            .expect("hazırla");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("sorgu");
        rows.map(|r| r.expect("satır")).collect()
    };
    assert_eq!(titles, vec!["Eski dosya".to_string()]);
    assert_eq!(
        case_titles(&resumed.safety_copy),
        vec!["Eski dosya".to_string(), "Yeni dosya".to_string()]
    );

    // Yedek dosyası yerinde kalır: geri dönüş onu tüketmez.
    assert!(list[0].path.is_file(), "yedek silinmez");

    // Denetim kaydı geri konan kasada, sürümlerle; dosya yolu yazılmaz.
    let (category, operation): (String, String) = resumed
        .database
        .conn()
        .query_row(
            "SELECT category, operation FROM audit_events WHERE category = 'vault_rollback' ORDER BY sequence DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("denetim");
    assert_eq!(category, "vault_rollback");
    assert_eq!(operation, "completed");
    let metadata: String = resumed
        .database
        .conn()
        .query_row(
            "SELECT metadata_allowlist FROM audit_events WHERE category = 'vault_rollback' ORDER BY sequence DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("meta");
    assert!(
        metadata.contains("\"restoredVersion\":12"),
        "meta: {metadata}"
    );
    assert!(
        !metadata.contains("vault.db"),
        "denetime dosya yolu yazılmaz: {metadata}"
    );
}

#[test]
fn rollback_refuses_corrupt_backup_and_leaves_vault_untouched() {
    let dir = tempfile::tempdir().expect("dizin");
    let path = dir.path().join("vault.db");
    create_v12(&path);
    let db = Database::open(&path, &key()).expect("güncelle");
    let backup = db.pre_migration_backup().expect("yedek").to_path_buf();
    drop(db);

    std::fs::write(&backup, b"bozuk yedek").expect("boz");
    let err = rollback_vault(&path, &backup, &key()).expect_err("bozuk yedek reddedilir");
    assert_eq!(err.code(), "E_DB_MIGRATION_BACKUP");

    assert_eq!(
        Database::verify_database_file(&path, &key()).expect("kasa sağlam"),
        CURRENT_SCHEMA_VERSION,
        "mevcut kasa bozulmamalı"
    );
    assert_eq!(case_titles(&path), vec!["Eski dosya".to_string()]);
}
