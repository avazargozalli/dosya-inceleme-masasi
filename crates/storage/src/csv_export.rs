//! F14-T02/T04 — CSV çıktısı: **güvenli hücre politikası** ve bölümler.
//!
//! Şartname (REQ-027, "Dışa aktarım"): CSV "UTF-8, belgelenmiş ayraç/tarih
//! biçimi, güvenli hücre politikası" taşır ve kronolojiyi, **belge dizinini** ve
//! görevleri aktarır. Aynı şartname hücre politikasını açıkça tarif eder:
//! "hücrenin başındaki boşluk/kontrol karakterleri hesaba katılarak `=`, `+`,
//! `-`, `@`, bunların tam genişlikli karşılıkları, sekme ve satır başlangıcı
//! içeren riskli metinler güvenli hücre profiline alınır."
//!
//! Politika: riskli hücrenin başına **tek tırnak** konur (tablo uygulamaları
//! hücreyi metin olarak yorumlar, formülü çalıştırmaz), ardından RFC 4180
//! gereği ayraç/tırnak/satır sonu içeren hücre çift tırnaklanır. Risk tespiti
//! hücrenin **ilk anlamlı** karakterine bakar: baştaki boşluk ve kontrol
//! karakterleri (sekme, satır başı, NBSP) atlanır — tablo uygulamaları onları
//! kırpar ve formülü yine çalıştırır.
//!
//! `-` için kural daraltılmıştır: yalnız ardından **boşluk gelmeyen** tire
//! risklidir (`-1+1`, `-A1`, `-cmd|…`); "- zamanaşımı itirazı" gibi düz metin
//! bozulmaz. Bu bilinçli tercih çıktının kendi biçim notunda yazılıdır.
//!
//! Bölüm sırası: kronoloji + görevler (aynı beş sütun, tek tablo) **önce**,
//! belge dizini **en sonda**. Böylece `comment='#'` ile okuyan araçlar ilk
//! tabloyu kesintisiz görür; ikinci şema sonda ayrı bir ek olarak durur.

use crate::documents::Document;
use crate::export_coverage::ExportCoverage;
use crate::tasks::TaskRow;
use crate::timeline::EventRow;

/// Çıktının kendi içinde taşıdığı biçim notu (şartname: "belgelenmiş
/// ayraç/tarih biçimi"). Yorum satırıdır: `#` ile başlar.
pub const CSV_FORMAT_NOTE: &str = "# biçim: ayraç=, · tarih=YYYY-AA-GG · kodlama=UTF-8 · \
riskli hücreler ' ile metne sabitlenir";

/// Kronoloji ve görev tablosunun sütunları (tek tablo, `kayit` = olay/gorev).
const EVENT_COLUMNS: &str = "kayit,tarih,tur,baslik,durum\n";

/// Belge dizini sütunları (ikinci şema, dosyanın sonunda).
const DOCUMENT_COLUMNS: &str = "belge,tur,durum,belge_tarihi,eklenme\n";

/// Hücrenin ilk anlamlı karakteri formül/komut tetikliyor mu?
///
/// Baştaki boşluk ve kontrol karakterleri atlanır; tam genişlikli karşılıklar
/// (`＝ ＋ － ＠`) da tetikleyicidir. Tire yalnız ardından boşluk **gelmiyorsa**
/// risklidir: düz metin tireleri (`- zamanaşımı`) korunur.
fn starts_with_formula(value: &str) -> bool {
    let rest =
        value.trim_start_matches(|c: char| c.is_whitespace() || c.is_control() || c == '\u{feff}');
    let mut chars = rest.chars();
    match chars.next() {
        Some('=' | '+' | '@' | '＝' | '＋' | '＠') => true,
        Some('-' | '－') => chars.next().is_some_and(|next| !next.is_whitespace()),
        _ => false,
    }
}

/// Bir metni CSV hücresi olarak üretir.
///
/// Riskli baş tek tırnakla metne sabitlenir; ayraç, çift tırnak, satır başı ve
/// satır sonu içeren hücre RFC 4180'e göre çift tırnaklanır ve içteki tırnaklar
/// ikilenir.
#[must_use]
pub fn safe_cell(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    if starts_with_formula(value) {
        out.push('\'');
    }
    out.push_str(value);
    if out.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", out.replace('"', "\"\""))
    } else {
        out
    }
}

/// Dosya kapsamının CSV gövdesini üretir.
///
/// Girdi sırası **korunur** (kronoloji kasadaki sırayla, belge dizini en yeni
/// önce); bölümler arasına `#` yorum satırları konur. Sayılar kasadan gelir,
/// burada yeniden hesaplanmaz.
#[must_use]
pub fn build_case_csv(
    coverage: &ExportCoverage,
    events: &[EventRow],
    tasks: &[TaskRow],
    documents: &[Document],
) -> String {
    let mut out = String::with_capacity(
        512 + (events.len() + tasks.len() + documents.len()).saturating_mul(96),
    );
    out.push_str(&coverage.csv_comment_block());
    out.push_str(CSV_FORMAT_NOTE);
    out.push('\n');
    out.push_str("# kronoloji\n");
    out.push_str(EVENT_COLUMNS);
    for event in events {
        out.push_str(&format!(
            "olay,{},{},{},{}\n",
            safe_cell(event.sortable_date.as_deref().unwrap_or("")),
            safe_cell(&event.date_role),
            safe_cell(&event.title),
            safe_cell(&event.status)
        ));
    }
    for task in tasks {
        out.push_str(&format!(
            "gorev,{},{},{},{}\n",
            safe_cell(task.user_due_date.as_deref().unwrap_or("")),
            safe_cell("gorev"),
            safe_cell(&task.title),
            safe_cell(&task.status)
        ));
    }
    out.push_str("# belge dizini\n");
    out.push_str(DOCUMENT_COLUMNS);
    for document in documents {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            safe_cell(&document.display_name),
            safe_cell(&document.document_type),
            safe_cell(&document.status),
            safe_cell(document.declared_document_date.as_deref().unwrap_or("")),
            safe_cell(&document.created_at)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export_coverage::COVERAGE_STATEMENT;

    fn coverage() -> ExportCoverage {
        ExportCoverage {
            documents: 2,
            versions: 2,
            parse_runs: 1,
            pages: 4,
            text_pages: 4,
            ocr_pages: 0,
            segments: 12,
            empty_pages: 0,
            unparsed_documents: 1,
            failed_parse_runs: 0,
            complete: false,
            warnings: vec!["1 belge hiç ayrıştırılmadı".into()],
            statement: COVERAGE_STATEMENT.to_string(),
            generated_at: "2026-11-08T00:00:00Z".into(),
        }
    }

    fn event(title: &str) -> EventRow {
        EventRow {
            id: "e1".into(),
            case_id: "c1".into(),
            event_kind: "hearing".into(),
            date_value: Some("2024-05-01".into()),
            date_precision: "day".into(),
            date_role: "hearing".into(),
            date_raw: None,
            sortable_date: Some("2024-05-01".into()),
            title: title.into(),
            description: String::new(),
            assertion_kind: "user".into(),
            status: "open".into(),
            created_at: "2026-11-08T00:00:00Z".into(),
            updated_at: "2026-11-08T00:00:00Z".into(),
            row_version: 1,
        }
    }

    fn task(title: &str) -> TaskRow {
        TaskRow {
            id: "t1".into(),
            case_id: "c1".into(),
            title: title.into(),
            status: "open".into(),
            user_due_date: Some("2024-06-01".into()),
            date_timezone: Some("Europe/Istanbul".into()),
            date_origin: "user".into(),
            assignee_label: None,
            reminder_policy: "none".into(),
            date_confirmed_at: None,
            created_at: "2026-11-08T00:00:00Z".into(),
            updated_at: "2026-11-08T00:00:00Z".into(),
            row_version: 1,
        }
    }

    fn document(name: &str) -> Document {
        Document {
            id: "d1".into(),
            case_id: "c1".into(),
            display_name: name.into(),
            document_type: "pdf".into(),
            status: "parsed".into(),
            current_version_id: Some("v1".into()),
            declared_document_date: Some("2024-04-01".into()),
            created_at: "2026-11-08T00:00:00Z".into(),
            updated_at: "2026-11-08T00:00:00Z".into(),
            row_version: 1,
        }
    }

    #[test]
    fn safe_cell_guards_formula_triggers_including_full_width() {
        for value in [
            "=1+1",
            "+90 532",
            "@kullanici",
            "-1+1",
            "-A1",
            "-cmd|' /C calc'!A0",
            "＝1+1",
            "＋90",
            "＠x",
            "－1",
            "\t=1+1",
            " =1+1",
            "\u{00a0}@x",
        ] {
            let cell = safe_cell(value);
            assert_eq!(cell, format!("'{value}"), "{value:?} korunmadı");
        }
    }

    #[test]
    fn safe_cell_keeps_plain_text_and_documents_the_hyphen_rule() {
        assert_eq!(safe_cell("kira sözleşmesi"), "kira sözleşmesi");
        assert_eq!(safe_cell("- zamanaşımı itirazı"), "- zamanaşımı itirazı");
        assert_eq!(safe_cell("2024/1234"), "2024/1234");
        assert_eq!(safe_cell(""), "");
        // Ayraç / tırnak / satır sonu: RFC 4180 tırnaklaması.
        assert_eq!(safe_cell("a,b"), "\"a,b\"");
        assert_eq!(safe_cell("a\"b"), "\"a\"\"b\"");
        assert_eq!(safe_cell("a\rb"), "\"a\rb\"");
        assert_eq!(safe_cell("a\nb"), "\"a\nb\"");
    }

    #[test]
    fn build_csv_carries_chronology_tasks_and_document_index() {
        let csv = build_case_csv(
            &coverage(),
            &[event("kira tespiti")],
            &[task("cevap dilekçesi")],
            &[document("kira.pdf")],
        );
        assert!(csv.starts_with("# Dosya İnceleme Masası — kapsam özeti\n"));
        assert!(csv.contains(CSV_FORMAT_NOTE));
        assert!(csv.contains("# kronoloji\nkayit,tarih,tur,baslik,durum\n"));
        assert!(csv.contains("olay,2024-05-01,hearing,kira tespiti,open\n"));
        assert!(csv.contains("gorev,2024-06-01,gorev,cevap dilekçesi,open\n"));
        assert!(csv.contains("# belge dizini\nbelge,tur,durum,belge_tarihi,eklenme\n"));
        assert!(csv.contains("kira.pdf,pdf,parsed,2024-04-01,2026-11-08T00:00:00Z\n"));
        // Belge dizini en sonda: ilk tablo kesintisiz kalır.
        let task_at = csv.find("gorev,2024-06-01").expect("görev satırı");
        let index_at = csv.find("# belge dizini").expect("dizin başlığı");
        assert!(index_at > task_at, "belge dizini görevlerden sonra gelmeli");
    }

    #[test]
    fn hostile_text_is_neutralized_inside_the_csv_body() {
        let csv = build_case_csv(
            &coverage(),
            &[event("=cmd|' /C calc'!A0")],
            &[task("Kira, \"özel\"")],
            &[document("＝1+1.pdf")],
        );
        assert!(csv.contains("'=cmd|"), "formül tetikleyicisi korunmadı");
        assert!(!csv.contains("\n=cmd|"), "satır başında çıplak formül var");
        // Görev başlığındaki ayraç/tırnak tırnaklanır, tire kuralı metni bozmaz.
        assert!(csv.contains("gorev,2024-06-01,gorev,\"Kira, \"\"özel\"\"\",open\n"));
        assert!(csv.contains("'＝1+1.pdf,pdf,parsed"));
    }
}
