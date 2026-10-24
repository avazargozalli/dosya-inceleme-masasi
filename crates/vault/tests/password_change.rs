//! Parola değişimi: ana anahtar korunur, eski parola kapanır, kurtarma
//! anahtarı geçerli kalır, zayıf parola reddedilir.

use dim_vault::VaultFile;

fn path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("v.dimvault");
    (dir, file)
}

#[test]
fn password_change_keeps_master_key_and_recovery() {
    let (_dir, file) = path();
    let (_vault, recovery, _unlocked) =
        VaultFile::create(&file, "eski-parola-123").expect("create");
    let before = VaultFile::open(&file, "eski-parola-123")
        .expect("eski parola")
        .master_key()
        .to_vec();

    VaultFile::change_password(&file, "eski-parola-123", "yeni-parola-456").expect("değişim");

    assert_eq!(
        VaultFile::open(&file, "eski-parola-123")
            .unwrap_err()
            .code(),
        "E_VAULT_CREDENTIALS"
    );
    let after = VaultFile::open(&file, "yeni-parola-456")
        .expect("yeni parola")
        .master_key()
        .to_vec();
    assert_eq!(before, after, "ana anahtar değişmemeli");

    let via_recovery =
        VaultFile::open_with_recovery(&file, &recovery.to_display_string()).expect("kurtarma");
    assert_eq!(via_recovery.master_key().to_vec(), before);
}

#[test]
fn weak_password_rejected_without_touching_header() {
    let (_dir, file) = path();
    let (_vault, _recovery, _unlocked) =
        VaultFile::create(&file, "eski-parola-123").expect("create");
    let before = std::fs::read(&file).expect("oku");

    assert_eq!(
        VaultFile::change_password(&file, "eski-parola-123", "kisa")
            .unwrap_err()
            .code(),
        "E_PASSWORD_WEAK"
    );
    assert_eq!(
        std::fs::read(&file).expect("oku"),
        before,
        "başlık değişmemeli"
    );
    assert!(VaultFile::open(&file, "eski-parola-123").is_ok());
}

#[test]
fn wrong_current_password_is_rejected() {
    let (_dir, file) = path();
    let (_vault, _recovery, _unlocked) =
        VaultFile::create(&file, "eski-parola-123").expect("create");
    assert_eq!(
        VaultFile::change_password(&file, "yanlis-parola", "yeni-parola-456")
            .unwrap_err()
            .code(),
        "E_VAULT_CREDENTIALS"
    );
    assert!(VaultFile::open(&file, "eski-parola-123").is_ok());
}
