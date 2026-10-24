//! F11-T06 — kapsamlılık raporunun **dışa aktarma çıktısına gömülmesi**.
//!
//! Dışa aktarılan her çıktı, üzerinde çalışılan kapsamı kendi içinde taşır:
//! kaç belge/sayfa ayrıştırıldı, kaç sayfa OCR ile okundu, hangi belge hiç
//! ayrıştırılmadı. Amaç tek bir cümleyi görünür tutmaktır: **incelenmedi ≠
//! temiz**. Rapor kasadan çıktıktan sonra kimse "her şey incelendi" sanmamalı.
//!
//! Metinler burada **tek yerde** üretilir; JSON, DOCX/PDF, CSV ve devir paketi
//! aynı dili kullanır. Sayılar `Database::coverage_report`'tan gelir, burada
//! yeniden hesaplanmaz — tek doğruluk kaynağı kasadır.

use serde::{Deserialize, Serialize};

use crate::ai::CoverageReport;
use crate::now_iso;

/// Her çıktıda aynen tekrarlanan sınır cümlesi.
pub const COVERAGE_STATEMENT: &str =
    "İncelenmedi ≠ temiz: bu çıktı yalnız kasadaki ayrıştırılmış metni \
kapsar; kapsam dışında kalan belge ve sayfalar incelenmiş sayılmaz.";

/// Sayımların nasıl okunacağını söyleyen teknik not (markdown çıktısında).
pub const COVERAGE_COUNTING_NOTE: &str = "Sayımlar kasadaki ayrıştırma koşuları üzerinden alınır: \
aynı belge birden çok kez ayrıştırıldıysa sayfa sayısı koşu sayısıyla birlikte artar. \
Belge sayısı silinmemiş kayıtları sayar.";

/// Dışa aktarma çıktısına gömülen kapsam bloğu.
///
/// `complete` yalnız "her belge ayrıştırıldı ve her sayfada metin var" demektir;
/// OCR kullanımı kapsamı eksik yapmaz ama güvenilirlik uyarısı üretir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCoverage {
    pub documents: i64,
    pub versions: i64,
    pub parse_runs: i64,
    pub pages: i64,
    /// Metni olan sayfa sayısı (`pages - empty_pages`).
    pub text_pages: i64,
    pub ocr_pages: i64,
    pub segments: i64,
    pub empty_pages: i64,
    pub unparsed_documents: i64,
    pub failed_parse_runs: i64,
    /// Kapsam tam mı? Uyarı varsa `false`.
    pub complete: bool,
    /// İnsan diliyle uyarılar (her biri tek satır).
    pub warnings: Vec<String>,
    /// Sabit sınır cümlesi ([`COVERAGE_STATEMENT`]).
    pub statement: String,
    pub generated_at: String,
}

impl ExportCoverage {
    /// Kasadaki rapordan gömülebilir bloğu üretir.
    #[must_use]
    pub fn from_report(report: &CoverageReport) -> Self {
        let text_pages = (report.pages - report.empty_pages).max(0);
        let mut warnings = Vec::new();
        if report.documents == 0 {
            warnings.push("Dosyada hiç belge yok; çıktı boş kapsam üzerine kuruludur.".to_string());
        }
        if report.unparsed_documents > 0 {
            warnings.push(format!(
                "{} belge hiç ayrıştırılmadı; içerikleri incelenmedi.",
                report.unparsed_documents
            ));
        }
        if report.empty_pages > 0 {
            warnings.push(format!(
                "{} sayfada metin yok (taranmış görüntü olabilir); bu sayfalar okunmadı.",
                report.empty_pages
            ));
        }
        if report.failed_parse_runs > 0 {
            warnings.push(format!(
                "{} ayrıştırma koşusu başarısız oldu; o koşuların sayfaları eksik olabilir.",
                report.failed_parse_runs
            ));
        }
        if report.ocr_pages > 0 {
            warnings.push(format!(
                "{} sayfa OCR ile okundu; OCR yazım hatası olabilir, alıntılar elle doğrulanmalı.",
                report.ocr_pages
            ));
        }
        let complete = report.documents > 0
            && report.unparsed_documents == 0
            && report.empty_pages == 0
            && report.failed_parse_runs == 0;
        Self {
            documents: report.documents,
            versions: report.versions,
            parse_runs: report.parse_runs,
            pages: report.pages,
            text_pages,
            ocr_pages: report.ocr_pages,
            segments: report.segments,
            empty_pages: report.empty_pages,
            unparsed_documents: report.unparsed_documents,
            failed_parse_runs: report.failed_parse_runs,
            complete,
            warnings,
            statement: COVERAGE_STATEMENT.to_string(),
            generated_at: now_iso(),
        }
    }

    /// Rapor tablosunun satırları (başlık hariç): ölçü adı + değer.
    #[must_use]
    pub fn rows(&self) -> Vec<(&'static str, i64)> {
        vec![
            ("Belge", self.documents),
            ("Belge sürümü", self.versions),
            ("Ayrıştırma koşusu", self.parse_runs),
            ("Sayfa", self.pages),
            ("Metinli sayfa", self.text_pages),
            ("OCR ile okunan sayfa", self.ocr_pages),
            ("Boş sayfa", self.empty_pages),
            ("Metin segmenti", self.segments),
            ("Ayrıştırılmamış belge", self.unparsed_documents),
            ("Başarısız ayrıştırma koşusu", self.failed_parse_runs),
        ]
    }

    /// Tek satırlık özet (başlık/meta ve CSV yorumu için).
    #[must_use]
    pub fn headline(&self) -> String {
        format!(
            "{} belge · {} sayfa ({} metinli, {} OCR, {} boş) · {} ayrıştırılmamış belge",
            self.documents,
            self.pages,
            self.text_pages,
            self.ocr_pages,
            self.empty_pages,
            self.unparsed_documents
        )
    }

    /// Rapor gövdesine eklenecek uyarı/sınır satırları.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .warnings
            .iter()
            .map(|w| format!("Uyarı: {w}"))
            .collect();
        out.push(self.statement.clone());
        out
    }

    /// CSV başına konan `#` yorum bloğu. Tablo satırlarına karışmaz; `#` ile
    /// başlayan satırları yorum sayan okuyucular (pandas `comment='#'`) temiz
    /// okur, saymayanlar için de insan tarafından görünür kalır.
    #[must_use]
    pub fn csv_comment_block(&self) -> String {
        let mut out = String::from("# Dosya İnceleme Masası — kapsam özeti\n");
        out.push_str(&format!("# {}\n", one_line(&self.headline())));
        out.push_str(&format!(
            "# kapsam={}\n",
            if self.complete { "tam" } else { "kısmi" }
        ));
        out.push_str(&format!("# üretim={}\n", one_line(&self.generated_at)));
        for warning in &self.warnings {
            out.push_str(&format!("# UYARI: {}\n", one_line(warning)));
        }
        out.push_str(&format!("# {}\n", one_line(&self.statement)));
        out
    }

    /// Devir paketine konan `KAPSAM.md` içeriği.
    #[must_use]
    pub fn markdown(&self, case_title: &str) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# İncelenen kapsam — {}\n\n",
            one_line(case_title)
        ));
        out.push_str(&format!(
            "- Dışa aktarma: {}\n",
            one_line(&self.generated_at)
        ));
        out.push_str(&format!(
            "- Kapsam durumu: **{}**\n",
            if self.complete { "tam" } else { "kısmi" }
        ));
        out.push_str(&format!("- Özet: {}\n\n", one_line(&self.headline())));
        out.push_str("| Ölçü | Değer |\n| --- | ---: |\n");
        for (label, value) in self.rows() {
            out.push_str(&format!("| {label} | {value} |\n"));
        }
        out.push_str("\n## Uyarılar\n\n");
        if self.warnings.is_empty() {
            out.push_str("- Yok.\n");
        } else {
            for warning in &self.warnings {
                out.push_str(&format!("- {}\n", one_line(warning)));
            }
        }
        out.push_str(&format!("\n## Sınır\n\n{}\n", one_line(&self.statement)));
        out.push_str(&format!("\n{}\n", one_line(COVERAGE_COUNTING_NOTE)));
        out
    }
}

/// Tek satıra indirger: satır sonu ve fazla boşluk metin yapısını bozmasın.
fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> CoverageReport {
        CoverageReport {
            documents: 4,
            versions: 5,
            parse_runs: 6,
            pages: 40,
            ocr_pages: 3,
            segments: 120,
            empty_pages: 2,
            unparsed_documents: 1,
            failed_parse_runs: 1,
        }
    }

    #[test]
    fn complete_report_has_no_warnings() {
        let coverage = ExportCoverage::from_report(&CoverageReport {
            documents: 2,
            versions: 2,
            parse_runs: 2,
            pages: 10,
            ocr_pages: 0,
            segments: 30,
            empty_pages: 0,
            unparsed_documents: 0,
            failed_parse_runs: 0,
        });
        assert!(coverage.complete);
        assert!(coverage.warnings.is_empty());
        assert_eq!(coverage.text_pages, 10);
        assert_eq!(coverage.lines(), vec![COVERAGE_STATEMENT.to_string()]);
    }

    #[test]
    fn warnings_name_every_gap_with_numbers() {
        let coverage = ExportCoverage::from_report(&report());
        assert!(!coverage.complete);
        assert_eq!(coverage.warnings.len(), 4);
        assert!(coverage.warnings[0].contains("1 belge hiç ayrıştırılmadı"));
        assert!(coverage.warnings[1].contains("2 sayfada metin yok"));
        assert!(coverage.warnings[2].contains("1 ayrıştırma koşusu başarısız"));
        assert!(coverage.warnings[3].contains("3 sayfa OCR"));
        assert_eq!(coverage.text_pages, 38);
    }

    #[test]
    fn empty_case_is_not_reported_as_clean() {
        let coverage = ExportCoverage::from_report(&CoverageReport {
            documents: 0,
            versions: 0,
            parse_runs: 0,
            pages: 0,
            ocr_pages: 0,
            segments: 0,
            empty_pages: 0,
            unparsed_documents: 0,
            failed_parse_runs: 0,
        });
        assert!(!coverage.complete);
        assert_eq!(coverage.warnings.len(), 1);
        assert!(coverage.warnings[0].contains("hiç belge yok"));
    }

    #[test]
    fn every_output_carries_the_statement() {
        let coverage = ExportCoverage::from_report(&report());
        let csv = coverage.csv_comment_block();
        let md = coverage.markdown("Örnek Dosya");
        let json = serde_json::to_string(&coverage).expect("json");
        assert!(csv.starts_with("# Dosya İnceleme Masası — kapsam özeti\n"));
        assert!(csv.contains(COVERAGE_STATEMENT));
        assert!(md.contains(COVERAGE_STATEMENT));
        assert!(json.contains("incelenmedi"));
        // CSV: yorum satırlarının hepsi `#` ile başlar, tablo başlığı kirlenmez.
        for line in csv.lines() {
            assert!(line.starts_with('#'), "yorum olmayan satır: {line}");
        }
    }

    #[test]
    fn csv_block_carries_partial_state_and_counts() {
        let coverage = ExportCoverage::from_report(&report());
        let csv = coverage.csv_comment_block();
        assert!(csv.contains("# kapsam=kısmi"));
        assert!(csv.contains("4 belge · 40 sayfa (38 metinli, 3 OCR, 2 boş)"));
        assert!(csv.contains("# UYARI: 1 belge hiç ayrıştırılmadı"));
    }

    #[test]
    fn markdown_table_lists_counts_and_limits() {
        let coverage = ExportCoverage::from_report(&report());
        let md = coverage.markdown("Örnek Dosya");
        assert!(md.starts_with("# İncelenen kapsam — Örnek Dosya\n"));
        assert!(md.contains("| Ayrıştırılmamış belge | 1 |"));
        assert!(md.contains("| Metinli sayfa | 38 |"));
        assert!(md.contains("## Uyarılar"));
        assert!(md.contains(COVERAGE_COUNTING_NOTE));
        assert!(md.contains("**kısmi**"));
    }

    #[test]
    fn user_text_cannot_break_markdown_structure() {
        let coverage = ExportCoverage::from_report(&report());
        let md = coverage.markdown("Dosya\n# Sahte başlık");
        assert!(md.starts_with("# İncelenen kapsam — Dosya # Sahte başlık\n"));
        assert_eq!(md.lines().filter(|l| l.starts_with("# ")).count(), 1);
    }

    #[test]
    fn block_is_built_from_the_vault_numbers() {
        use crate::cases::{CaseKind, NewCase};
        use crate::{Database, DatabaseKey};

        let db = Database::open_in_memory(&DatabaseKey::new([71u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kapsam".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: crate::Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        let coverage = ExportCoverage::from_report(&db.coverage_report(&case.id).expect("rapor"));
        assert_eq!(coverage.documents, 0);
        assert_eq!(coverage.pages, 0);
        assert!(!coverage.complete);
        let value = serde_json::to_value(&coverage).expect("json");
        assert_eq!(value["unparsedDocuments"], 0);
        assert_eq!(value["textPages"], 0);
        assert_eq!(value["statement"], COVERAGE_STATEMENT);
        assert!(value["warnings"][0]
            .as_str()
            .unwrap_or_default()
            .contains("hiç belge yok"));
    }
}
