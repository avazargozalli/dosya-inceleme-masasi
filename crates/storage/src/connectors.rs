//! F12-T06 — **dış bağlayıcı sicili** (belgelenmiş, etkin değil).
//!
//! Şartname (bölüm 14.4) resmî web kaynaklarına bağlayıcıyı v1.0 kapsamına
//! almaz: her bağlayıcı için erişim yöntemi, kullanım koşulları, veri kapsamı,
//! hız sınırı ve yeniden kullanım değerlendirmesi **önce** belgelenir; kimlik
//! doğrulama, CAPTCHA veya erişim engeli aşılmaz ve belgelendirilmemiş uç
//! nokta ürün garantisine dönüştürülmez.
//!
//! Bu modül yalnız **sicili** taşır. Üründe ağ isteği gönderen hiçbir kod
//! yolu yoktur (`network_allowed` her satırda `false`); içe alma kullanıcının
//! kendi indirdiği dosyayla yapılır. Sicil kaydı, ileride bir bağlayıcı
//! açılmadan önce hangi koşulların yazılı olması gerektiğini görünür kılar.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::StorageError;

/// `status` alanının tek geçerli değeri: koşullar belgelendi, bağlayıcı
/// **etkin değil**. Yeni bir değer eklemek kapsam kararı gerektirir.
pub const STATUS_DOCUMENTED_NOT_ENABLED: &str = "documented_not_enabled";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorInfo {
    pub id: String,
    pub name: String,
    pub operator: String,
    /// Erişim yöntemi: ürünün değil, kullanıcının tarayıcısının yolu.
    pub access_method: String,
    pub terms_url: String,
    pub scope: String,
    pub rate_limit: String,
    pub reuse_note: String,
    pub status: String,
    /// Ürünün bu kaynağa istek göndermesi. Sicildeki her satırda `false`.
    pub network_allowed: bool,
    /// Neden kapalı olduğunun kısa, dürüst açıklaması.
    pub blocked_reason: String,
}

struct Entry {
    id: &'static str,
    name: &'static str,
    operator: &'static str,
    terms_url: &'static str,
    scope: &'static str,
    reuse_note: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry {
        id: "yargitay",
        name: "Yargıtay karar arama",
        operator: "T.C. Yargıtay Başkanlığı",
        terms_url: "https://www.yargitay.gov.tr/item/71/internet-sayfasindan-emsal-karar-arama-nasil-yapilir",
        scope: "İçerik ve künye alanlarıyla aranan kararların tam metni; kullanıcı arayüzü üzerinden erişim.",
        reuse_note: "Toplu veri indirme ve kalıcı yeniden yayımlama yetkisi doğrulanmadı; ürün kendi adına kazıma yapmaz.",
    },
    Entry {
        id: "danistay",
        name: "Danıştay karar arama",
        operator: "T.C. Danıştay Başkanlığı",
        terms_url: "https://karararama.danistay.gov.tr/",
        scope: "Karar arama arayüzü; sorgu ve sonuç kapsamı arayüzle sınırlı.",
        reuse_note: "Arayüzün belgelendirilmiş bir veri servisi yok; ürün belgelendirilmemiş uç nokta kullanmaz.",
    },
    Entry {
        id: "aym",
        name: "Anayasa Mahkemesi Kararlar Bilgi Bankası",
        operator: "T.C. Anayasa Mahkemesi",
        terms_url: "https://kararlarbilgibankasi.anayasa.gov.tr/",
        scope: "Bireysel başvuru ve norm denetimi kararları; başvuru numarasıyla arama.",
        reuse_note: "Kararların yeniden kullanım koşulları ayrıca değerlendirilmeden otomatik içe alma açılmaz.",
    },
];

/// Sicilin tamamı. Her satırda `network_allowed=false`; ağ isteği yolu yok.
#[must_use]
pub fn connector_catalog() -> Vec<ConnectorInfo> {
    ENTRIES
        .iter()
        .map(|entry| ConnectorInfo {
            id: entry.id.to_string(),
            name: entry.name.to_string(),
            operator: entry.operator.to_string(),
            access_method: "Resmî web arayüzü; erişim kullanıcının tarayıcısından, ürün istek göndermez."
                .to_string(),
            terms_url: entry.terms_url.to_string(),
            scope: entry.scope.to_string(),
            rate_limit: "Ürün istek göndermediği için hız sınırı uygulanmaz; kaynak koşulları değişirse yeniden değerlendirilir."
                .to_string(),
            reuse_note: entry.reuse_note.to_string(),
            status: STATUS_DOCUMENTED_NOT_ENABLED.to_string(),
            network_allowed: false,
            blocked_reason:
                "Bağlayıcı v1.0 kapsamında değil: kullanım koşulları ve yeniden kullanım hakkı yazılı olarak doğrulanmadan açılmaz (F18 kapsam kararı)."
                    .to_string(),
        })
        .collect()
}

/// Sicilin içerik özeti: kayıt sessizce değişirse fark edilir.
pub fn connector_catalog_hash() -> Result<String, StorageError> {
    let catalog = connector_catalog();
    let canonical = serde_json::to_string(&catalog)
        .map_err(|_| StorageError::Manifest("sicil serileştirilemedi".into()))?;
    Ok(hex::encode(Sha256::digest(canonical.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_entry_is_documented_and_disabled() {
        let catalog = connector_catalog();
        assert_eq!(catalog.len(), 3);
        for entry in &catalog {
            assert_eq!(entry.status, STATUS_DOCUMENTED_NOT_ENABLED);
            assert!(!entry.network_allowed, "{} ağ isteği açmamalı", entry.id);
            assert!(entry.terms_url.starts_with("https://"));
            assert!(!entry.scope.is_empty());
            assert!(!entry.rate_limit.is_empty());
            assert!(!entry.reuse_note.is_empty());
            assert!(!entry.blocked_reason.is_empty());
        }
        let ids: Vec<&str> = catalog.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["yargitay", "danistay", "aym"]);
    }

    #[test]
    fn catalog_hash_is_stable_and_content_bound() {
        let first = connector_catalog_hash().expect("hash");
        let second = connector_catalog_hash().expect("hash");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        // Özet içeriğe bağlıdır: sicil değişirse hash değişir (kayıt sessiz kalmaz).
        let mut probe = serde_json::to_string(&connector_catalog()).expect("json");
        probe.push(' ');
        assert_ne!(hex::encode(Sha256::digest(probe.as_bytes())), first);
    }
}
