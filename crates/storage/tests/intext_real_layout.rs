//! F12 — **gerçek PDF düzeninden** gelen metinle metin içi alıntı doğrulaması.
//!
//! Bugüne kadarki alıntı testleri elle yazılmış sayfa metinleriyle koşuyordu
//! ("gerçek PDF/worker ile uçtan uca denenmedi" — dürüst sınır). Bu test,
//! ürün worker'ının (`dim_worker.engines.extract_pdf`) **gerçek çıktısını**
//! kullanır: metin katmanı gömülü Unicode fontla üretilmiş 3 sayfalık
//! `fixtures/synthetic/intext-layout.pdf`. Çıkarım kaydı
//! (`fixtures/synthetic/intext-layout.extracted.json`) üretici tarafından
//! yazılır; CI'da Python/PyMuPDF gerekmeden koşar. Kayıtla canlı çıkarımın
//! birebirliğini `crates/ingest/tests/intext_layout_live.rs` (`#[ignore]`)
//! ölçer.
//!
//! Kapatılan sınıf: PDF'ten gelen metin, elle yazılmış metinden **iki** yerde
//! ayrılıyor ve ikisi de alıntıyı sessizce "bulunamadı" yapıyordu:
//!
//! 1. **Ligatür:** font `fl`/`fi` çiftini tek glif yazar; PyMuPDF `U+FB02`
//!    (`ﬂ`) döndürür. `taraflar` yazan avukat metinde `taraﬂar` olduğu için
//!    alıntı bulunamıyordu (`fiili`, `tasfiye`, `taraflar` hukuk metninde sık).
//! 2. **Sayfa sonunda tire:** sayfa metni `ve-\n` ile bitiyor, pencere
//!    ayırıcısı bir `\n` daha ekliyor → tire kuralı iki satır sonunu
//!    "paragraf ayrımı" sayıp birleştirmiyordu; kelime sayfa sınırında
//!    bölündüğünde alıntı bulunamıyordu.

use std::path::PathBuf;

use dim_storage::{
    AiPolicy, CaseKind, Database, DatabaseKey, InTextQuoteInput, NewAnchor, NewCase, NewPage,
    Sensitivity,
};

const LIGATURE_FL: char = '\u{FB02}'; // ﬂ
const LIGATURE_FI: char = '\u{FB01}'; // ﬁ

struct SnapshotPage {
    index: i64,
    text: String,
    width: f64,
    height: f64,
    rotation: i64,
    coordinate_space: String,
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("synthetic")
        .join(name)
}

fn load_snapshot() -> (String, String, Vec<SnapshotPage>) {
    let raw = std::fs::read_to_string(fixture_path("intext-layout.extracted.json"))
        .expect("anlık görüntü okunamadı (fixtures/synthetic/intext-layout.extracted.json)");
    let value: serde_json::Value =
        serde_json::from_str(&raw).expect("anlık görüntü geçerli JSON değil");
    let engine = value["engine"]
        .as_str()
        .expect("engine alanı yok")
        .to_string();
    let engine_version = value["engineVersion"]
        .as_str()
        .expect("engineVersion alanı yok")
        .to_string();
    let pages = value["pages"]
        .as_array()
        .expect("pages alanı yok")
        .iter()
        .map(|page| SnapshotPage {
            index: page["index"].as_i64().expect("page.index"),
            text: page["text"].as_str().expect("page.text").to_string(),
            width: page["width"].as_f64().unwrap_or(595.0),
            height: page["height"].as_f64().unwrap_or(842.0),
            rotation: page["rotation"].as_i64().unwrap_or(0),
            coordinate_space: page["coordinateSpace"]
                .as_str()
                .unwrap_or("unrotated_cropbox_points")
                .to_string(),
        })
        .collect();
    (engine, engine_version, pages)
}

/// Anlık görüntüdeki sayfaları **ürünün yazdığı gibi** (origin `native`)
/// gerçek kasaya yazar; belge kimliğini döndürür.
fn vault_with_fixture(
    pages: &[SnapshotPage],
    engine: &str,
    version: &str,
) -> (Database, String, String) {
    let db = Database::open_in_memory(&DatabaseKey::new([77u8; 32])).expect("kasa");
    let case = db
        .create_case(&NewCase {
            title: "Alıntı düzeni".into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: Sensitivity::Normal,
            default_ai_policy: AiPolicy::Off,
        })
        .expect("dosya");
    let blob = db
        .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
        .expect("blob");
    let document = db
        .create_document(&case.id, "karar.pdf", "pdf", None, "unknown")
        .expect("belge");
    let doc_version = db
        .add_version(
            &document.id,
            &case.id,
            &blob,
            &"c".repeat(64),
            74_295,
            "application/pdf",
        )
        .expect("sürüm");
    let run = db
        .start_parse_run(&doc_version.id, &case.id, engine, version, "pdf|ocr=false")
        .expect("koşu");
    let stored: Vec<NewPage> = pages
        .iter()
        .map(|page| NewPage {
            index: page.index,
            text: page.text.clone(),
            width: Some(page.width),
            height: Some(page.height),
            origin: "native".into(),
            warnings: Vec::new(),
            rotation: page.rotation,
            coordinate_space: Some(page.coordinate_space.clone()),
            page_label: None,
            blocks: Vec::new(),
        })
        .collect();
    db.store_pages(&run.id, &case.id, &stored)
        .expect("sayfalar");
    db.finish_parse_run(&run.id, "succeeded", &[])
        .expect("bitir");
    (db, case.id, document.id)
}

/// Kullanıcının yazdığı metin: alıntılar tipografik tırnak içinde.
const USER_TEXT: &str = "Kararda “KARAR GEREKÇESİ” başlığı var; davacı “bedellini ödemediğini” \
     ileri sürmüş, mahkeme “reddine karar verilmiştir” demiş ve “gerekçesi şöyledir” diye \
     devam etmiş; “ödeme yapılmadığı sabit olup” ifadesi geçiyor. Sözleşme “taraflar arasındaki \
     sözleşme” diye anılmış. Buna karşılık “faiz işletilmesine yer vardır” cümlesi kararda yok.";

#[test]
fn real_pdf_layout_quotes_are_found_with_the_right_layer() {
    let (engine, engine_version, pages) = load_snapshot();
    assert_eq!(engine, "pymupdf", "anlık görüntü ürün worker'ından gelmeli");
    assert!(!engine_version.is_empty());
    assert_eq!(pages.len(), 3, "fikstür üç sayfa olmalı");

    // Fikstür gerçekten ligatür ve sayfa sonu tiresi taşıyor mu? Taşımıyorsa
    // test sessizce boşalırdı (ölçülen olgu kaybolurdu).
    assert!(
        pages[2].text.contains(LIGATURE_FL),
        "fikstürde `fl` ligatürü yok: {:?}",
        pages[2].text
    );
    assert!(
        pages[0].text.ends_with("karar ve-\n"),
        "fikstür sayfa sonunda tire taşımıyor: {:?}",
        pages[0].text
    );
    assert!(
        pages[1].text.starts_with("rilmiştir."),
        "sayfa sınırı devamı beklenen yerde değil: {:?}",
        pages[1].text
    );
    // `fi` ligatürü bu fikstürde yok; normalize'ın onu da açtığı ayrı testte
    // (birim testi) doğrulanır — burada yalnız ölçülen olgu kanıtlanır.
    assert!(!pages.iter().any(|p| p.text.contains(LIGATURE_FI)));

    let (db, case_id, document_id) = vault_with_fixture(&pages, &engine, &engine_version);
    let check = db
        .check_in_text_quotes(
            &case_id,
            &InTextQuoteInput {
                text: USER_TEXT.to_string(),
                document_id: Some(document_id.clone()),
            },
        )
        .expect("kontrol");

    assert_eq!(check.quotes.len(), 7, "yedi alıntı çıkarılmalı");
    assert_eq!(check.pages_scanned, 3);
    assert_eq!(check.documents_with_text, 1);

    struct Expect {
        quote: &'static str,
        state: &'static str,
        kind: &'static str,
        page: i64,
        end_page: i64,
        spans_pages: bool,
    }
    let expectations = [
        Expect {
            quote: "KARAR GEREKÇESİ",
            state: "exact",
            kind: "exact",
            page: 0,
            end_page: 0,
            spans_pages: false,
        },
        Expect {
            quote: "bedellini ödemediğini",
            state: "normalized",
            kind: "dehyphenated",
            page: 0,
            end_page: 0,
            spans_pages: false,
        },
        Expect {
            quote: "reddine karar verilmiştir",
            state: "normalized",
            kind: "dehyphenated",
            page: 0,
            end_page: 1,
            spans_pages: true,
        },
        Expect {
            quote: "gerekçesi şöyledir",
            state: "normalized",
            kind: "whitespace",
            page: 1,
            end_page: 2,
            spans_pages: true,
        },
        Expect {
            quote: "ödeme yapılmadığı sabit olup",
            state: "normalized",
            kind: "whitespace",
            page: 1,
            end_page: 1,
            spans_pages: false,
        },
        Expect {
            quote: "taraflar arasındaki sözleşme",
            state: "normalized",
            kind: "ligature",
            page: 2,
            end_page: 2,
            spans_pages: false,
        },
        Expect {
            quote: "faiz işletilmesine yer vardır",
            state: "not_found",
            kind: "",
            page: -1,
            end_page: -1,
            spans_pages: false,
        },
    ];

    let mut measured = Vec::new();
    for finding in check.quotes.iter() {
        measured.push(format!(
            "{} → {} / {} / s.{}{}",
            finding.quote,
            finding.state,
            finding
                .position
                .as_ref()
                .map(|p| p.match_kind.as_str())
                .unwrap_or("-"),
            finding
                .position
                .as_ref()
                .map(|p| p.page_index)
                .or_else(|| finding.candidates.first().map(|c| c.page_index))
                .map(|index| index.to_string())
                .unwrap_or_else(|| "-".into()),
            if finding.position.as_ref().is_some_and(|p| p.spans_pages) {
                " (sayfa sınırını aşıyor)"
            } else {
                ""
            }
        ));
    }
    // Ölçüm **önce** yazılır: kırmızıda hangi olgu olduğu görünür kalır.
    println!("ölçüm: {}", measured.join(" | "));
    for (finding, expected) in check.quotes.iter().zip(expectations.iter()) {
        assert_eq!(finding.quote, expected.quote, "alıntı sırası");
        assert_eq!(
            finding.state,
            expected.state,
            "`{}` hükmü yanlış (ölçüm: {})",
            expected.quote,
            measured[measured.len() - 1]
        );
        if expected.state == "not_found" {
            assert!(
                finding.position.is_none() && finding.candidates.is_empty(),
                "bulunamayan alıntıda konum olmamalı"
            );
            continue;
        }
        let position = finding
            .position
            .as_ref()
            .unwrap_or_else(|| panic!("`{}` için konum yok", expected.quote));
        assert_eq!(
            position.match_kind,
            expected.kind,
            "`{}` katmanı yanlış (ölçüm: {})",
            expected.quote,
            measured[measured.len() - 1]
        );
        assert_eq!(
            position.page_index, expected.page,
            "`{}` sayfası",
            expected.quote
        );
        assert_eq!(
            position.end_page_index, expected.end_page,
            "`{}` bitiş sayfası",
            expected.quote
        );
        assert_eq!(
            position.spans_pages, expected.spans_pages,
            "`{}` sayfa sınırı bayrağı",
            expected.quote
        );
        // Kaynak metin, gerçek çıkarımdaki ham aralık olmalı: alıntı ile
        // kaynak arasındaki fark **görünür** kalır (gizlenmez).
        assert!(
            !position.source_text.is_empty(),
            "`{}` kaynak metni boş",
            expected.quote
        );
    }

    // Sayfa içi tire ve ligatür: kaynak metin gerçekten ham hâli gösteriyor.
    let dehyphenated = check
        .quotes
        .iter()
        .find(|f| f.quote == "bedellini ödemediğini")
        .and_then(|f| f.position.as_ref())
        .expect("tire birleştirme konumu");
    assert_eq!(
        dehyphenated.source_text, "bedel-\nlini ödemediğini",
        "kaynak metin satır sonu tiresini göstermeli"
    );
    let ligature = check
        .quotes
        .iter()
        .find(|f| f.quote == "taraflar arasındaki sözleşme")
        .and_then(|f| f.position.as_ref())
        .expect("ligatür konumu");
    assert!(
        ligature.source_text.contains(LIGATURE_FL),
        "kaynak metin ligatürü göstermeli: {:?}",
        ligature.source_text
    );
    // Ligatür katmanı birebir değildir: anchor'a bağlanmaz (tek sayfa olsa da).
    assert_ne!(ligature.match_kind, "exact");

    // Birebir tek konum gerçekten bağlanabiliyor mu? (uçtan uca: konum → anchor
    // → okuyucu konumu, alıntı hâlâ yerinde mi)
    let exact = check
        .quotes
        .iter()
        .find(|f| f.quote == "KARAR GEREKÇESİ")
        .and_then(|f| f.position.as_ref())
        .expect("birebir konum");
    let anchor = db
        .create_anchor(&NewAnchor {
            case_id: case_id.clone(),
            document_version_id: exact.document_version_id.clone(),
            parse_run_id: exact.parse_run_id.clone(),
            segment_id: exact.segment_id.clone().expect("segment"),
            start_offset: exact.char_start,
            end_offset: exact.char_end,
            exact_quote: exact.source_text.clone(),
        })
        .expect("anchor");
    let location = db.locate_anchor(&anchor.id).expect("konum").expect("kayıt");
    assert!(location.quote_intact, "bağlanan alıntı metinde durmalı");
    assert_eq!(location.page_index, Some(0));

    // Özet: `--nocapture` ile ölçüm görünür (rapor bunu yazar).
    println!("ölçüm: {}", measured.join(" | "));
    println!(
        "sayaçlar: exact={} normalized={} ambiguous={} not_found={} dehyphenated={} ligature={}",
        check.exact,
        check.normalized,
        check.ambiguous,
        check.not_found,
        check.dehyphenated,
        check.ligature
    );
    assert_eq!(check.exact, 1);
    assert_eq!(check.normalized, 5);
    assert_eq!(check.ambiguous, 0);
    assert_eq!(check.not_found, 1);
    assert_eq!(check.dehyphenated, 2, "sayfa içi + sayfa sınırı tiresi");
    assert_eq!(check.ligature, 1, "yalnız ligatürle bulunan alıntı");
}
