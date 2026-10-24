use dim_vault::package::{open, seal, PACKAGE_MAGIC};

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
