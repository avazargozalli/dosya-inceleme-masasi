use dim_vault::package::{open, seal, seal_dir, zip_dir, PACKAGE_MAGIC};
use std::fs;
use std::path::{Path, PathBuf};

/// Gerçek bir devir paketi klasörü: iki dosya + açık metin işaretçisi.
fn package_dir(root: &Path) -> PathBuf {
    let dir = root.join("DimDevir-202611051200");
    fs::create_dir_all(&dir).expect("klasör");
    fs::write(dir.join("case.json"), br#"{"title":"ISARETCI-ACIK-METIN"}"#).expect("case");
    fs::write(dir.join("KAPSAM.md"), "# kapsam\nISARETCI-ACIK-METIN\n").expect("kapsam");
    dir
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn package_roundtrip_and_header_shape() {
    let plain = b"devir paketi icerigi".to_vec();
    let sealed = seal(&plain, "paket-parolasi-1").expect("seal");
    assert_eq!(&sealed[..8], PACKAGE_MAGIC);
    assert_ne!(sealed[16..].to_vec(), plain);
    assert_eq!(open(&sealed, "paket-parolasi-1").expect("open"), plain);
}

#[test]
fn wrong_password_and_tamper_are_rejected() {
    let sealed = seal(b"gizli", "paket-parolasi-1").expect("seal");
    assert_eq!(
        open(&sealed, "yanlis-parola").unwrap_err().code(),
        "E_VAULT_CREDENTIALS"
    );

    let mut tampered = sealed.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    assert_eq!(
        open(&tampered, "paket-parolasi-1").unwrap_err().code(),
        "E_VAULT_CREDENTIALS"
    );
}

#[test]
fn weak_password_rejected() {
    assert_eq!(seal(b"x", "kisa").unwrap_err().code(), "E_PASSWORD_WEAK");
}

#[test]
fn foreign_bytes_are_rejected() {
    assert_eq!(
        open(b"not-a-package", "paket-parolasi-1")
            .unwrap_err()
            .code(),
        "E_VAULT_HEADER"
    );
}

// --- `seal_dir`: açık metin kalıntısı bırakmayan paketleme (F04-T08) ---------

/// Mutlu yol: klasör `.dimpak`a dönüşür, **açık metin silinir**, geçici arşiv
/// kalmaz ve paket gerçekten şifrelidir (işaretçi baytlarda yok, açılınca var).
#[test]
fn seal_dir_produces_dimpak_and_removes_plaintext() {
    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());

    let out = seal_dir(&dir, "paket-parolasi-1").expect("paket üretilmeli");

    assert!(!dir.exists(), "açık metin klasörü silinmeli");
    assert!(out.leftover_dir.is_none(), "kalıntı bildirilmemeli");
    assert!(out.dimpak.exists(), "şifreli paket yazılmalı");
    assert_eq!(
        out.dimpak.extension().and_then(|ext| ext.to_str()),
        Some("dimpak")
    );
    assert!(
        !dir.with_extension("zip").exists(),
        "geçici arşiv silinmeli"
    );

    let sealed = fs::read(&out.dimpak).expect("paket okunmalı");
    assert_eq!(&sealed[..8], PACKAGE_MAGIC);
    assert!(
        !contains(&sealed, b"ISARETCI-ACIK-METIN"),
        "açık metin paket baytlarında olmamalı"
    );
    let inner = open(&sealed, "paket-parolasi-1").expect("paket açılmalı");
    assert_eq!(&inner[..2], b"PK", "içerik zip olmalı");
    // Zip içeriği **Deflate** ile sıkıştırılır: metin baytları aranamaz. Dosya
    // **adları** sıkıştırılmaz — arşivin gerçekten klasörü taşıdığının kanıtı.
    assert!(
        contains(&inner, b"KAPSAM.md") && contains(&inner, b"case.json"),
        "dosya adları arşivde olmalı"
    );
}

/// Kısa parola: **hiçbir şey** yazılmaz — ne klasör silinir ne arşiv doğar.
#[test]
fn seal_dir_short_password_writes_nothing() {
    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());

    let error = seal_dir(&dir, "kisa").expect_err("kısa parola reddedilmeli");

    assert_eq!(error.code(), "E_PASSWORD_WEAK");
    assert!(dir.exists(), "klasör olduğu gibi kalmalı");
    assert!(dir.join("case.json").exists(), "içerik korunmalı");
    assert!(!dir.with_extension("zip").exists(), "arşiv yazılmamalı");
    assert!(!dir.with_extension("dimpak").exists(), "paket yazılmamalı");
}

/// Zip düşerse açık metin **bırakılmaz**: klasör, geçici arşiv ve yarım paket
/// silinir. (Eski kod bu yolda klasörü diskte bırakıyordu.)
#[cfg(unix)]
#[test]
fn seal_dir_removes_plaintext_when_zipping_fails() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());
    let locked = dir.join("kilitli.json");
    fs::write(&locked, b"gizli").expect("yaz");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("izin");

    let error = seal_dir(&dir, "paket-parolasi-1").expect_err("okunamayan dosya düşürmeli");

    assert_ne!(
        error.code(),
        "E_PACKAGE_PLAINTEXT",
        "klasör silinebildiği için kalıntı bildirilmemeli: {error}"
    );
    assert!(!dir.exists(), "açık metin klasörü silinmeli");
    assert!(
        !dir.with_extension("zip").exists(),
        "geçici arşiv silinmeli"
    );
    assert!(
        !dir.with_extension("dimpak").exists(),
        "yarım paket silinmeli"
    );

    // Geri alma: tempdir temizliği için izin gerekmez (dosya klasörle gitti).
    let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o600));
}

/// Açık metin silinemezse paket yine üretilir ama yol **bildirilir**:
/// "şifreli" sanılan bir açık metin klasörü sessizce bırakılmaz.
#[cfg(unix)]
#[test]
fn seal_dir_reports_leftover_when_plaintext_cannot_be_removed() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());
    // `r-x`: okunur (zip kurulur) ama içerik silinemez (yazma izni yok).
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).expect("izin");

    let out = seal_dir(&dir, "paket-parolasi-1").expect("paket üretilmeli");

    assert!(out.dimpak.exists(), "şifreli paket yazılmalı");
    assert_eq!(
        out.leftover_dir.as_deref(),
        Some(dir.as_path()),
        "kalan açık metin yolu bildirilmeli"
    );
    assert!(dir.exists(), "silinemeyen klasör yerinde kalmalı");

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("geri al");
}

/// Hem paketleme hem temizlik düşerse hata **yolu yazar** (`E_PACKAGE_PLAINTEXT`):
/// kalıntı gizlenmez, kullanıcı elle silebilir.
#[cfg(unix)]
#[test]
fn seal_dir_names_the_plaintext_path_when_cleanup_also_fails() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());
    let locked = dir.join("kilitli.json");
    fs::write(&locked, b"gizli").expect("yaz");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("dosya izni");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).expect("klasör izni");

    let error = seal_dir(&dir, "paket-parolasi-1").expect_err("düşmeli");

    assert_eq!(error.code(), "E_PACKAGE_PLAINTEXT");
    assert!(
        error.to_string().contains("DimDevir-202611051200"),
        "hata yolu yazmalı: {error}"
    );
    assert!(dir.exists(), "silinemeyen açık metin yerinde");

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("geri al");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o600)).expect("geri al");
}

/// Arşiv **geçici ada** yazılır: yarıda kalan arşiv geçerli sanılmasın, geride
/// `.tmp` kalmasın (F15-T06 davranışı `zip_dir`e taşındı).
#[test]
fn zip_dir_leaves_no_temp_sibling() {
    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());
    let archive = tmp.path().join("arsiv.zip");

    zip_dir(&dir, &archive).expect("arşiv");

    assert!(archive.exists(), "arşiv yazılmalı");
    let names: Vec<String> = fs::read_dir(tmp.path())
        .expect("liste")
        .map(|entry| {
            entry
                .expect("girdi")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert!(
        !names.iter().any(|name| name.ends_with(".tmp")),
        "geçici dosya kalmamalı: {names:?}"
    );
    let bytes = fs::read(&archive).expect("oku");
    assert_eq!(&bytes[..2], b"PK", "geçerli zip olmalı");
}

/// Hedef adda **klasör** varsa paket yazılmaz, kullanıcının klasörü silinmez
/// ve geçici dosya kalmaz (atomik yerleştirme ölçümü).
#[test]
fn seal_dir_does_not_replace_a_directory_and_leaves_no_temp() {
    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());
    let blocker = dir.with_extension("dimpak");
    fs::create_dir_all(&blocker).expect("klasör");
    fs::write(blocker.join("kullanici.txt"), b"kullanici verisi").expect("yaz");

    let error = seal_dir(&dir, "paket-parolasi-1").expect_err("klasörün üzerine yazılmamalı");

    assert_eq!(error.code(), "E_IO", "hata G/Ç olmalı: {error}");
    assert!(blocker.is_dir(), "kullanıcının klasörü korunmalı");
    assert!(
        blocker.join("kullanici.txt").exists(),
        "klasör içeriği silinmemeli"
    );
    assert!(!dir.exists(), "açık metin yine silinmeli");
    let names: Vec<String> = fs::read_dir(tmp.path())
        .expect("liste")
        .map(|entry| {
            entry
                .expect("girdi")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert!(
        !names.iter().any(|name| name.ends_with(".tmp")),
        "geçici dosya kalmamalı: {names:?}"
    );
}

/// Arşiv yarıda düşerse **hedef adda** dosya kalmaz: yarıda kalan bir `.zip`
/// geçerli arşiv sanılmasın (geçici ad + `rename`).
#[cfg(unix)]
#[test]
fn zip_dir_leaves_no_partial_archive_on_failure() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());
    let locked = dir.join("kilitli.json");
    fs::write(&locked, b"gizli").expect("yaz");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("izin");
    let archive = tmp.path().join("arsiv.zip");

    assert!(
        zip_dir(&dir, &archive).is_err(),
        "okunamayan dosya düşürmeli"
    );

    assert!(!archive.exists(), "hedef adda yarım arşiv kalmamalı");
    let names: Vec<String> = fs::read_dir(tmp.path())
        .expect("liste")
        .map(|entry| {
            entry
                .expect("girdi")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert!(
        !names.iter().any(|name| name.ends_with(".tmp")),
        "geçici dosya kalmamalı: {names:?}"
    );

    let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o600));
}

/// Başarısız bir arşivleme **önceki geçerli arşivi bozmaz**: yarıda kalan
/// yazma hazır dosyanın üzerine gitmez (geçici ad + `rename`).
#[cfg(unix)]
#[test]
fn zip_dir_failure_keeps_the_previous_archive() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().expect("dizin");
    let dir = package_dir(tmp.path());
    let archive = tmp.path().join("arsiv.zip");
    zip_dir(&dir, &archive).expect("ilk arşiv");
    let before = fs::read(&archive).expect("oku");

    let locked = dir.join("kilitli.json");
    fs::write(&locked, b"gizli").expect("yaz");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("izin");

    assert!(zip_dir(&dir, &archive).is_err(), "ikinci yazma düşmeli");

    assert_eq!(
        fs::read(&archive).expect("oku"),
        before,
        "önceki geçerli arşiv bozulmamalı"
    );
    let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o600));
}
