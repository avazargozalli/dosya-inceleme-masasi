//! F12-T11 — kaynak kullanım koşulu ve korpus dağıtım kapısı.
//!
//! Kütüphanedeki her karar için **kullanım dayanağı** (`usage_basis`) ve
//! **yeniden dağıtım izni** (`redistribution`) ayrı kaydedilir. Varsayılan
//! `unknown`'dır: kayıt yokluğu izin sayılmaz.
//!
//! Toplu karar korpusu (tam metinler) yalnız `redistribution = allowed` olan
//! kayıtlarla ve yalnız yazılı dayanak varsa dışa aktarılır. `all` kipinde tek
//! bir izinsiz kayıt varsa **hiçbir metin** dosyaya yazılmaz; sessizce eksik
//! korpus üretilmez.
//!
//! v57: `allowed` iddiası **dışa aktarma anında yeniden denetlenir** — dayanak
//! türü dağıtılabilir olmalı, yazılı dayanak (bağlantı ya da not) bulunmalı ve
//! kayıt damgası taşınmalı. Şema kapısı (0057) yeni yazmayı buna zorlar; bu
//! denetim v57 öncesi ya da elle değiştirilmiş satırlar içindir: kanıtsız
//! `allowed` **izin sayılmaz**, metni dosyaya yazılmaz ve `redistribution_unknown`
//! («izin kaydı yok») olarak raporlanır.
//!
//! Bu katman ağ isteği yapmaz, dosya yazmaz; dosya yazımı ve denetim kaydı
//! Tauri komutundadır (`precedent_corpus_export`).

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{now_iso, Database, StorageError};

/// Kabul edilen kullanım dayanakları.
pub const USAGE_BASES: [&str; 5] = [
    "official_public",
    "licence",
    "permission",
    "internal_only",
    "unknown",
];

/// Yeniden dağıtım izni değerleri.
pub const REDISTRIBUTION_VALUES: [&str; 3] = ["allowed", "not_allowed", "unknown"];

/// Yeniden dağıtım için kabul edilen dayanaklar. `internal_only` ve `unknown`
/// burada yoktur: yalnız iç kullanım için alınmış karar dağıtılamaz.
const DISTRIBUTABLE_BASES: [&str; 3] = ["official_public", "licence", "permission"];

const MAX_NOTE_CHARS: usize = 4000;
const MAX_URL_CHARS: usize = 2000;
/// Liste yanıtlarında gösterilecek en çok engelli kayıt; toplam ayrı sayılır.
const MAX_BLOCKED_LIST: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentUsageTerms {
    pub precedent_id: String,
    pub usage_basis: String,
    pub redistribution: String,
    pub terms_url: Option<String>,
    pub terms_note: String,
    /// Koşul kaydının **alındığı** zaman. `None` = hiç kaydedilmedi.
    /// Hukuki inceleme tarihi değildir.
    pub terms_recorded_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetUsageTermsInput {
    pub usage_basis: String,
    pub redistribution: String,
    #[serde(default)]
    pub terms_url: Option<String>,
    #[serde(default)]
    pub terms_note: String,
}

/// Dışa aktarım kipi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusMode {
    /// Yalnız izinli kayıtlar; izinsizler sayılır ve dışarıda bırakılır.
    PermittedOnly,
    /// Tüm tam metinler; tek bir izinsiz kayıt varsa kapı kapanır.
    All,
}

impl CorpusMode {
    pub fn parse(value: &str) -> Result<Self, StorageError> {
        match value.trim() {
            "permitted_only" => Ok(Self::PermittedOnly),
            "all" => Ok(Self::All),
            other => Err(StorageError::Terms(format!(
                "bilinmeyen dağıtım kipi: {other}"
            ))),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PermittedOnly => "permitted_only",
            Self::All => "all",
        }
    }
}

/// Kütüphanenin dağıtım açısından dürüst özeti.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CorpusCoverage {
    pub total: usize,
    /// En yeni tam metni olan kayıt sayısı. Yalnız bunlar korpusa girebilir.
    pub with_text: usize,
    pub allowed: usize,
    pub not_allowed: usize,
    pub unknown: usize,
    pub blocked_total: usize,
    /// İzin kaydı olmadığı için korpusa giremeyen kayıtlar (en çok 100).
    pub blocked: Vec<CorpusExcluded>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CorpusExcluded {
    pub precedent_id: String,
    pub court: String,
    pub docket_no: Option<String>,
    /// `redistribution_not_allowed` | `redistribution_unknown`.
    /// Kanıtsız `allowed` iddiası (dayanak türü dağıtılabilir değil, yazılı
    /// dayanak ya da damga yok) **izin sayılmaz** ve `redistribution_unknown`
    /// olarak raporlanır — sütun «izinli» dese de geçerli izin kaydı yoktur.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CorpusItem {
    pub precedent_id: String,
    pub court: String,
    pub chamber: Option<String>,
    pub docket_no: Option<String>,
    pub decision_no: Option<String>,
    pub decision_date: Option<String>,
    pub title: Option<String>,
    pub source_url: Option<String>,
    pub usage_basis: String,
    pub redistribution: String,
    pub terms_url: Option<String>,
    pub acquired_at: String,
    pub char_count: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusExport {
    pub mode: String,
    pub exported: usize,
    pub excluded: usize,
    pub excluded_items: Vec<CorpusExcluded>,
    pub items: Vec<CorpusItem>,
    /// `items` dizisinin kanonik JSON'u üzerinden SHA-256. Dosya yazılmadan
    /// önce içeriğin ne olduğunu sabitler.
    pub content_hash: String,
    pub coverage: CorpusCoverage,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct CorpusRecord {
    id: String,
    court: String,
    chamber: Option<String>,
    docket_no: Option<String>,
    decision_no: Option<String>,
    decision_date: Option<String>,
    title: Option<String>,
    source_url: Option<String>,
    usage_basis: String,
    redistribution: String,
    terms_url: Option<String>,
    terms_note: String,
    terms_recorded_at: Option<String>,
    text: Option<String>,
    acquired_at: Option<String>,
    char_count: i64,
}

impl CorpusRecord {
    /// `allowed` iddiası **desteklenmiş** mi: dayanak türü dağıtılabilir
    /// (yalnız iç kullanım/bilinmeyen dağıtılamaz), yazılı dayanak var
    /// (bağlantı ya da not) ve kayıt damgası taşınıyor. 0057 şema kapısı yeni
    /// yazmayı buna zorlar; burada aynı kural **okuma anında** uygulanır çünkü
    /// korpus metni dosyaya yazar — sütuna güvenmek yetmez.
    fn permission_backed(&self) -> bool {
        DISTRIBUTABLE_BASES.contains(&self.usage_basis.as_str())
            && (self
                .terms_url
                .as_deref()
                .is_some_and(|url| !url.trim().is_empty())
                || !self.terms_note.trim().is_empty())
            && self
                .terms_recorded_at
                .as_deref()
                .is_some_and(|at| !at.trim().is_empty())
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        match self.redistribution.as_str() {
            "allowed" if self.permission_backed() => None,
            "not_allowed" => Some("redistribution_not_allowed"),
            _ => Some("redistribution_unknown"),
        }
    }

    fn excluded(&self) -> CorpusExcluded {
        CorpusExcluded {
            precedent_id: self.id.clone(),
            court: self.court.clone(),
            docket_no: self.docket_no.clone(),
            reason: self
                .exclusion_reason()
                .unwrap_or("redistribution_unknown")
                .to_string(),
        }
    }
}

impl Database {
    /// Kullanım koşulunu kaydeder. `allowed` için **yazılı dayanak** şarttır:
    /// dayanak türü resmî yayın/lisans/izin olmalı ve not ya da bağlantı
    /// verilmelidir. Aksi hâlde yazılmaz (`E_TERMS`).
    pub fn set_precedent_usage_terms(
        &self,
        precedent_id: &str,
        input: &SetUsageTermsInput,
    ) -> Result<PrecedentUsageTerms, StorageError> {
        if self.get_precedent(precedent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let basis = input.usage_basis.trim().to_ascii_lowercase();
        if !USAGE_BASES.contains(&basis.as_str()) {
            return Err(StorageError::Terms(format!(
                "bilinmeyen kullanım dayanağı: {}",
                input.usage_basis.trim()
            )));
        }
        let redistribution = input.redistribution.trim().to_ascii_lowercase();
        if !REDISTRIBUTION_VALUES.contains(&redistribution.as_str()) {
            return Err(StorageError::Terms(format!(
                "bilinmeyen dağıtım izni: {}",
                input.redistribution.trim()
            )));
        }
        let note = input.terms_note.trim().to_string();
        if note.chars().count() > MAX_NOTE_CHARS {
            return Err(StorageError::Terms(format!(
                "koşul notu en çok {MAX_NOTE_CHARS} karakter olabilir"
            )));
        }
        let url = input
            .terms_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(value) = &url {
            if value.chars().count() > MAX_URL_CHARS {
                return Err(StorageError::Terms(format!(
                    "koşul bağlantısı en çok {MAX_URL_CHARS} karakter olabilir"
                )));
            }
            if value.chars().any(char::is_control) {
                return Err(StorageError::Terms(
                    "koşul bağlantısı kontrol karakteri içeremez".into(),
                ));
            }
            if !(value.starts_with("http://") || value.starts_with("https://")) {
                return Err(StorageError::Terms(
                    "koşul bağlantısı http:// veya https:// ile başlamalı".into(),
                ));
            }
        }
        if redistribution == "allowed" {
            if !DISTRIBUTABLE_BASES.contains(&basis.as_str()) {
                return Err(StorageError::Terms(
                    "yeniden dağıtım 'izinli' işaretlenemez: dayanak resmî yayın, lisans veya yazılı izin olmalı (yalnız iç kullanım dağıtılamaz)"
                        .into(),
                ));
            }
            if url.is_none() && note.is_empty() {
                return Err(StorageError::Terms(
                    "yeniden dağıtım 'izinli' için yazılı dayanak gerekir: kaynak bağlantısı veya not ekleyin"
                        .into(),
                ));
            }
        }

        let recorded_at = now_iso();
        self.conn().execute(
            "UPDATE precedents SET usage_basis=?2,redistribution=?3,terms_url=?4,terms_note=?5,terms_recorded_at=?6
             WHERE id=?1",
            rusqlite::params![precedent_id, basis, redistribution, url, note, recorded_at],
        )?;
        // Denetime not metni ve bağlantı yazılmaz; yalnız dayanak türü ve
        // yazılı dayanağın var olup olmadığı.
        self.append_audit(
            "precedent",
            Some(precedent_id),
            "usage_terms",
            "local-user",
            &serde_json::json!({
                "usageBasis": basis,
                "redistribution": redistribution,
                "hasUrl": url.is_some(),
                "hasNote": !note.is_empty(),
            }),
        )?;
        self.precedent_usage_terms(precedent_id)
    }

    pub fn precedent_usage_terms(
        &self,
        precedent_id: &str,
    ) -> Result<PrecedentUsageTerms, StorageError> {
        self.conn()
            .query_row(
                "SELECT id,usage_basis,redistribution,terms_url,terms_note,terms_recorded_at
                 FROM precedents WHERE id=?1",
                [precedent_id],
                |row| {
                    Ok(PrecedentUsageTerms {
                        precedent_id: row.get(0)?,
                        usage_basis: row.get(1)?,
                        redistribution: row.get(2)?,
                        terms_url: row.get(3)?,
                        terms_note: row.get(4)?,
                        terms_recorded_at: row.get(5)?,
                    })
                },
            )
            .optional()?
            .ok_or(StorageError::NotFound)
    }

    /// Dağıtım kapısının gördüğü tablo: kaç kayıt var, kaçında metin var,
    /// kaçı izinli. Metinsiz kayıt dağıtılacak metin taşımadığı için kapıyı
    /// kapatmaz ama sayıdan gizlenmez.
    pub fn precedent_corpus_coverage(&self) -> Result<CorpusCoverage, StorageError> {
        let records = self.load_corpus_records()?;
        Ok(coverage_of(&records))
    }

    /// Korpusu kurar. `All` kipinde tek bir izinsiz/izin kaydı olmayan metin
    /// varsa hata döner ve **hiçbir metin** seçilmez; `PermittedOnly` kipinde
    /// yalnız izinliler alınır, dışarıda kalanlar sayılır ve gerekçesi yazılır.
    pub fn build_precedent_corpus(&self, mode: CorpusMode) -> Result<CorpusExport, StorageError> {
        let records = self.load_corpus_records()?;
        let coverage = coverage_of(&records);
        if mode == CorpusMode::All && coverage.allowed != coverage.with_text {
            return Err(StorageError::CorpusNotPermitted(format!(
                "{} tam metinli kayıtta yeniden dağıtım izni yok ({} izinsiz, {} bilinmiyor); izin kaydı olmadan toplu karar korpusu dağıtılmaz",
                coverage.blocked_total, coverage.not_allowed, coverage.unknown
            )));
        }

        let mut items = Vec::new();
        let mut excluded_items = Vec::new();
        for record in &records {
            let Some(text) = &record.text else { continue };
            if record.exclusion_reason().is_some() {
                excluded_items.push(record.excluded());
                continue;
            }
            items.push(CorpusItem {
                precedent_id: record.id.clone(),
                court: record.court.clone(),
                chamber: record.chamber.clone(),
                docket_no: record.docket_no.clone(),
                decision_no: record.decision_no.clone(),
                decision_date: record.decision_date.clone(),
                title: record.title.clone(),
                source_url: record.source_url.clone(),
                usage_basis: record.usage_basis.clone(),
                redistribution: record.redistribution.clone(),
                terms_url: record.terms_url.clone(),
                acquired_at: record.acquired_at.clone().unwrap_or_default(),
                char_count: record.char_count,
                text: text.clone(),
            });
        }

        let canonical = serde_json::to_vec(&items)
            .map_err(|_| StorageError::Integrity("korpus içeriği serileştirilemedi".into()))?;
        let content_hash = hex::encode(Sha256::digest(&canonical));

        let mut notes = Vec::new();
        if coverage.total > coverage.with_text {
            notes.push(format!(
                "{} kaydın tam metni yok; korpusa alınmadı (künye dağıtımı ayrı iştir).",
                coverage.total - coverage.with_text
            ));
        }
        if !excluded_items.is_empty() {
            notes.push(format!(
                "{} kayıt yeniden dağıtım izni olmadığı için dışa aktarılmadı; metinleri dosyaya yazılmadı.",
                excluded_items.len()
            ));
        }
        if items.is_empty() {
            notes.push(
                "Dışa aktarılacak izinli tam metin yok; boş korpus dosyası yazılmaz.".to_string(),
            );
        }
        notes.push(
            "Korpus yalnız tam metinleri taşır; künye alanları kimlik içindir. İzin kaydı olmayan metin dosyaya girmez."
                .to_string(),
        );

        Ok(CorpusExport {
            mode: mode.as_str().to_string(),
            exported: items.len(),
            excluded: excluded_items.len(),
            excluded_items,
            items,
            content_hash,
            coverage,
            notes,
        })
    }

    fn load_corpus_records(&self) -> Result<Vec<CorpusRecord>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT p.id,p.court,p.chamber,p.docket_no,p.decision_no,p.decision_date,p.title,p.source_url,
                    p.usage_basis,p.redistribution,p.terms_url,p.terms_note,p.terms_recorded_at,
                    v.full_text,v.acquired_at,coalesce(v.text_char_count,0)
             FROM precedents p
             LEFT JOIN precedent_versions v ON v.id = (
                 SELECT v2.id FROM precedent_versions v2
                 WHERE v2.precedent_id = p.id
                   AND v2.full_text IS NOT NULL
                   AND length(trim(v2.full_text)) > 0
                 ORDER BY v2.acquired_at DESC, v2.id DESC
                 LIMIT 1
             )
             ORDER BY p.id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CorpusRecord {
                id: row.get(0)?,
                court: row.get(1)?,
                chamber: row.get(2)?,
                docket_no: row.get(3)?,
                decision_no: row.get(4)?,
                decision_date: row.get(5)?,
                title: row.get(6)?,
                source_url: row.get(7)?,
                usage_basis: row.get(8)?,
                redistribution: row.get(9)?,
                terms_url: row.get(10)?,
                terms_note: row.get(11)?,
                terms_recorded_at: row.get(12)?,
                text: row.get(13)?,
                acquired_at: row.get(14)?,
                char_count: row.get(15)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn coverage_of(records: &[CorpusRecord]) -> CorpusCoverage {
    let mut coverage = CorpusCoverage {
        total: records.len(),
        with_text: 0,
        allowed: 0,
        not_allowed: 0,
        unknown: 0,
        blocked_total: 0,
        blocked: Vec::new(),
    };
    for record in records {
        if record.text.is_none() {
            continue;
        }
        coverage.with_text += 1;
        match record.exclusion_reason() {
            None => coverage.allowed += 1,
            Some("redistribution_not_allowed") => {
                coverage.not_allowed += 1;
                coverage.blocked_total += 1;
                if coverage.blocked.len() < MAX_BLOCKED_LIST {
                    coverage.blocked.push(record.excluded());
                }
            }
            Some(_) => {
                coverage.unknown += 1;
                coverage.blocked_total += 1;
                if coverage.blocked.len() < MAX_BLOCKED_LIST {
                    coverage.blocked.push(record.excluded());
                }
            }
        }
    }
    coverage
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precedents::{NewPrecedent, NewPrecedentText};
    use crate::DatabaseKey;

    const SECRET: &str = "ÖZEL-KORPUS-BELİRTEÇ-7741 kararın gizli gövdesi.";

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([31u8; 32])).expect("db")
    }

    fn precedent(db: &Database, court: &str, docket: &str) -> String {
        db.create_precedent(&NewPrecedent {
            court: court.into(),
            chamber: Some("1. Hukuk Dairesi".into()),
            docket_no: Some(docket.into()),
            decision_no: Some("2025/456".into()),
            application_no: None,
            decision_date: Some("2025-05-02".into()),
            title: Some("Kira alacağı".into()),
            source_url: Some("https://example.invalid/karar".into()),
            user_tags: vec![],
            legal_issue: None,
        })
        .expect("emsal")
        .id
    }

    fn with_text(db: &Database, id: &str, text: &str) {
        db.add_precedent_text(
            id,
            &NewPrecedentText {
                source_kind: "paste".into(),
                text: text.into(),
                document_version_id: None,
            },
        )
        .expect("metin");
    }

    fn allowed(basis: &str, url: Option<&str>, note: &str) -> SetUsageTermsInput {
        SetUsageTermsInput {
            usage_basis: basis.into(),
            redistribution: "allowed".into(),
            terms_url: url.map(str::to_string),
            terms_note: note.into(),
        }
    }

    #[test]
    fn new_precedent_starts_unknown_and_unrecorded() {
        let db = db();
        let id = precedent(&db, "Yargıtay", "2024/123");
        let terms = db.precedent_usage_terms(&id).expect("koşul");
        assert_eq!(terms.usage_basis, "unknown");
        assert_eq!(terms.redistribution, "unknown");
        assert!(terms.terms_recorded_at.is_none());
        assert!(terms.terms_note.is_empty());

        let row = db.get_precedent(&id).expect("satır").expect("var");
        assert_eq!(row.redistribution, "unknown");
        let coverage = db.precedent_corpus_coverage().expect("kapsam");
        assert_eq!(coverage.total, 1);
        assert_eq!(coverage.with_text, 0);
    }

    #[test]
    fn allowed_requires_written_basis() {
        let db = db();
        let id = precedent(&db, "Yargıtay", "2024/123");

        let no_basis = db
            .set_precedent_usage_terms(
                &id,
                &SetUsageTermsInput {
                    usage_basis: "unknown".into(),
                    redistribution: "allowed".into(),
                    terms_url: Some("https://example.invalid/karar".into()),
                    terms_note: String::new(),
                },
            )
            .expect_err("dayanaksız izin");
        assert_eq!(no_basis.code(), "E_TERMS");

        let internal = db
            .set_precedent_usage_terms(&id, &allowed("internal_only", None, "büro içi"))
            .expect_err("iç kullanım dağıtılamaz");
        assert_eq!(internal.code(), "E_TERMS");

        let no_evidence = db
            .set_precedent_usage_terms(&id, &allowed("official_public", None, ""))
            .expect_err("yazılı dayanak yok");
        assert_eq!(no_evidence.code(), "E_TERMS");

        // Hiçbiri yazılmadı: kayıt hâlâ unknown.
        assert_eq!(
            db.precedent_usage_terms(&id).expect("koşul").redistribution,
            "unknown"
        );

        let ok = db
            .set_precedent_usage_terms(
                &id,
                &allowed("official_public", None, "Resmî Gazete 12.05.2025"),
            )
            .expect("izinli");
        assert_eq!(ok.redistribution, "allowed");
        assert_eq!(ok.usage_basis, "official_public");
        assert!(ok.terms_recorded_at.is_some());
        assert_eq!(ok.terms_note, "Resmî Gazete 12.05.2025");
    }

    #[test]
    fn invalid_values_and_urls_are_rejected_without_write() {
        let db = db();
        let id = precedent(&db, "Yargıtay", "2024/123");
        for (basis, redistribution, url) in [
            ("okul", "unknown", None),
            ("official_public", "belki", None),
            (
                "official_public",
                "not_allowed",
                Some("example.invalid/karar"),
            ),
            (
                "official_public",
                "not_allowed",
                Some("ftp://example.invalid"),
            ),
        ] {
            let error = db
                .set_precedent_usage_terms(
                    &id,
                    &SetUsageTermsInput {
                        usage_basis: basis.into(),
                        redistribution: redistribution.into(),
                        terms_url: url.map(str::to_string),
                        terms_note: String::new(),
                    },
                )
                .expect_err("geçersiz");
            assert_eq!(error.code(), "E_TERMS");
        }
        let terms = db.precedent_usage_terms(&id).expect("koşul");
        assert_eq!(terms.usage_basis, "unknown");
        assert!(terms.terms_recorded_at.is_none());
    }

    #[test]
    fn corpus_gate_blocks_all_mode_when_any_text_has_no_permission() {
        let db = db();
        let permitted = precedent(&db, "Yargıtay", "2024/123");
        let unknown = precedent(&db, "Danıştay", "2024/999");
        with_text(&db, &permitted, SECRET);
        with_text(&db, &unknown, "İkinci karar metni.");
        db.set_precedent_usage_terms(&permitted, &allowed("official_public", None, "Resmî yayın"))
            .expect("izin");

        let coverage = db.precedent_corpus_coverage().expect("kapsam");
        assert_eq!(coverage.total, 2);
        assert_eq!(coverage.with_text, 2);
        assert_eq!(coverage.allowed, 1);
        assert_eq!(coverage.unknown, 1);
        assert_eq!(coverage.blocked_total, 1);

        let error = db
            .build_precedent_corpus(CorpusMode::All)
            .expect_err("kapı kapalı");
        assert_eq!(error.code(), "E_CORPUS_NOT_PERMITTED");
        let message = error.to_string();
        assert!(message.contains("1 tam metinli kayıtta"));
        assert!(message.contains("bilinmiyor"));
    }

    #[test]
    fn permitted_only_excludes_and_never_leaks_withheld_text() {
        let db = db();
        let permitted = precedent(&db, "Yargıtay", "2024/123");
        let withheld = precedent(&db, "Yargıtay", "2024/124");
        with_text(&db, &permitted, "İzinli karar metni.");
        with_text(&db, &withheld, SECRET);
        db.set_precedent_usage_terms(&permitted, &allowed("official_public", None, "Resmî yayın"))
            .expect("izin");

        let export = db
            .build_precedent_corpus(CorpusMode::PermittedOnly)
            .expect("korpus");
        assert_eq!(export.exported, 1);
        assert_eq!(export.excluded, 1);
        assert_eq!(
            export.excluded_items.first().map(|e| e.reason.as_str()),
            Some("redistribution_unknown")
        );
        assert_eq!(
            export.items.first().map(|i| i.precedent_id.as_str()),
            Some(permitted.as_str())
        );
        let items_json = serde_json::to_string(&export.items).expect("json");
        assert!(!items_json.contains("ÖZEL-KORPUS-BELİRTEÇ-7741"));
        assert!(export
            .notes
            .iter()
            .any(|n| n.contains("1 kayıt yeniden dağıtım izni")));
        assert_eq!(
            export.content_hash,
            hex::encode(Sha256::digest(
                serde_json::to_vec(&export.items).expect("json")
            ))
        );
    }

    #[test]
    fn all_mode_exports_when_every_text_is_permitted_and_hash_is_stable() {
        let db = db();
        let first = precedent(&db, "Yargıtay", "2024/123");
        let second = precedent(&db, "Yargıtay", "2024/124");
        with_text(&db, &first, "Birinci karar metni.");
        with_text(&db, &second, "İkinci karar metni.");
        for id in [&first, &second] {
            db.set_precedent_usage_terms(
                id,
                &allowed(
                    "licence",
                    Some("https://creativecommons.org/licenses/by/4.0/"),
                    "CC-BY-4.0",
                ),
            )
            .expect("izin");
        }

        let export = db.build_precedent_corpus(CorpusMode::All).expect("korpus");
        assert_eq!(export.exported, 2);
        assert_eq!(export.excluded, 0);
        let again = db.build_precedent_corpus(CorpusMode::All).expect("korpus");
        assert_eq!(export.content_hash, again.content_hash);
        // Sıra kimliğe göre sabit: aynı veri aynı hash'i verir.
        let ids: Vec<&str> = export
            .items
            .iter()
            .map(|i| i.precedent_id.as_str())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn records_without_text_do_not_block_but_are_reported() {
        let db = db();
        let with = precedent(&db, "Yargıtay", "2024/123");
        let without = precedent(&db, "Yargıtay", "2024/124");
        with_text(&db, &with, "Metinli karar.");
        db.set_precedent_usage_terms(
            &with,
            &allowed("permission", None, "Yazılı izin 03.06.2026"),
        )
        .expect("izin");

        let export = db.build_precedent_corpus(CorpusMode::All).expect("korpus");
        assert_eq!(export.exported, 1);
        assert_eq!(export.coverage.total, 2);
        assert_eq!(export.coverage.with_text, 1);
        assert!(export
            .notes
            .iter()
            .any(|n| n.contains("1 kaydın tam metni yok")));
        assert!(!export.items.iter().any(|i| i.precedent_id == without));
    }

    #[test]
    fn not_allowed_is_reported_separately_from_unknown() {
        let db = db();
        let denied = precedent(&db, "Yargıtay", "2024/123");
        let unknown = precedent(&db, "Yargıtay", "2024/124");
        with_text(&db, &denied, "İzinsiz karar metni.");
        with_text(&db, &unknown, "Bilinmeyen karar metni.");
        db.set_precedent_usage_terms(
            &denied,
            &SetUsageTermsInput {
                usage_basis: "internal_only".into(),
                redistribution: "not_allowed".into(),
                terms_url: None,
                terms_note: "Lisans yalnız büro içi kullanım".into(),
            },
        )
        .expect("izinsiz kaydı");

        let coverage = db.precedent_corpus_coverage().expect("kapsam");
        assert_eq!(coverage.not_allowed, 1);
        assert_eq!(coverage.unknown, 1);
        let reasons: Vec<&str> = coverage.blocked.iter().map(|b| b.reason.as_str()).collect();
        assert!(reasons.contains(&"redistribution_not_allowed"));
        assert!(reasons.contains(&"redistribution_unknown"));

        let export = db
            .build_precedent_corpus(CorpusMode::PermittedOnly)
            .expect("korpus");
        assert_eq!(export.exported, 0);
        assert_eq!(export.excluded, 2);
        assert!(export
            .notes
            .iter()
            .any(|n| n.contains("boş korpus dosyası yazılmaz")));
    }

    #[test]
    fn corpus_mode_refuses_unknown_mode() {
        assert_eq!(CorpusMode::parse("all").expect("kip").as_str(), "all");
        assert_eq!(
            CorpusMode::parse(" permitted_only ").expect("kip").as_str(),
            "permitted_only"
        );
        assert_eq!(
            CorpusMode::parse("hepsi").expect_err("bilinmeyen").code(),
            "E_TERMS"
        );
    }

    /// v57: **kullanım koşulu iddiası** doğuşta ve her yazımda tutarlı olmalı.
    ///
    /// Ölçülen kusur (gerçek SQLite v56, `measure_before.log`): 37 kusur
    /// yazımının **37'si** kabul ediliyordu — `allowed` dayanaksız doğabiliyordu
    /// (dağıtım kapısı yalnız `redistribution` sütununa baktığı için metin
    /// **dosyaya** yazılıyordu), url `ftp://`/şemasız/kırpılmamış olabiliyor,
    /// not ve damga biçimsiz doğabiliyordu. `unknown`/`not_allowed` serbest
    /// kalır: izin **vermeyen** iddia kanıt istemez.
    #[test]
    fn usage_terms_claim_is_backed_and_well_formed() {
        let db = db();
        const AT: &str = "2026-01-01T00:00:00Z";
        const SQL: &str = "INSERT INTO precedents
               (id,court,completeness,created_at,usage_basis,redistribution,
                terms_url,terms_note,terms_recorded_at)
             VALUES (?1,'Yargıtay','incomplete',?2,?3,?4,?5,?6,?7)";
        type Terms = (
            String,
            String,
            String,
            Option<String>,
            String,
            Option<String>,
        );
        let base = |id: &str| -> Terms {
            (
                id.into(),
                "unknown".into(),
                "unknown".into(),
                None,
                String::new(),
                None,
            )
        };
        let allowed_row = |id: &str| -> Terms {
            (
                id.into(),
                "official_public".into(),
                "allowed".into(),
                Some("https://example.invalid/terms".into()),
                String::new(),
                Some(AT.into()),
            )
        };
        let note_row = |id: &str| -> Terms {
            (
                id.into(),
                "official_public".into(),
                "allowed".into(),
                None,
                "kurum izni".into(),
                Some(AT.into()),
            )
        };
        let write = |row: &Terms| {
            db.conn()
                .execute(
                    SQL,
                    rusqlite::params![&row.0, AT, &row.1, &row.2, &row.3, &row.4, &row.5],
                )
                .map_err(StorageError::from)
        };
        let rejects = |result: Result<usize, StorageError>, name: &str| match result {
            Err(StorageError::Integrity(message)) => {
                assert!(message.contains("precedents"), "{name}: {message}")
            }
            other => panic!("v57 kusuru reddetmeli ({name}): {other:?}"),
        };

        // --- doğum kusurları: her biri **tek** cümleyi taşırır -----------------
        let mut attacks: Vec<(&str, Terms)> = Vec::new();
        let mut row = base("t-basis-unknown");
        row.1 = "unknown".into();
        row.2 = "allowed".into();
        row.3 = Some("https://example.invalid/terms".into());
        row.5 = Some(AT.into());
        attacks.push(("allowed + dayanak 'unknown'", row));
        let mut row = base("t-basis-internal");
        row.1 = "internal_only".into();
        row.2 = "allowed".into();
        row.3 = Some("https://example.invalid/terms".into());
        row.5 = Some(AT.into());
        attacks.push(("allowed + dayanak 'internal_only'", row));
        let mut row = base("t-no-evidence");
        row.1 = "official_public".into();
        row.2 = "allowed".into();
        row.5 = Some(AT.into());
        attacks.push(("allowed + yazılı dayanak yok", row));
        let mut row = base("t-note-space");
        row.1 = "official_public".into();
        row.2 = "allowed".into();
        row.4 = "   ".into();
        row.5 = Some(AT.into());
        attacks.push(("allowed + not yalnız boşluk", row));
        let mut row = base("t-no-stamp");
        row.1 = "licence".into();
        row.2 = "allowed".into();
        row.3 = Some("https://example.invalid/terms".into());
        attacks.push(("allowed + damga yok", row));
        for (index, stamp) in ["dün", "", "01.06.2026"].into_iter().enumerate() {
            let mut row = base(&format!("t-stamp-{index}"));
            row.5 = Some(stamp.into());
            attacks.push(("damga ISO değil", row));
        }
        for (index, url) in [
            "ftp://example.invalid/terms",
            "example.invalid/terms",
            "https://example.invalid/terms ",
            "",
            "   ",
            "https://example.invalid/a\nb",
            "https://example.invalid/a\rb",
            "https://example.invalid/a\tb",
        ]
        .into_iter()
        .enumerate()
        {
            let mut row = base(&format!("t-url-{index}"));
            row.3 = Some(url.into());
            attacks.push(("url biçimi", row));
        }
        let mut row = base("t-url-long");
        row.3 = Some(format!("https://example.invalid/{}", "n".repeat(1983)));
        attacks.push(("url 2001 karakter", row));
        let mut row = base("t-note-pad");
        row.4 = " kurum izni ".into();
        attacks.push(("not kırpılmamış", row));
        let mut row = base("t-note-long");
        row.4 = "n".repeat(4001);
        attacks.push(("not 4001 karakter", row));
        for (name, row) in &attacks {
            rejects(write(row), name);
        }

        // --- meşru doğumlar ----------------------------------------------------
        let mut legit: Vec<(&str, Terms)> = Vec::new();
        legit.push(("kayıt yok", base("t-l-defaults")));
        let mut row = base("t-l-url");
        row.1 = "official_public".into();
        row.2 = "allowed".into();
        row.3 = Some("https://example.invalid/terms".into());
        row.5 = Some(AT.into());
        legit.push(("allowed + resmî yayın + bağlantı", row));
        legit.push(("allowed + lisans + not", note_row("t-l-note")));
        let mut row = base("t-l-http");
        row.1 = "permission".into();
        row.2 = "allowed".into();
        row.3 = Some("http://example.invalid/terms".into());
        row.4 = "yazılı izin".into();
        row.5 = Some("2026-01-02T03:04:05.123456Z".into());
        legit.push(("allowed + düz http + mikrosaniyeli damga", row));
        let mut row = base("t-l-denied");
        row.1 = "internal_only".into();
        row.2 = "not_allowed".into();
        row.4 = "yalnız büro içi".into();
        legit.push(("not_allowed + internal_only", row));
        let mut row = base("t-l-pending");
        row.1 = "official_public".into();
        row.4 = "resmî yayın".into();
        legit.push(("dayanak var, karar yok (unknown)", row));
        for (name, row) in &legit {
            write(row)
                .unwrap_or_else(|error| panic!("v57 meşru yazmayı reddetti ({name}): {error}"));
        }

        // --- UPDATE: iddia değişiyorsa yeni satır yine tutarlı olmalı ----------
        let updates: Vec<(&str, Terms, &str)> = vec![
            (
                "dayanak 'unknown'a çevrildi",
                allowed_row("t-u-basis"),
                "UPDATE precedents SET usage_basis='unknown' WHERE id='t-u-basis'",
            ),
            (
                "dayanak 'internal_only'a çevrildi",
                allowed_row("t-u-internal"),
                "UPDATE precedents SET usage_basis='internal_only' WHERE id='t-u-internal'",
            ),
            (
                "url ve not birlikte silindi",
                allowed_row("t-u-evidence"),
                "UPDATE precedents SET terms_url=NULL, terms_note='' WHERE id='t-u-evidence'",
            ),
            (
                "damga NULL'a çekildi",
                allowed_row("t-u-stamp"),
                "UPDATE precedents SET terms_recorded_at=NULL WHERE id='t-u-stamp'",
            ),
            (
                "damga 'dün' yazıldı",
                allowed_row("t-u-stamp2"),
                "UPDATE precedents SET terms_recorded_at='dün' WHERE id='t-u-stamp2'",
            ),
            (
                "url 'ftp://' yapıldı",
                allowed_row("t-u-ftp"),
                "UPDATE precedents SET terms_url='ftp://example.invalid/terms' WHERE id='t-u-ftp'",
            ),
            (
                "not yalnız boşluğa çevrildi",
                note_row("t-u-blank"),
                "UPDATE precedents SET terms_note='   ' WHERE id='t-u-blank'",
            ),
            (
                "not kırpılmamış yazıldı",
                note_row("t-u-pad"),
                "UPDATE precedents SET terms_note=' kurum izni ' WHERE id='t-u-pad'",
            ),
            (
                "not 4001 karaktere çıkarıldı",
                note_row("t-u-long"),
                "UPDATE precedents SET terms_note=printf('%.4001c','n') WHERE id='t-u-long'",
            ),
            (
                "url kırpılmamış yazıldı",
                allowed_row("t-u-urlpad"),
                "UPDATE precedents SET terms_url='https://example.invalid/terms ' WHERE id='t-u-urlpad'",
            ),
            (
                "url içine satır sonu eklendi",
                allowed_row("t-u-newline"),
                "UPDATE precedents SET terms_url='https://example.invalid/a' || char(10) || 'b' WHERE id='t-u-newline'",
            ),
            (
                "izin, yazılı dayanak yokken sonradan açıldı",
                (
                "t-u-claim".into(),
                "official_public".into(),
                "unknown".into(),
                None,
                String::new(),
                None,
                ),
                "UPDATE precedents SET redistribution='allowed' WHERE id='t-u-claim'",
            ),
            (
                "url içine satır başı eklendi",
                allowed_row("t-u-cr"),
                "UPDATE precedents SET terms_url='https://example.invalid/a' || char(13) || 'b' WHERE id='t-u-cr'",
            ),
            (
                "url içine sekme eklendi",
                allowed_row("t-u-tab"),
                "UPDATE precedents SET terms_url='https://example.invalid/a' || char(9) || 'b' WHERE id='t-u-tab'",
            ),
            (
                "url 2001 karaktere çıkarıldı",
                allowed_row("t-u-urllong"),
                "UPDATE precedents SET terms_url='https://example.invalid/' || printf('%.1983c','n') WHERE id='t-u-urllong'",
            ),
        ];

        for (name, seed, sql) in &updates {
            write(seed).unwrap_or_else(|error| panic!("tohum yazılamadı ({name}): {error}"));
            rejects(db.conn().execute(sql, []).map_err(StorageError::from), name);
        }

        // --- meşru UPDATE'ler --------------------------------------------------
        let mut row = allowed_row("t-o-note");
        row.4 = String::new();
        write(&row).expect("tohum");
        db.conn()
            .execute(
                "UPDATE precedents SET terms_note='kurum izni 03.06.2026' WHERE id='t-o-note'",
                [],
            )
            .expect("not güncellenebilmeli");
        db.conn()
            .execute(
                "UPDATE precedents SET usage_basis='unknown', redistribution='unknown',
                        terms_url=NULL, terms_note='', terms_recorded_at=NULL
                  WHERE id='t-o-note'",
                [],
            )
            .expect("koşul sıfırlanabilmeli");
        let mut row = note_row("t-o-deny");
        row.4 = String::new();
        row.3 = Some("https://example.invalid/terms".into());
        write(&row).expect("tohum");
        db.conn()
            .execute(
                "UPDATE precedents SET redistribution='not_allowed', usage_basis='internal_only',
                        terms_url=NULL, terms_note='yalnız büro içi'
                  WHERE id='t-o-deny'",
                [],
            )
            .expect("izin geri alınabilmeli");

        // Çekirdek meşru yol kapıdan geçer.
        let core_id = precedent(&db, "Danıştay", "2024/777");
        let terms = db
            .set_precedent_usage_terms(
                &core_id,
                &allowed("official_public", Some("https://example.invalid/karar"), ""),
            )
            .expect("çekirdek koşul yazımı");
        assert_eq!(terms.redistribution, "allowed");
        assert!(terms.terms_recorded_at.is_some());
    }

    /// v57: kanıtsız `allowed` iddiası **izin sayılmaz** — korpus metni
    /// dosyaya yazılmaz. v57 öncesi (ya da elle değiştirilmiş) satırı taklit
    /// etmek için iki tetikleyici **bilinçli** düşürülür; kapı yerindeyken aynı
    /// yazmanın reddedildiği ayrıca ölçülür.
    #[test]
    fn corpus_export_does_not_trust_an_unbacked_allowed_claim() {
        let db = db();
        let id = precedent(&db, "Yargıtay", "2024/123");
        with_text(&db, &id, SECRET);

        // Kapı yerindeyken kanıtsız izin yazılamaz.
        let error = db
            .conn()
            .execute(
                "UPDATE precedents SET redistribution='allowed' WHERE id=?1",
                [&id],
            )
            .map_err(StorageError::from)
            .expect_err("kanıtsız izin kapıda reddedilmeli");
        assert_eq!(error.code(), "E_DB_INTEGRITY");

        // v57 öncesi satır: tetikleyiciler bilinçli düşürülür (onarım/legacy yolu).
        db.conn()
            .execute_batch(
                "DROP TRIGGER precedent_usage_terms_guard_bi;
                 DROP TRIGGER precedent_usage_terms_guard_bu;",
            )
            .expect("tetikleyici düşürme");
        db.conn()
            .execute(
                "UPDATE precedents SET redistribution='allowed' WHERE id=?1",
                [&id],
            )
            .expect("eski satır");

        let coverage = db.precedent_corpus_coverage().expect("kapsam");
        assert_eq!(coverage.with_text, 1);
        assert_eq!(coverage.allowed, 0, "kanıtsız izin sayılmaz");
        assert_eq!(coverage.unknown, 1);
        assert_eq!(coverage.blocked_total, 1);

        let export = db
            .build_precedent_corpus(CorpusMode::PermittedOnly)
            .expect("korpus");
        assert_eq!(export.exported, 0);
        assert_eq!(export.excluded, 1);
        assert_eq!(
            export.excluded_items.first().map(|e| e.reason.as_str()),
            Some("redistribution_unknown")
        );
        let items_json = serde_json::to_string(&export.items).expect("json");
        assert!(!items_json.contains("ÖZEL-KORPUS-BELİRTEÇ-7741"));

        let error = db
            .build_precedent_corpus(CorpusMode::All)
            .expect_err("kanıtsız izin `all` kapısını kapatmalı");
        assert_eq!(error.code(), "E_CORPUS_NOT_PERMITTED");
    }
}
