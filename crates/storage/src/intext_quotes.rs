//! Metin içi alıntı denetimi (F12-T14).
//!
//! Serbest metindeki (iddia, özet, not) tırnaklı pasajları **çıkarır** ve
//! dosyadaki belgelerin güncel sürümlerinin metin katmanında doğrular.
//!
//! Üç eşleşme katmanı ayrı raporlanır: `exact` (birebir), `whitespace` (yalnız
//! boşluk/satır farkı), `ligature` (font kaynaklı bitişik yazı: `ﬂ`→`fl`),
//! `apostrophe` (kesme işareti yazımı: `’` ↔ `'`), `turkish_fold` (harf
//! sadeleştirmesi) ve `dehyphenated` (satır/sayfa sonunda tire ile bölünmüş
//! kelime). Katmanlar sırayla denenir; üst katman bulunduysa alt katman
//! aranmaz.
//!
//! `not_found` sonucu "uydurma" hükmü değildir: yalnız taranan metin
//! katmanında bulunmadığını söyler. Taranmış (OCR'sız) sayfada görünen metin
//! bu denetimde bulunamaz.
//!
//! Tek sayfa taramasından sonra, hâlâ bulunamayan alıntılar için **sayfa
//! sınırını aşan** eşleşme aranır: ardışık sayfalar bir pencerede birleştirilir,
//! yalnız iki sayfaya da gerçekten yayılan eşleşmeler kaydedilir. Böyle bir
//! eşleşme `spansPages` ile işaretlenir; tek sayfalık anchor'a bağlanmaz.
//!
//! Güncel sürümde bulunamayan alıntı için **belgenin eski sürümleri** de
//! taranır (en çok `MAX_HISTORICAL_VERSIONS_PER_DOCUMENT`, yeniden yeniye).
//! Eski sürümde bulunan konum `isCurrentVersion=false` taşır: alıntı güncel
//! metinde geçmiyor, belge güncellenmiş olabilir. Anchor sürümü zaten kaydettiği
//! için bağlanabilir, ama arayüz bunu ayrıca yazar.

use std::collections::BTreeSet;

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::precedents::{
    apostrophe_with_map, char_offset, collapse_ws_with_map, dehyphenate_with_map, elided_segments,
    find_elided_sequences_in_text, find_quote_in_scopes, ligature_with_map, turkish_fold_with_map,
    MappedText, QuoteTier, TextScope, MAX_HISTORICAL_VERSIONS_PER_PRECEDENT, MIN_QUOTE_CHARS,
};
use crate::{now_iso, Database, StorageError};

/// Denetlenecek serbest metnin üst sınırı (karakter).
pub const MAX_IN_TEXT_CHECK_CHARS: usize = 50_000;
/// Tek metinde denetlenecek alıntı sayısı üst sınırı.
pub const MAX_IN_TEXT_QUOTES: usize = 30;
/// Kütüphane geneli aramada taranacak emsal sayısı üst sınırı (F12-T14k).
/// Aşılırsa rapor `truncated` yazar; `bulunamadı` kesin sayılmaz.
pub const MAX_LIBRARY_PRECEDENTS: usize = 200;
/// Alıntı başına listelenecek eşleşen emsal sayısı üst sınırı; kalanı
/// `hiddenMatches` olarak sayılır (sessiz kesme yok).
pub const MAX_LIBRARY_MATCHES_PER_QUOTE: usize = 20;
/// Kütüphane geneli aramada güncel metinde bulunamayan alıntılar için
/// taranacak **eski sürüm** metinlerine ayrılan toplam karakter bütçesi
/// (F12-T14l). Bütçe aşılırsa kalan eski sürümler taranmaz ve eşleşmeyen
/// alıntı `partial_scan` olur — `bulunamadı` kesin sayılmaz (sessiz kesme yok).
pub const MAX_LIBRARY_HISTORY_CHARS: usize = 4_000_000;
/// Tek alıntının üst sınırı: daha uzunu büyük olasılıkla tırnak hatasıdır.
const MAX_SPAN_CHARS: usize = 1_000;
/// Bulunulan konumdan rapora taşınacak en çok aday sayısı.
const MAX_CANDIDATES: usize = 8;
/// Tek denetimde taranacak sayfa üst sınırı; aşılırsa belge seçilmelidir.
const MAX_SCOPE_PAGES: i64 = 20_000;
/// Belge başına taranacak **eski** sürüm sayısı üst sınırı. Güncel sürüm her
/// zaman taranır; sınırın ötesindeki daha eski sürümler taranmaz ve durum
/// notlarda yazılır (sessiz kesme yok).
const MAX_HISTORICAL_VERSIONS_PER_DOCUMENT: usize = 5;

/// Alıntı çıkarımının kapsam notu: üç yol (dosya, tek emsal, kütüphane) da
/// **aynı** metni yazar. Kopyalar ayrışırsa bir yol «tek tırnak tanınmaz»
/// derken öteki tanır — tek kaynak (F12-T14n).
const QUOTE_EXTRACTION_NOTE: &str =
    "Alıntı çıkarımı çift tırnak ailesini (“…” · „…“ · «…» · \"…\") ve tek tırnak ailesini \
     (‘…’ · '…') tanır; Türkçe kesme işareti (İstanbul'da, Anayasa’nın) alıntı sınırı değildir. \
     Alıntı içindeki “…” / “...” kısaltma işareti parçaları ayırır: parçalar kaynakta aynı \
     sırayla aranır, atlanan kısım denetlenmez. İşaret alıntının başında/sonundaysa alıntı \
     kırpılmıştır; kalan tek parça da aranır.";

/// "Sayfada kullanılabilir metin var mı" tanımı **tek yerde**. Sayfa taraması
/// ([`Database::scan_quote_tier`]) ve sayım ([`Database::page_counts`]) aynı
/// ifadeyi kullanır; iki tanım ayrışırsa `pages_scanned` "tarandı" dediği
/// sayfayı taramayabilir (metin katmanı olmayan sayfa `not_found` hükmü
/// üretirdi — alıntı uydurma sanılırdı). Kapsam raporu
/// (`ai::Database::coverage_report`) da aynı ifadeyi kullanır: "metinli sayfa"
/// iki yerde farklı tanımlanamaz.
pub(crate) const PAGE_TEXT_EXPR: &str =
    "COALESCE((SELECT s.raw_text FROM text_segments s WHERE s.page_id=p.id ORDER BY s.rowid LIMIT 1),'')";

/// Kapsam sayfalarını okur: kimlik, indeks, metin kaynağı, segment ve **ham**
/// metin. Metin ifadesi [`PAGE_TEXT_EXPR`]den gelir.
fn scope_page_sql() -> String {
    format!(
        "SELECT p.id,p.page_index,p.text_origin,\
         (SELECT s.id FROM text_segments s WHERE s.page_id=p.id LIMIT 1),\
         {PAGE_TEXT_EXPR} \
         FROM pages p WHERE p.parse_run_id=?1 ORDER BY p.page_index"
    )
}

/// Metinden çıkarılmış tek alıntı. `start`/`end`, alıntı **içeriğinin**
/// (tırnaklar hariç) karakter aralığıdır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedQuote {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub style: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractedQuotes {
    pub quotes: Vec<SpannedQuote>,
    pub skipped_short: usize,
    pub skipped_long: usize,
    pub unclosed: usize,
}

/// Tek tırnak ailesi: açılış ve **kendi** kapanışı. Karışık stiller birbirini
/// kapatmaz — `«…»` içindeki `“…”` dış alıntıyı erken kapatmaz (T14m).
///
/// Tek tırnak aileleri (T14n) `is_single()` ile işaretlidir: Türkçe kesme
/// işareti (`İstanbul'da`, `Anayasa’nın`) bu ailelerin sınırıyla karıştığı
/// için iki koruma uygulanır — kapanış ancak **harf/rakamdan önce
/// gelmiyorsa** kapanıştır ve düz tek tırnak yalnız **kelime başında**
/// açılış olabilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteFamily {
    Typographic,
    German,
    Guillemet,
    Straight,
    SingleTypographic,
    SingleStraight,
}

impl QuoteFamily {
    const ALL: [QuoteFamily; 6] = [
        QuoteFamily::Typographic,
        QuoteFamily::German,
        QuoteFamily::Guillemet,
        QuoteFamily::Straight,
        QuoteFamily::SingleTypographic,
        QuoteFamily::SingleStraight,
    ];

    fn name(self) -> &'static str {
        match self {
            QuoteFamily::Typographic => "typographic",
            QuoteFamily::German => "german",
            QuoteFamily::Guillemet => "guillemet",
            QuoteFamily::Straight => "straight",
            QuoteFamily::SingleTypographic => "single-typographic",
            QuoteFamily::SingleStraight => "single-straight",
        }
    }

    fn closer(self) -> char {
        match self {
            QuoteFamily::Typographic => '\u{201D}',
            QuoteFamily::German => '\u{201C}',
            QuoteFamily::Guillemet => '\u{00BB}',
            QuoteFamily::Straight => '"',
            QuoteFamily::SingleTypographic => '\u{2019}',
            QuoteFamily::SingleStraight => '\'',
        }
    }

    fn opener(ch: char) -> Option<Self> {
        match ch {
            '\u{201C}' => Some(QuoteFamily::Typographic),
            '\u{201E}' => Some(QuoteFamily::German),
            '\u{00AB}' => Some(QuoteFamily::Guillemet),
            '"' => Some(QuoteFamily::Straight),
            '\u{2018}' => Some(QuoteFamily::SingleTypographic),
            '\'' => Some(QuoteFamily::SingleStraight),
            _ => None,
        }
    }

    /// Tek tırnak ailesi mi? Kesme işareti korumaları yalnız bu ailelerde
    /// uygulanır (Türkçe'de `’` ve `'` kesme işaretidir).
    fn is_single(self) -> bool {
        matches!(
            self,
            QuoteFamily::SingleTypographic | QuoteFamily::SingleStraight
        )
    }

    fn slot(self) -> usize {
        match self {
            QuoteFamily::Typographic => 0,
            QuoteFamily::German => 1,
            QuoteFamily::Guillemet => 2,
            QuoteFamily::Straight => 3,
            QuoteFamily::SingleTypographic => 4,
            QuoteFamily::SingleStraight => 5,
        }
    }
}

/// `chars[index]` bu ailenin **gerçek** kapanışı mı? Tek tırnakta kapanış
/// ancak harf/rakamdan **önce gelmiyorsa** kapanıştır: `‘İstanbul’da
/// yaşanan olay’` içindeki ilk `’` Türkçe kesme işaretidir, alıntıyı
/// kapatmaz; son `’` (ardından boşluk) kapatır. Aksi hâlde alıntı ilk
/// kesme işaretinde kesilir ve metnin kalanı sahte alıntı/sessiz yutma olur.
fn closer_is_boundary(family: QuoteFamily, chars: &[char], index: usize) -> bool {
    if chars[index] != family.closer() {
        return false;
    }
    if family.is_single() {
        return !matches!(chars.get(index + 1), Some(next) if next.is_alphanumeric());
    }
    true
}

/// `chars[index]` bu ailenin **geçerli** açılışı mı? Bir alıntı **kelime/rakam
/// ortasında** açılamaz: `İstanbul'da` kesme işaretidir, `15"` / `15“` ölçü
/// işaretidir; ikisi de alıntı başlatmaz (T14n, T14o). Önceki karakter
/// harf/rakam değilse (boşluk, noktalama, metin başı) açılış geçerlidir.
fn opens_here(chars: &[char], index: usize) -> bool {
    index == 0 || !chars[index - 1].is_alphanumeric()
}

/// Serbest metindeki çift tırnak ailesini (`“…”`, `„…“`, `«…»`, `"…"`) ve
/// tek tırnak ailesini (`‘…’`, `'…'`) çıkarır. Tek tırnakta Türkçe kesme
/// işareti sınır sayılmaz: `İstanbul'da` alıntı açmaz, `‘İstanbul’da’`
/// içindeki `’` alıntıyı kapatmaz (T14n).
///
/// Açılış **kendi ailesinin** kapanışıyla eşleşir; iç içe tırnaklar yığında
/// tutulur ve dış alıntının sınırı iç alıntıyla bölünmez (ör. `“A «B» C”` tek
/// alıntıdır). Kapanışı hiç olmayan bir açılış alıntı başlatmaz, `unclosed`
/// sayılır ama **ondan sonraki** sağlam alıntıların taranmasını durdurmaz.
#[must_use]
pub fn extract_quoted_spans(text: &str) -> ExtractedQuotes {
    let chars: Vec<char> = text.chars().collect();
    // `closer_after[i][aile]`: i. konumdan **sonra** o ailenin gerçek kapanış
    // işareti var mı? Kapanışı hiç olmayan açılış yığına girmez (sessiz yutma
    // yok: `unclosed` sayılır), böylece tek bir kapanmamış tırnak metnin geri
    // kalanındaki alıntıları gizlemez.
    let mut closer_after = vec![[false; 6]; chars.len() + 1];
    for index in (0..chars.len()).rev() {
        closer_after[index] = closer_after[index + 1];
        for family in QuoteFamily::ALL {
            if closer_is_boundary(family, &chars, index) {
                closer_after[index][family.slot()] = true;
            }
        }
    }

    let mut out = ExtractedQuotes::default();
    let mut stack: Vec<(QuoteFamily, usize)> = Vec::new();
    for (index, &ch) in chars.iter().enumerate() {
        // Önce kapanış: `“` hem tipografik açılış hem alman kapanışıdır; açık
        // alman alıntısının içinde kapanış olarak okunur. Tek tırnakta kesme
        // işareti kapanış sayılmaz (T14n).
        if let Some(&(family, open)) = stack.last() {
            if closer_is_boundary(family, &chars, index) {
                stack.pop();
                if stack.is_empty() {
                    emit_span(&chars, family, open, index, &mut out);
                }
                continue;
            }
        }
        if let Some(family) = QuoteFamily::opener(ch) {
            if opens_here(&chars, index) {
                if closer_after[index + 1][family.slot()] {
                    stack.push((family, index));
                } else {
                    out.unclosed += 1;
                }
            }
        }
    }
    out.unclosed += stack.len();
    out
}

/// Yığından çıkan dış alıntıyı kırpıp rapora ekler. Konumlar **içeriğin**
/// (tırnaklar hariç) karakter aralığıdır; kısa/uzun alıntı sayılır, döndürülmez.
fn emit_span(
    chars: &[char],
    family: QuoteFamily,
    open: usize,
    close: usize,
    out: &mut ExtractedQuotes,
) {
    let raw: String = chars[open + 1..close].iter().collect();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        out.skipped_short += 1;
        return;
    }
    let lead = raw.chars().take_while(|c| c.is_whitespace()).count();
    let text_len = trimmed.chars().count();
    if text_len < MIN_QUOTE_CHARS {
        out.skipped_short += 1;
    } else if text_len > MAX_SPAN_CHARS {
        out.skipped_long += 1;
    } else {
        out.quotes.push(SpannedQuote {
            start: open + 1 + lead,
            end: open + 1 + lead + text_len,
            text: trimmed.to_string(),
            style: family.name(),
        });
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuoteInput {
    pub text: String,
    /// Verilirse yalnız bu belgenin **güncel** sürümü taranır.
    #[serde(default)]
    pub document_id: Option<String>,
}

/// Alıntının bulunduğu gerçek konum. `sourceText`, kaynak metnin o aralıktaki
/// **ham** hâlidir; `normalized` eşleşmede yazılan alıntıdan farklı olabilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuotePosition {
    pub document_id: String,
    pub document_name: String,
    pub document_version_id: String,
    pub parse_run_id: String,
    pub page_id: Option<String>,
    pub segment_id: Option<String>,
    pub page_index: i64,
    /// Unicode scalar konumu (bayt değil). Sayfa sınırını aşan eşleşmede
    /// `char_start` **ilk** sayfadaki, `char_end` **son** sayfadaki konumdur
    /// (`end_page_index`).
    pub char_start: i64,
    pub char_end: i64,
    /// Eşleşmenin bittiği sayfa. Sayfa sınırını aşmayan eşleşmede
    /// `page_index` ile aynıdır.
    pub end_page_index: i64,
    /// Eşleşme sayfa sınırını aşıyor mu (kaynakta sayfa/satır sonu).
    pub spans_pages: bool,
    /// Kaynağın sürüm numarası (`document_versions.version_no`).
    pub version_no: i64,
    /// Konum **güncel** sürümde mi? `false` ise alıntı yalnız eski bir sürümde
    /// bulundu; belge güncellenmiş olabilir.
    pub is_current_version: bool,
    /// `exact` | `whitespace` | `turkish_fold`
    pub match_kind: String,
    pub text_origin: String,
    pub source_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuoteFinding {
    pub index: usize,
    pub quote: String,
    /// `exact` | `normalized` | `ambiguous` | `not_found` | `unverifiable`
    pub state: String,
    /// `no_text` (taranacak metin yok) — başka neden uydurulmaz.
    pub reason: Option<String>,
    /// Eşleşen **sürüm içindeki** konum sayısı. Aynı pasajın birden çok sürümde
    /// (düzeltilmiş/uzun nüsha) geçmesi ayrı konum sayılmaz: en yeni sürüm seçilir.
    pub occurrences: i64,
    pub distinct_documents: i64,
    /// Yalnız tek konum varsa dolu.
    pub position: Option<InTextQuotePosition>,
    /// Birden fazla konum varsa ilk adaylar (en çok 8).
    pub candidates: Vec<InTextQuotePosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InTextQuoteCheck {
    pub case_id: String,
    pub scope: String,
    pub scope_document_id: Option<String>,
    pub scope_document_name: Option<String>,
    pub text_chars: i64,
    pub spans_found: usize,
    pub skipped_short: usize,
    pub skipped_long: usize,
    pub unclosed: usize,
    pub quotes: Vec<InTextQuoteFinding>,
    pub exact: usize,
    pub normalized: usize,
    pub ambiguous: usize,
    pub not_found: usize,
    pub unverifiable: usize,
    pub documents_total: usize,
    pub documents_with_text: usize,
    pub documents_without_text: usize,
    /// **Gerçekten aranan** sayfa sayısı: yalnız metin katmanı olan sayfalar.
    /// Metin katmanı olmayan sayfa bu sayıya girmez (`pages_without_text`).
    pub pages_scanned: usize,
    /// Kapsamın **güncel** sürümlerinde metin katmanı olmayan (aranamayan)
    /// sayfa sayısı. `> 0` ise alıntı yalnız bu sayfalarda geçiyor olabilir;
    /// `not_found` hükmü "uydurma" değildir.
    pub pages_without_text: usize,
    /// Taranan sürüm sayısı (güncel + eski).
    pub versions_scanned: usize,
    /// Bunlardan **eski** sürüm sayısı.
    pub historical_versions_scanned: usize,
    /// Yalnız eski sürümde bulunan alıntı sayısı.
    pub found_in_old_version: usize,
    /// Yalnız **tire ile bölünmüş kelime** birleştirilerek bulunan alıntı
    /// sayısı (kaynakta `kelime-\nsatır`). Birebir sayılmaz.
    pub dehyphenated: usize,
    /// Yalnız **bitişik yazı (ligatür)** açılarak bulunan alıntı sayısı
    /// (kaynakta `taraﬂar`, alıntıda `taraflar`). Birebir sayılmaz.
    pub ligature: usize,
    /// Yalnız **`…` ile kısaltılmış ya da başı/sonu kırpılmış** alıntıların
    /// gösterilen parçalarının sırayla bulunmasıyla doğrulanan alıntı sayısı
    /// (F12-T14p/T14q). Atlanan/kırpılan kısım denetlenmediği için birebir
    /// sayılmaz.
    pub elided: usize,
    pub notes: Vec<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchTier {
    Exact,
    Whitespace,
    /// Yalnız font kaynaklı **bitişik yazı** (`ﬂ`→`fl`, `ﬁ`→`fi`) açılır ve
    /// boşluklar teklenir; harfler korunur. PDF'ten gelen metinde `taraﬂar`
    /// yazarken avukat `taraflar` yazar — bu bir yazım farkı değildir.
    Ligature,
    /// **Kesme işareti** ailesi eşitlenir: `’` (U+2019) ve `ʼ` (U+02BC) ASCII
    /// `'` sayılır; harfler korunur (F12-T14r). Kaynak `Anayasa’nın` yazarken
    /// dilekçe `Anayasa'nın` yazabilir.
    Apostrophe,
    Fold,
    /// En zayıf katman: harf sadeleştirmesi + **satır/sayfa sonunda tire ile
    /// bölünmüş** kelimenin birleştirilmesi. Kaynakta `bedel-\nlini` yazıyorsa
    /// alıntı `bedellini` olarak bulunur; yalnız bu katmanda bulunan eşleşme
    /// "birebir değil" sayılır ve kaynağa bağlanmaz.
    Dehyphenated,
    /// `…`/`...` ile **kısaltılmış** alıntı (F12-T14p): parçalar kaynakta
    /// **aynı sırayla**, aralarına metin girmeden aranır. Beş taban
    /// normalizasyonunun her biri ayrı katmandır; raporlanan `matchKind`
    /// tek: `elided`.
    ElidedExact,
    ElidedWhitespace,
    ElidedLigature,
    ElidedFold,
    ElidedDehyphenated,
}

impl MatchTier {
    const fn kind(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Whitespace => "whitespace",
            Self::Ligature => "ligature",
            Self::Apostrophe => "apostrophe",
            Self::Fold => "turkish_fold",
            Self::Dehyphenated => "dehyphenated",
            Self::ElidedExact
            | Self::ElidedWhitespace
            | Self::ElidedLigature
            | Self::ElidedFold
            | Self::ElidedDehyphenated => "elided",
        }
    }

    /// Bu katman bir `elided` katmanı mı? `Some(base)`, parçaların hangi taban
    /// normalizasyonda aranacağını söyler; `None` tam alıntı katmanıdır.
    const fn elided_base(self) -> Option<QuoteTier> {
        match self {
            Self::ElidedExact => Some(QuoteTier::Exact),
            Self::ElidedWhitespace => Some(QuoteTier::Whitespace),
            Self::ElidedLigature => Some(QuoteTier::Ligature),
            Self::ElidedFold => Some(QuoteTier::TurkishFold),
            Self::ElidedDehyphenated => Some(QuoteTier::Dehyphenated),
            _ => None,
        }
    }
}

struct VersionScope {
    document_id: String,
    document_name: String,
    version_id: Option<String>,
    /// `document_versions.version_no`; metin yoksa 0.
    version_no: i64,
    /// Bu kapsam belgenin **güncel** sürümü mü?
    is_current: bool,
    parse_run_id: Option<String>,
}

/// Bir kapsamın sayfa sayımı: **aranabilir** (metin katmanı olan) ve
/// aranamayan sayfalar. `pages_scanned` yalnız `with_text`i sayar.
#[derive(Debug, Clone, Copy, Default)]
struct PageCounts {
    with_text: usize,
    without_text: usize,
}

impl PageCounts {
    fn add(&mut self, other: Self) {
        self.with_text = self.with_text.saturating_add(other.with_text);
        self.without_text = self.without_text.saturating_add(other.without_text);
    }

    fn total(self) -> usize {
        self.with_text.saturating_add(self.without_text)
    }
}

/// `resolve_historical_scopes` içinde belge başına sürüm toplama torbası.
struct HistoryBucket {
    document_id: String,
    document_name: String,
    /// Yeniden eskiye `(version_id, version_no)`.
    versions: Vec<(String, i64)>,
}

#[derive(Clone)]
struct RawHit {
    document_id: String,
    document_name: String,
    version_id: String,
    parse_run_id: String,
    page_id: Option<String>,
    segment_id: Option<String>,
    page_index: i64,
    char_start: i64,
    /// Eşleşmenin bittiği sayfa ve o sayfadaki bitiş konumu.
    end_page_index: i64,
    char_end: i64,
    spans_pages: bool,
    version_no: i64,
    is_current_version: bool,
    match_kind: &'static str,
    text_origin: String,
    source_text: String,
}

impl RawHit {
    fn position(&self) -> InTextQuotePosition {
        InTextQuotePosition {
            document_id: self.document_id.clone(),
            document_name: self.document_name.clone(),
            document_version_id: self.version_id.clone(),
            parse_run_id: self.parse_run_id.clone(),
            page_id: self.page_id.clone(),
            segment_id: self.segment_id.clone(),
            page_index: self.page_index,
            char_start: self.char_start,
            char_end: self.char_end,
            end_page_index: self.end_page_index,
            spans_pages: self.spans_pages,
            version_no: self.version_no,
            is_current_version: self.is_current_version,
            match_kind: self.match_kind.to_string(),
            text_origin: self.text_origin.clone(),
            source_text: self.source_text.clone(),
        }
    }
}

#[derive(Default)]
struct QuoteAccum {
    hits: Vec<RawHit>,
    /// **Ham** eşleşme sayısı; yalnız "bu alıntı bir şey buldu mu" kapısı
    /// (`pending_quotes`) içindir. Rapora yazılan sayı `resolve_version_copies`
    /// çıktısıdır — sürüm kopyaları orada birleştirilir.
    occurrences: i64,
    documents: BTreeSet<String>,
    /// (belge, sürüm) başına eşleşme sayısı.
    versions: Vec<VersionHit>,
}

/// Bir alıntının tek bir belge sürümündeki eşleşme sayısı.
struct VersionHit {
    document_id: String,
    version_id: String,
    version_no: i64,
    is_current: bool,
    count: i64,
}

/// Sürüm önceliği: güncel sürüm her zaman kazanır, yoksa en yüksek sürüm no.
fn version_rank(scope: &VersionHit) -> (bool, i64) {
    (scope.is_current, scope.version_no)
}

/// Bir alıntının sürüm kopyaları birleştirilmiş hâli.
struct ResolvedHits {
    occurrences: i64,
    hits: Vec<RawHit>,
    /// Elenen sürüm kopyası sayısı (dürüstlük notu için).
    dropped_versions: usize,
}

impl QuoteAccum {
    fn record(&mut self, hit: RawHit) {
        self.occurrences += 1;
        self.documents.insert(hit.document_id.clone());
        match self.versions.iter_mut().find(|scope| {
            scope.document_id == hit.document_id && scope.version_id == hit.version_id
        }) {
            Some(scope) => scope.count += 1,
            None => self.versions.push(VersionHit {
                document_id: hit.document_id.clone(),
                version_id: hit.version_id.clone(),
                version_no: hit.version_no,
                is_current: hit.is_current_version,
                count: 1,
            }),
        }
        if self.hits.len() < MAX_CANDIDATES {
            self.hits.push(hit);
        }
    }

    /// Aynı pasaj bir belgenin birden çok sürümünde geçebilir (düzeltilmemiş
    /// nüsha, uzun nüsha). Bunlar ayrı **konum** değildir: belge başına en yeni
    /// sürüm seçilir (güncel varsa güncel, yoksa en yüksek `version_no`) ve
    /// `occurrences` yalnız seçilen sürümün içindeki konumları sayar. Emsal
    /// doğrulaması (`verify_precedent_quote`) da tek sürümün sayısını verir;
    /// iki amiral gemisi aynı olguda aynı hükmü vermeli.
    fn resolve_version_copies(&self) -> ResolvedHits {
        let mut winners: Vec<&VersionHit> = Vec::new();
        for scope in &self.versions {
            match winners
                .iter_mut()
                .find(|winner| winner.document_id == scope.document_id)
            {
                Some(winner) => {
                    if version_rank(scope) > version_rank(winner) {
                        *winner = scope;
                    }
                }
                None => winners.push(scope),
            }
        }
        let occurrences = winners.iter().map(|winner| winner.count).sum();
        let mut hits: Vec<RawHit> = self
            .hits
            .iter()
            .filter(|hit| {
                winners.iter().any(|winner| {
                    winner.document_id == hit.document_id && winner.version_id == hit.version_id
                })
            })
            .cloned()
            .collect();
        if hits.is_empty() && occurrences > 0 {
            // Kuramsal: konum kapağı (8) dolduğu için kazanan sürümün konumu
            // düşmüş olabilir; boş konum listesi göstermektense ham liste verilir.
            hits = self.hits.clone();
        }
        ResolvedHits {
            occurrences,
            hits,
            dropped_versions: self.versions.len().saturating_sub(winners.len()),
        }
    }
}

/// Karar **tam metninde** metin içi alıntı denetimi girdisi (F12-T14j).
/// Serbest metindeki tırnaklı pasajlar çıkarılır; her biri seçili kararın
/// metinli sürümlerinde aranır.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentInTextInput {
    pub text: String,
}

/// Karar metninde bulunan tek konum. `source_text`, kaynak metnin o aralıktaki
/// **ham** hâlidir; normalize eşleşmede yazılan alıntıdan farklı olabilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentQuotePosition {
    pub version_id: String,
    /// Konum kararın **güncel** (en yeni metinli) sürümünde mi?
    pub is_current_version: bool,
    pub char_start: i64,
    pub char_end: i64,
    pub source_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentQuoteFinding {
    pub index: usize,
    pub quote: String,
    /// `exact` | `normalized` | `ambiguous` | `not_found` | `unverifiable`
    pub state: String,
    /// `not_found` | `no_full_text`
    pub reason: Option<String>,
    /// `exact` | `whitespace` | `ligature` | `apostrophe` | `turkish_fold` |
    /// `dehyphenated` | `elided`
    pub method: Option<String>,
    /// Eşleşen sürümün **içindeki** konum sayısı.
    pub occurrences: i64,
    /// Yalnız tek konum varsa dolu.
    pub position: Option<PrecedentQuotePosition>,
    /// Birden fazla konum varsa ilk adaylar (en çok 8).
    pub candidates: Vec<PrecedentQuotePosition>,
    /// Alıntı yalnız eski bir sürümde bulundu; güncel tam metinde geçmiyor.
    pub found_in_old_version: bool,
}

/// Karar tam metninde metin içi alıntı denetiminin raporu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecedentInTextCheck {
    pub precedent_id: String,
    pub text_chars: i64,
    pub spans_found: usize,
    pub skipped_short: usize,
    pub skipped_long: usize,
    pub unclosed: usize,
    pub quotes: Vec<PrecedentQuoteFinding>,
    pub exact: usize,
    pub normalized: usize,
    pub ambiguous: usize,
    pub not_found: usize,
    pub unverifiable: usize,
    pub dehyphenated: usize,
    pub ligature: usize,
    /// Yalnız **`…` ile kısaltılmış ya da başı/sonu kırpılmış** alıntıların
    /// gösterilen parçalarının sırayla bulunmasıyla doğrulanan alıntı sayısı
    /// (F12-T14p/T14q). Atlanan/kırpılan kısım denetlenmez.
    pub elided: usize,
    pub found_in_old_version: usize,
    /// Kararın metinli sürüm sayısı (güncel + sınır içindeki eski sürümler).
    pub versions_total: usize,
    /// En çok kaç sürüm tarandı (alıntı başına tepe).
    pub versions_scanned: usize,
    /// Bunlardan eski sürüm sayısı (tepe).
    pub historical_versions_scanned: usize,
    /// Daha eski sürümler sınır yüzünden taranmadı.
    pub historical_truncated: bool,
    pub notes: Vec<String>,
    pub checked_at: String,
}

/// Kütüphane geneli alıntı aramasında eşleşen tek emsal (F12-T14k).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQuoteMatch {
    pub precedent_id: String,
    pub court: String,
    pub chamber: Option<String>,
    pub docket_no: Option<String>,
    pub decision_no: Option<String>,
    pub decision_date: Option<String>,
    pub title: Option<String>,
    /// Eşleşmenin bulunduğu tam metin sürümü: güncel metin ya da (alıntı
    /// güncel metinde yoksa) emsalin taranan bir **eski** sürümü.
    pub version_id: String,
    /// `true` ise eşleşme yalnız **eski** sürümde bulundu; güncel metin
    /// değişmiş olabilir (F12-T14l).
    pub found_in_old_version: bool,
    /// `exact` | `whitespace` | `ligature` | `apostrophe` | `turkish_fold` |
    /// `dehyphenated`
    pub method: String,
    /// Eşleşmenin o sürüm içindeki konum sayısı (1'den fazlaysa `ambiguous`).
    pub occurrences: i64,
    pub char_start: i64,
    pub char_end: i64,
    /// Kaynak metnin o aralıktaki **ham** hâli.
    pub source_text: String,
}

/// Tek alıntının kütüphane geneli sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQuoteFinding {
    pub index: usize,
    pub quote: String,
    /// `found` — en az bir emsalde bulundu; `not_found` — taranan bütün
    /// emsallerde yok; `partial_scan` — kütüphane sınırı yüzünden tamamı
    /// taranmadı, `bulunamadı` kesin değil; `no_library_text` — kütüphanede
    /// tam metni olan emsal yok.
    pub state: String,
    /// Eşleşen emsaller (deterministik sırada, en çok
    /// `MAX_LIBRARY_MATCHES_PER_QUOTE`).
    pub matches: Vec<LibraryQuoteMatch>,
    /// Eşleşen emsal sayısı — liste sınırından bağımsız.
    pub matched_precedents: i64,
    /// Liste sınırı yüzünden gösterilmeyen eşleşme sayısı.
    pub hidden_matches: i64,
}

/// Kütüphane geneli metin içi alıntı aramasının raporu. Kalıcı yazma yoktur;
/// her arama kayıtlı tam metinlerden yeniden üretilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInTextCheck {
    pub text_chars: i64,
    pub spans_found: usize,
    pub skipped_short: usize,
    pub skipped_long: usize,
    pub unclosed: usize,
    pub quotes: Vec<LibraryQuoteFinding>,
    pub found: usize,
    pub not_found: usize,
    pub partial_scan: usize,
    pub no_library_text: usize,
    /// Kütüphanede tam metni olan emsal sayısı (sınır öncesi).
    pub library_total: usize,
    /// Gerçekten taranan emsal sayısı.
    pub scanned: usize,
    /// Kütüphane sınırı aşıldı: `bulunamadı` kesin değil.
    pub truncated: bool,
    /// Güncel metinde bulunamayan alıntılar için taranan **eski sürüm**
    /// metni sayısı (F12-T14l).
    pub history_scanned: usize,
    /// Eski sürüm taraması sınıra takıldı (emsal başına sürüm sayısı ya da
    /// toplam `MAX_LIBRARY_HISTORY_CHARS` bütçesi): eşleşmeyen alıntı
    /// `partial_scan` — `bulunamadı` kesin değil.
    pub history_truncated: bool,
    pub dehyphenated: usize,
    pub ligature: usize,
    /// Yalnız **`…` ile kısaltılmış ya da başı/sonu kırpılmış** alıntıların
    /// gösterilen parçalarının sırayla bulunmasıyla doğrulanan alıntı sayısı
    /// (F12-T14p/T14q). Atlanan/kırpılan kısım denetlenmez.
    pub elided: usize,
    pub notes: Vec<String>,
    pub checked_at: String,
}

impl Database {
    /// Serbest metindeki alıntıları dosyanın belgelerinde doğrular.
    ///
    /// Kapsam: `document_id` verilirse o belgenin güncel sürümü; verilmezse
    /// dosyadaki bütün belgelerin güncel sürümleri. Her sürümün **en yeni**
    /// başarılı/kısmi ayrıştırması taranır.
    ///
    /// Güncel sürümde bulunamayan alıntı için belgenin **eski sürümleri** de
    /// taranır (belge başına en çok `MAX_HISTORICAL_VERSIONS_PER_DOCUMENT`,
    /// yeniden yeniye). Eski sürümde bulunan konum `is_current_version=false`
    /// taşır; anchor sürümü kaydettiği için bağlanabilir, arayüz bunu yazar.
    pub fn check_in_text_quotes(
        &self,
        case_id: &str,
        input: &InTextQuoteInput,
    ) -> Result<InTextQuoteCheck, StorageError> {
        let text_chars = input.text.chars().count();
        if text_chars > MAX_IN_TEXT_CHECK_CHARS {
            return Err(StorageError::QuoteTextTooLong);
        }
        let extracted = extract_quoted_spans(&input.text);
        if extracted.quotes.len() > MAX_IN_TEXT_QUOTES {
            return Err(StorageError::QuoteTooMany);
        }
        if self.get_case(case_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let scopes = self.resolve_quote_scopes(case_id, input.document_id.as_deref())?;
        let documents_total = scopes.len();
        // "Metni var" = **en az bir aranabilir sayfa**. Ayrıştırma koşusu olup
        // bütün sayfaları metin katmanısız olan belge (OCR'sız tarama) metinli
        // sayılmaz: aksi hâlde "taranamadı" notu verilmez ve alıntılar
        // `not_found` ("Bulunamadı") hükmü alırdı.
        let mut documents_with_text = 0usize;
        for scope in &scopes {
            if self.page_counts(scope)?.with_text > 0 {
                documents_with_text += 1;
            }
        }
        let documents_without_text = documents_total - documents_with_text;

        let mut notes = vec![
            QUOTE_EXTRACTION_NOTE.to_string(),
            "Bulunamadı sonucu “uydurma” hükmü değildir: yalnız taranan metin katmanında \
             bulunmadığını söyler."
                .to_string(),
        ];
        if let Some(document_id) = input.document_id.as_deref() {
            let name = scopes
                .iter()
                .find(|scope| scope.document_id == document_id)
                .map(|scope| scope.document_name.clone())
                .unwrap_or_default();
            notes.push(format!(
                "Yalnız “{name}” belgesinin güncel sürümü tarandı; alıntı başka belgede geçebilir."
            ));
        }
        if documents_without_text > 0 {
            notes.push(format!(
                "{documents_without_text} belgenin güncel sürümünde ayrıştırılmış metin yok \
                 (bu belgeler taranamadı)."
            ));
        }

        let mut accum = (0..extracted.quotes.len())
            .map(|_| QuoteAccum::default())
            .collect::<Vec<_>>();
        // `pages_scanned` yalnız **gerçekten aranan** (metin katmanı olan)
        // sayfaları sayar; `pages_without_text` güncel sürümlerde aranamayan
        // sayfaları ayrı bildirir. `pages_total` maliyet kapısı içindir.
        let mut pages_scanned = 0usize;
        let mut pages_without_text = 0usize;
        let mut versions_scanned = 0usize;
        let mut historical_versions_scanned = 0usize;
        let mut historical_truncated_documents = 0usize;
        let mut historical_skipped_for_pages = false;
        if extracted.quotes.is_empty() {
            notes.push("Metinde tırnaklı alıntı bulunamadı.".to_string());
        } else {
            let max_pages = usize::try_from(MAX_SCOPE_PAGES).unwrap_or(usize::MAX);
            let current_pages = self.count_scope_pages(&scopes)?;
            pages_scanned = current_pages.with_text;
            pages_without_text = current_pages.without_text;
            let mut pages_total = current_pages.total();
            versions_scanned = scopes
                .iter()
                .filter(|scope| scope.parse_run_id.is_some())
                .count();
            if pages_total > max_pages {
                return Err(StorageError::QuoteScopeTooLarge);
            }
            // Eski sürüm kapsamları yalnız güncel sürümde bir şey bulunamazsa
            // çözülür ve taranır: sık durumda ek maliyet yok.
            let mut historical: Option<Vec<VersionScope>> = None;
            let mut active: Vec<usize> = (0..extracted.quotes.len()).collect();
            // Katman sırası korunur: üst katman bulduysa alt kata inilmez.
            // Her katmanda önce güncel sürümün tek sayfası, sonra sayfa sınırı,
            // ardından **eski sürümler** taranır; böylece eski sürümdeki birebir
            // eşleşme, güncel sürümdeki normalize eşleşmeden üstün kalır.
            for tier in [
                MatchTier::Exact,
                MatchTier::Whitespace,
                MatchTier::Ligature,
                MatchTier::Apostrophe,
                MatchTier::Fold,
                MatchTier::Dehyphenated,
                // `…` ile kısaltılmış alıntı en zayıf ailedir: parçaların
                // sırası ve örtüşmemesi bu katmanlarda zorlanır (T14p).
                MatchTier::ElidedExact,
                MatchTier::ElidedWhitespace,
                MatchTier::ElidedLigature,
                MatchTier::ElidedFold,
                MatchTier::ElidedDehyphenated,
            ] {
                if active.is_empty() {
                    break;
                }
                self.scan_quote_tier(&scopes, &extracted.quotes, &active, &mut accum, tier)?;
                active = pending_quotes(&accum);
                if active.is_empty() {
                    break;
                }
                self.scan_page_break_tier(&scopes, &extracted.quotes, &active, &mut accum, tier)?;
                active = pending_quotes(&accum);
                if active.is_empty() {
                    break;
                }
                if historical.is_none() {
                    let (historic, truncated) =
                        self.resolve_historical_scopes(case_id, input.document_id.as_deref())?;
                    historical_truncated_documents = truncated;
                    let extra = self.count_scope_pages(&historic)?;
                    if extra.total() > 0 && pages_total.saturating_add(extra.total()) > max_pages {
                        // Sessizce kesmek yerine hiç taranmaz ve nedeni yazılır.
                        historical_skipped_for_pages = true;
                        historical = Some(Vec::new());
                    } else {
                        pages_total = pages_total.saturating_add(extra.total());
                        // Eski sürümlerde aranamayan sayfalar güncel belgenin
                        // "metinsiz sayfa" sayısına eklenmez (not güncel
                        // sürümü anlatır); yalnız aranan sayfalar eklenir.
                        pages_scanned = pages_scanned.saturating_add(extra.with_text);
                        historical_versions_scanned = historic
                            .iter()
                            .filter(|scope| scope.parse_run_id.is_some())
                            .count();
                        versions_scanned =
                            versions_scanned.saturating_add(historical_versions_scanned);
                        historical = Some(historic);
                    }
                }
                let historic = historical.as_deref().unwrap_or(&[]);
                if !historic.is_empty() {
                    self.scan_quote_tier(historic, &extracted.quotes, &active, &mut accum, tier)?;
                    active = pending_quotes(&accum);
                    if !active.is_empty() {
                        self.scan_page_break_tier(
                            historic,
                            &extracted.quotes,
                            &active,
                            &mut accum,
                            tier,
                        )?;
                        active = pending_quotes(&accum);
                    }
                }
            }
        }
        if historical_versions_scanned > 0 {
            notes.push(format!(
                "Güncel sürümde bulunamayan alıntılar için belgelerin eski sürümleri de \
                 tarandı ({historical_versions_scanned} sürüm)."
            ));
        }
        if historical_truncated_documents > 0 {
            notes.push(format!(
                "{historical_truncated_documents} belgede en çok \
                 {MAX_HISTORICAL_VERSIONS_PER_DOCUMENT} eski sürüm tarandı; daha eski \
                 sürümler taranmadı."
            ));
        }
        if historical_skipped_for_pages {
            notes.push(format!(
                "Eski sürüm taraması {MAX_SCOPE_PAGES} sayfa sınırı nedeniyle yapılmadı; \
                 kapsamı belge seçerek daraltın."
            ));
        }

        let mut counts = [0usize; 5];
        let mut spanning = 0usize;
        let mut bindable = 0usize;
        let mut found_in_old_version = 0usize;
        let mut dehyphenated = 0usize;
        let mut ligature = 0usize;
        let mut elided = 0usize;
        let mut collapsed_version_copies = 0usize;
        let mut quotes = Vec::with_capacity(extracted.quotes.len());
        for (index, span) in extracted.quotes.iter().enumerate() {
            let found = &accum[index];
            let resolved = found.resolve_version_copies();
            if resolved.dropped_versions > 0 {
                collapsed_version_copies += 1;
            }
            let mut finding = InTextQuoteFinding {
                index,
                quote: span.text.clone(),
                state: "not_found".into(),
                reason: None,
                occurrences: resolved.occurrences,
                distinct_documents: i64::try_from(found.documents.len()).unwrap_or(i64::MAX),
                position: None,
                candidates: Vec::new(),
            };
            if pages_scanned == 0 {
                finding.state = "unverifiable".into();
                finding.reason = Some("no_text".into());
            } else if resolved.occurrences == 0 {
                finding.state = "not_found".into();
            } else if resolved.occurrences == 1 {
                let hit = resolved.hits.first().ok_or(StorageError::NotFound)?;
                finding.state = if hit.match_kind == MatchTier::Exact.kind() {
                    "exact".into()
                } else {
                    "normalized".into()
                };
                finding.position = Some(hit.position());
            } else {
                finding.state = "ambiguous".into();
                finding.candidates = resolved.hits.iter().map(RawHit::position).collect();
            }
            match finding.state.as_str() {
                "exact" => counts[0] += 1,
                "normalized" => counts[1] += 1,
                "ambiguous" => counts[2] += 1,
                "not_found" => counts[3] += 1,
                _ => counts[4] += 1,
            }
            let hit_spans = finding
                .position
                .as_ref()
                .is_some_and(|position| position.spans_pages)
                || finding
                    .candidates
                    .iter()
                    .any(|candidate| candidate.spans_pages);
            if hit_spans {
                spanning += 1;
            } else if finding.state == "exact" {
                // Tek sayfada birebir: tek sayfalık anchor'a bağlanabilir.
                bindable += 1;
            }
            // "Yalnız eski sürümde" hükmü konum yokken de (belirsiz eşleşme)
            // verilmeli: kullanıcı için en önemli olgu, alıntının **güncel**
            // metinde geçmediğidir.
            let only_old = !resolved.hits.is_empty()
                && resolved.hits.iter().all(|hit| !hit.is_current_version);
            if only_old {
                found_in_old_version += 1;
            }
            if finding
                .position
                .as_ref()
                .is_some_and(|position| position.match_kind == MatchTier::Dehyphenated.kind())
                || finding
                    .candidates
                    .iter()
                    .any(|candidate| candidate.match_kind == MatchTier::Dehyphenated.kind())
            {
                dehyphenated += 1;
            }
            if finding
                .position
                .as_ref()
                .is_some_and(|position| position.match_kind == MatchTier::Ligature.kind())
                || finding
                    .candidates
                    .iter()
                    .any(|candidate| candidate.match_kind == MatchTier::Ligature.kind())
            {
                ligature += 1;
            }
            if finding
                .position
                .as_ref()
                .is_some_and(|position| position.match_kind == "elided")
                || finding
                    .candidates
                    .iter()
                    .any(|candidate| candidate.match_kind == "elided")
            {
                elided += 1;
            }
            quotes.push(finding);
        }
        if dehyphenated > 0 {
            notes.push(format!(
                "{dehyphenated} alıntı yalnız **tire ile bölünmüş kelime** birleştirilerek \
                 bulundu (kaynakta `kelime-\\nsatır` gibi); birebir sayılmaz ve anchor'a \
                 bağlanmaz, kaynak metin gösterilir."
            ));
        }
        if ligature > 0 {
            notes.push(format!(
                "{ligature} alıntı yalnız **bitişik yazı (ligatür)** açılarak bulundu \
                 (kaynakta `taraﬂar` gibi tek glif, alıntıda `taraflar`); birebir sayılmaz \
                 ve anchor'a bağlanmaz, kaynak metin gösterilir."
            ));
        }
        if elided > 0 {
            notes.push(format!(
                "{elided} alıntı **`…` ile kısaltılmış ya da başı/sonu kırpılmış**: gösterilen \
                 parçalar kaynakta **sırasıyla** bulundu; atlanan/kırpılan kısım denetlenmedi, \
                 birebir sayılmaz ve anchor'a bağlanmaz — kaynak metin eşleşen aralık olarak \
                 gösterilir."
            ));
        }
        if found_in_old_version > 0 {
            notes.push(format!(
                "{found_in_old_version} alıntı yalnız **eski** bir sürümde bulundu; güncel \
                 sürümde geçmiyor (belge güncellenmiş olabilir). Kaynak, eşleşmenin bulunduğu \
                 sürüme bağlanır (belirsiz eşleşme bağlanmaz)."
            ));
        }
        if collapsed_version_copies > 0 {
            notes.push(format!(
                "{collapsed_version_copies} alıntı aynı belgenin birden çok sürümünde geçiyordu; \
                 sürüm kopyaları ayrı konum sayılmadı, **en yeni** sürümdeki konum kullanıldı."
            ));
        }
        if bindable > 0 {
            notes.push(format!(
                "{bindable} alıntı birebir tek konumda bulundu; kaynak bağlanabilir."
            ));
        }
        if spanning > 0 {
            notes.push(format!(
                "{spanning} alıntı yalnız sayfa sınırını aşarak bulundu (kaynakta sayfa \
                 sonu/satır sonu var); tek sayfalık anchor'a bağlanmaz, kaynak metin \
                 gösterilir."
            ));
        }
        if counts[1] > 0 {
            notes.push(
                "Birebir olmayan (boşluk veya harf farkı) eşleşmeler kaynak metniyle gösterilir; \
                 anchor'a bağlanmaz."
                    .to_string(),
            );
        }
        if counts[2] > 0 {
            notes.push(
                "Birden fazla konumda geçen alıntı tek kaynağa bağlanmaz; konumu kullanıcı seçmelidir."
                    .to_string(),
            );
        }
        if pages_without_text > 0 {
            notes.push(format!(
                "Güncel sürümlerde {pages_without_text} sayfanın metin katmanı yok \
                 (OCR'sız tarama); alıntı yalnız bu sayfalarda geçiyorsa bulunamaz — \
                 bu sayfalar taranmadı."
            ));
        }
        if pages_scanned == 0 && !extracted.quotes.is_empty() {
            notes.push(
                "Taranacak ayrıştırılmış metin bulunamadı; alıntılar bu denetimde \
                 doğrulanamadı."
                    .to_string(),
            );
        }

        let scope_document = input.document_id.as_deref().and_then(|id| {
            scopes
                .iter()
                .find(|scope| scope.document_id == id)
                .map(|scope| (scope.document_id.clone(), scope.document_name.clone()))
        });
        Ok(InTextQuoteCheck {
            case_id: case_id.to_string(),
            scope: if input.document_id.is_some() {
                "document".into()
            } else {
                "case".into()
            },
            scope_document_id: scope_document.as_ref().map(|(id, _)| id.clone()),
            scope_document_name: scope_document.map(|(_, name)| name),
            text_chars: i64::try_from(text_chars).unwrap_or(i64::MAX),
            spans_found: extracted.quotes.len(),
            skipped_short: extracted.skipped_short,
            skipped_long: extracted.skipped_long,
            unclosed: extracted.unclosed,
            quotes,
            exact: counts[0],
            normalized: counts[1],
            ambiguous: counts[2],
            not_found: counts[3],
            unverifiable: counts[4],
            documents_total,
            documents_with_text,
            documents_without_text,
            pages_scanned,
            pages_without_text,
            versions_scanned,
            historical_versions_scanned,
            found_in_old_version,
            dehyphenated,
            ligature,
            elided,
            notes,
            checked_at: now_iso(),
        })
    }

    fn resolve_quote_scopes(
        &self,
        case_id: &str,
        document_id: Option<&str>,
    ) -> Result<Vec<VersionScope>, StorageError> {
        let mut scopes = Vec::new();
        if let Some(document_id) = document_id {
            let row: Option<(String, String, Option<String>, i64)> = self
                .conn()
                .query_row(
                    "SELECT d.id,d.display_name,
                            (SELECT cv.version_id FROM current_document_versions cv
                              WHERE cv.document_id=d.id),
                            COALESCE((SELECT cv.version_no FROM current_document_versions cv
                                      WHERE cv.document_id=d.id),0)
                     FROM documents d
                     WHERE d.id=?1 AND d.case_id=?2 AND d.deleted_at IS NULL",
                    [document_id, case_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            let Some((id, name, version_id, version_no)) = row else {
                return Err(StorageError::NotFound);
            };
            let run_id = match version_id.as_deref() {
                Some(version) => self.latest_text_run(version)?,
                None => None,
            };
            scopes.push(VersionScope {
                document_id: id,
                document_name: name,
                version_id,
                version_no,
                is_current: true,
                parse_run_id: run_id,
            });
            return Ok(scopes);
        }
        let mut stmt = self.conn().prepare(
            "SELECT d.id,d.display_name,
                    (SELECT cv.version_id FROM current_document_versions cv
                      WHERE cv.document_id=d.id),
                    COALESCE((SELECT cv.version_no FROM current_document_versions cv
                              WHERE cv.document_id=d.id),0)
             FROM documents d
             WHERE d.case_id=?1 AND d.deleted_at IS NULL ORDER BY d.created_at DESC, d.id DESC",
        )?;
        let rows = stmt.query_map([case_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut docs = Vec::new();
        for row in rows {
            docs.push(row?);
        }
        for (id, name, version_id, version_no) in docs {
            let run_id = match version_id.as_deref() {
                Some(version) => self.latest_text_run(version)?,
                None => None,
            };
            scopes.push(VersionScope {
                document_id: id,
                document_name: name,
                version_id,
                version_no,
                is_current: true,
                parse_run_id: run_id,
            });
        }
        Ok(scopes)
    }

    /// Güncel sürümde bulunamayan alıntılar için **eski sürüm** kapsamları.
    ///
    /// Her belge için güncel sürüm dışındaki sürümler yeniden eskiye alınır ve
    /// `MAX_HISTORICAL_VERSIONS_PER_DOCUMENT` ile sınırlanır; sınırın ötesinde
    /// kalan sürüm varsa ikinci dönüş değeri (kesilen belge sayısı) artar.
    /// Metni olmayan sürümler de kapsama girer: `documents_without_text`
    /// sayımı güncel sürüm üzerinden yapıldığı için burada ayrıca sayılmaz.
    fn resolve_historical_scopes(
        &self,
        case_id: &str,
        document_id: Option<&str>,
    ) -> Result<(Vec<VersionScope>, usize), StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT v.document_id,d.display_name,v.id,v.version_no
             FROM document_versions v JOIN documents d ON d.id=v.document_id
             LEFT JOIN current_document_versions cv ON cv.document_id=d.id
             WHERE d.case_id=?1 AND d.deleted_at IS NULL
               AND (?2 IS NULL OR d.id=?2)
               AND (cv.version_id IS NULL OR v.id<>cv.version_id)
             ORDER BY v.document_id, v.version_no DESC, v.id DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![case_id, document_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut buckets: Vec<HistoryBucket> = Vec::new();
        for row in rows {
            let (doc_id, name, version_id, version_no) = row?;
            match buckets.last_mut() {
                Some(bucket) if bucket.document_id == doc_id => {
                    bucket.versions.push((version_id, version_no));
                }
                _ => buckets.push(HistoryBucket {
                    document_id: doc_id,
                    document_name: name,
                    versions: vec![(version_id, version_no)],
                }),
            }
        }
        let mut scopes = Vec::new();
        let mut truncated_documents = 0usize;
        for bucket in buckets {
            if bucket.versions.len() > MAX_HISTORICAL_VERSIONS_PER_DOCUMENT {
                truncated_documents += 1;
            }
            for (version_id, version_no) in bucket
                .versions
                .into_iter()
                .take(MAX_HISTORICAL_VERSIONS_PER_DOCUMENT)
            {
                let run_id = self.latest_text_run(&version_id)?;
                scopes.push(VersionScope {
                    document_id: bucket.document_id.clone(),
                    document_name: bucket.document_name.clone(),
                    version_id: Some(version_id),
                    version_no,
                    is_current: false,
                    parse_run_id: run_id,
                });
            }
        }
        Ok((scopes, truncated_documents))
    }

    fn latest_text_run(&self, version_id: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id FROM parse_runs
                 WHERE document_version_id=?1 AND status IN ('succeeded','partial')
                 ORDER BY COALESCE(completed_at, created_at) DESC, id DESC LIMIT 1",
                [version_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Bir ayrıştırma koşusunun sayfalarını **metinli / metinsiz** olarak
    /// sayar. "Metinli" ölçütü tarayıcının kullandığı [`PAGE_TEXT_EXPR`]dir:
    /// `pages_scanned` yalnız gerçekten aranan sayfaları sayar, metin katmanı
    /// olmayan sayfa "tarandı" görünmez.
    fn page_counts(&self, scope: &VersionScope) -> Result<PageCounts, StorageError> {
        let Some(run_id) = scope.parse_run_id.as_deref() else {
            return Ok(PageCounts::default());
        };
        let sql = format!(
            "SELECT \
             COALESCE(sum(CASE WHEN trim({PAGE_TEXT_EXPR}) <> '' THEN 1 ELSE 0 END),0),\
             COALESCE(sum(CASE WHEN trim({PAGE_TEXT_EXPR}) = '' THEN 1 ELSE 0 END),0) \
             FROM pages p WHERE p.parse_run_id=?1"
        );
        let (with_text, without_text): (i64, i64) =
            self.conn()
                .query_row(&sql, [run_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(PageCounts {
            with_text: usize::try_from(with_text).unwrap_or(usize::MAX),
            without_text: usize::try_from(without_text).unwrap_or(usize::MAX),
        })
    }

    fn count_scope_pages(&self, scopes: &[VersionScope]) -> Result<PageCounts, StorageError> {
        let mut total = PageCounts::default();
        for scope in scopes {
            total.add(self.page_counts(scope)?);
        }
        Ok(total)
    }

    /// Tek bir eşleşme katmanını tarar. `active` yalnız o kata bakılacak
    /// alıntı indeksleridir; üst katmanda bulunan alıntı alt kata inmez.
    fn scan_quote_tier(
        &self,
        scopes: &[VersionScope],
        spans: &[SpannedQuote],
        active: &[usize],
        accum: &mut [QuoteAccum],
        tier: MatchTier,
    ) -> Result<(), StorageError> {
        if active.is_empty() {
            return Ok(());
        }
        let needles: Vec<Option<MappedText>> = spans
            .iter()
            .map(|span| match tier {
                MatchTier::Exact => None,
                MatchTier::Whitespace => Some(collapse_ws_with_map(&span.text)),
                MatchTier::Ligature => Some(ligature_with_map(&span.text)),
                MatchTier::Apostrophe => Some(apostrophe_with_map(&span.text)),
                MatchTier::Fold => Some(turkish_fold_with_map(&span.text)),
                MatchTier::Dehyphenated => Some(dehyphenate_with_map(&span.text)),
                // `elided` katmanları parçalarla çalışır; tam alıntı iğnesi yok.
                MatchTier::ElidedExact
                | MatchTier::ElidedWhitespace
                | MatchTier::ElidedLigature
                | MatchTier::ElidedFold
                | MatchTier::ElidedDehyphenated => None,
            })
            .collect();
        // Kısaltılmış alıntı parçaları katman başına **bir kez** çözülür.
        let elided: Vec<Option<Vec<String>>> = spans
            .iter()
            .map(|span| elided_segments(&span.text))
            .collect();
        let needle_chars: Vec<usize> = needles
            .iter()
            .map(|needle| {
                needle
                    .as_ref()
                    .map(|mapped| mapped.text.chars().count())
                    .unwrap_or(0)
            })
            .collect();

        let connection = self.conn();
        let mut stmt = connection.prepare(&scope_page_sql())?;
        for scope in scopes {
            let Some(run_id) = scope.parse_run_id.as_deref() else {
                continue;
            };
            let rows = stmt.query_map([run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            for row in rows {
                let (page_id, page_index, text_origin, segment_id, text) = row?;
                if text.trim().is_empty() {
                    continue;
                }
                match tier {
                    MatchTier::Exact => {
                        for &quote_index in active {
                            let quote = &spans[quote_index].text;
                            for (byte, _) in text.match_indices(quote) {
                                let start = char_offset(&text, byte);
                                accum[quote_index].record(RawHit {
                                    document_id: scope.document_id.clone(),
                                    document_name: scope.document_name.clone(),
                                    version_id: scope.version_id.clone().unwrap_or_default(),
                                    parse_run_id: run_id.to_string(),
                                    page_id: Some(page_id.clone()),
                                    segment_id: segment_id.clone(),
                                    page_index,
                                    char_start: start,
                                    end_page_index: page_index,
                                    char_end: start
                                        + i64::try_from(quote.chars().count()).unwrap_or(i64::MAX),
                                    spans_pages: false,
                                    version_no: scope.version_no,
                                    is_current_version: scope.is_current,
                                    match_kind: tier.kind(),
                                    text_origin: text_origin.clone(),
                                    source_text: quote.clone(),
                                });
                            }
                        }
                    }
                    MatchTier::Whitespace
                    | MatchTier::Ligature
                    | MatchTier::Apostrophe
                    | MatchTier::Fold
                    | MatchTier::Dehyphenated => {
                        let mapped = match tier {
                            MatchTier::Whitespace => collapse_ws_with_map(&text),
                            MatchTier::Ligature => ligature_with_map(&text),
                            MatchTier::Apostrophe => apostrophe_with_map(&text),
                            MatchTier::Fold => turkish_fold_with_map(&text),
                            _ => dehyphenate_with_map(&text),
                        };
                        for &quote_index in active {
                            let Some(needle) = needles[quote_index].as_ref() else {
                                continue;
                            };
                            if needle.text.is_empty() || needle_chars[quote_index] == 0 {
                                continue;
                            }
                            let mut from = 0usize;
                            while let Some(offset) = mapped.text[from..].find(&needle.text) {
                                let at = from + offset;
                                if let Some((start, end)) =
                                    source_span(&mapped, at, needle_chars[quote_index])
                                {
                                    let source_text: String = text
                                        .chars()
                                        .skip(usize::try_from(start).unwrap_or(usize::MAX))
                                        .take(usize::try_from(end - start).unwrap_or(0))
                                        .collect();
                                    accum[quote_index].record(RawHit {
                                        document_id: scope.document_id.clone(),
                                        document_name: scope.document_name.clone(),
                                        version_id: scope.version_id.clone().unwrap_or_default(),
                                        parse_run_id: run_id.to_string(),
                                        page_id: Some(page_id.clone()),
                                        segment_id: segment_id.clone(),
                                        page_index,
                                        char_start: start,
                                        char_end: end,
                                        end_page_index: page_index,
                                        spans_pages: false,
                                        version_no: scope.version_no,
                                        is_current_version: scope.is_current,
                                        match_kind: tier.kind(),
                                        text_origin: text_origin.clone(),
                                        source_text,
                                    });
                                }
                                from = at + needle.text.len().max(1);
                            }
                        }
                    }
                    MatchTier::ElidedExact
                    | MatchTier::ElidedWhitespace
                    | MatchTier::ElidedLigature
                    | MatchTier::ElidedFold
                    | MatchTier::ElidedDehyphenated => {
                        let Some(base) = tier.elided_base() else {
                            continue;
                        };
                        for &quote_index in active {
                            let Some(segments) = elided[quote_index].as_deref() else {
                                continue;
                            };
                            // Tüm konumlar istenir: `occurrences` gerçek sayıyı
                            // taşımalı (belirsizlik hükmü bu sayıya dayanır).
                            let (positions, _) =
                                find_elided_sequences_in_text(&text, segments, base, usize::MAX);
                            for (start, length) in positions {
                                let source_text: String = text
                                    .chars()
                                    .skip(usize::try_from(start).unwrap_or(usize::MAX))
                                    .take(usize::try_from(length).unwrap_or(0))
                                    .collect();
                                accum[quote_index].record(RawHit {
                                    document_id: scope.document_id.clone(),
                                    document_name: scope.document_name.clone(),
                                    version_id: scope.version_id.clone().unwrap_or_default(),
                                    parse_run_id: run_id.to_string(),
                                    page_id: Some(page_id.clone()),
                                    segment_id: segment_id.clone(),
                                    page_index,
                                    char_start: start,
                                    end_page_index: page_index,
                                    char_end: start.saturating_add(length),
                                    spans_pages: false,
                                    version_no: scope.version_no,
                                    is_current_version: scope.is_current,
                                    match_kind: tier.kind(),
                                    text_origin: text_origin.clone(),
                                    source_text,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// **Sayfa sınırını aşan** eşleşmeleri arar (F12-T14 genişletmesi).
    ///
    /// Ardışık sayfalar tek pencerede birleştirilir (araya bir ayırıcı konur);
    /// yalnız iki sayfaya da gerçekten yayılan eşleşmeler kaydedilir, çünkü tek
    /// sayfa içinde kalanlar birinci taramada zaten bulunmuştur. Katman
    /// kuralları `scan_quote_tier` ile aynıdır; `spansPages=true` yazılır ve
    /// böyle bir konum tek sayfalık anchor'a bağlanamaz.
    fn scan_page_break_tier(
        &self,
        scopes: &[VersionScope],
        spans: &[SpannedQuote],
        active: &[usize],
        accum: &mut [QuoteAccum],
        tier: MatchTier,
    ) -> Result<(), StorageError> {
        if active.is_empty() {
            return Ok(());
        }
        let needles: Vec<Option<MappedText>> = spans
            .iter()
            .map(|span| match tier {
                MatchTier::Exact => None,
                MatchTier::Whitespace => Some(collapse_ws_with_map(&span.text)),
                MatchTier::Ligature => Some(ligature_with_map(&span.text)),
                MatchTier::Apostrophe => Some(apostrophe_with_map(&span.text)),
                MatchTier::Fold => Some(turkish_fold_with_map(&span.text)),
                MatchTier::Dehyphenated => Some(dehyphenate_with_map(&span.text)),
                // `elided` katmanları parçalarla çalışır; tam alıntı iğnesi yok.
                MatchTier::ElidedExact
                | MatchTier::ElidedWhitespace
                | MatchTier::ElidedLigature
                | MatchTier::ElidedFold
                | MatchTier::ElidedDehyphenated => None,
            })
            .collect();
        // Kısaltılmış alıntı parçaları katman başına **bir kez** çözülür.
        let elided: Vec<Option<Vec<String>>> = spans
            .iter()
            .map(|span| elided_segments(&span.text))
            .collect();
        let needle_chars: Vec<usize> = needles
            .iter()
            .map(|needle| {
                needle
                    .as_ref()
                    .map(|mapped| mapped.text.chars().count())
                    .unwrap_or(0)
            })
            .collect();

        // Taşınan kuyruk en uzun alıntıdan (+ayırıcılar) büyük olmalı; daha
        // uzun bir alıntı çok kısa sayfalar üzerinden üç sayfaya yayılırsa
        // pencere başlangıcı yetmeyebilir (dürüst sınır, README'de yazılı).
        let carry_limit = MAX_SPAN_CHARS + 64;
        let connection = self.conn();
        let mut stmt = connection.prepare(&scope_page_sql())?;
        for scope in scopes {
            let Some(run_id) = scope.parse_run_id.as_deref() else {
                continue;
            };
            let rows = stmt.query_map([run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            let mut carry: Vec<WindowChar> = Vec::new();
            let mut metas: Vec<PageMeta> = Vec::new();
            for row in rows {
                let (page_id, page_index, text_origin, segment_id, text) = row?;
                if text.trim().is_empty() {
                    continue;
                }
                let mut window: Vec<WindowChar> =
                    Vec::with_capacity(carry.len() + 1 + text.chars().count());
                window.extend(carry.iter().cloned());
                // Sayfa ayırıcısı **yalnız gerekiyorsa** eklenir. Worker her
                // satırın sonuna `\n` yazdığı için sayfa metni genelde satır
                // sonuyla biter; o durumda ikinci bir `\n` eklemek gerçek bir
                // sayfa sonu tiresini (`ve-` + `\n` + `\n` + `rilmiştir`)
                // "paragraf ayrımı" gibi gösterip birleştirmeyi engelliyordu
                // (ölçüldü: gerçek PDF, F12). Ayırıcı yoksa sınır, yeni
                // sayfanın **ilk karakterinin** indeksidir.
                if window
                    .last()
                    .is_some_and(|item| !matches!(item.ch, '\n' | '\r'))
                {
                    window.push(WindowChar {
                        page_index: None,
                        char_in_page: 0,
                        ch: '\n',
                    });
                }
                let boundary = window.len();
                for (index, ch) in text.chars().enumerate() {
                    window.push(WindowChar {
                        page_index: Some(page_index),
                        char_in_page: i64::try_from(index).unwrap_or(i64::MAX),
                        ch,
                    });
                }
                // Pencerede artık bulunmayan sayfaların üst verisi düşer;
                // böylece kuyruk sınırlı kalır (sayfa sayısıyla büyümez).
                metas.retain(|meta| {
                    window
                        .iter()
                        .any(|item| item.page_index == Some(meta.page_index))
                });
                metas.push(PageMeta {
                    page_index,
                    page_id: page_id.clone(),
                    segment_id: segment_id.clone(),
                    text_origin: text_origin.clone(),
                });

                let joined: String = window.iter().map(|item| item.ch).collect();
                match tier {
                    MatchTier::Exact => {
                        for &quote_index in active {
                            let quote = &spans[quote_index].text;
                            for (byte, _) in joined.match_indices(quote) {
                                let start = joined[..byte].chars().count();
                                let end = start + quote.chars().count();
                                if let Some(hit) = page_break_hit(
                                    scope,
                                    run_id,
                                    &metas,
                                    &window,
                                    boundary,
                                    start,
                                    end,
                                    tier.kind(),
                                ) {
                                    accum[quote_index].record(hit);
                                }
                            }
                        }
                    }
                    MatchTier::Whitespace
                    | MatchTier::Ligature
                    | MatchTier::Apostrophe
                    | MatchTier::Fold
                    | MatchTier::Dehyphenated => {
                        let mapped = match tier {
                            MatchTier::Whitespace => collapse_ws_with_map(&joined),
                            MatchTier::Ligature => ligature_with_map(&joined),
                            MatchTier::Apostrophe => apostrophe_with_map(&joined),
                            MatchTier::Fold => turkish_fold_with_map(&joined),
                            _ => dehyphenate_with_map(&joined),
                        };
                        for &quote_index in active {
                            let Some(needle) = needles[quote_index].as_ref() else {
                                continue;
                            };
                            if needle.text.is_empty() || needle_chars[quote_index] == 0 {
                                continue;
                            }
                            let mut from = 0usize;
                            while let Some(offset) = mapped.text[from..].find(&needle.text) {
                                let at = from + offset;
                                if let Some((start, end)) =
                                    source_span(&mapped, at, needle_chars[quote_index])
                                {
                                    if let Some(hit) = page_break_hit(
                                        scope,
                                        run_id,
                                        &metas,
                                        &window,
                                        boundary,
                                        usize::try_from(start).unwrap_or(usize::MAX),
                                        usize::try_from(end).unwrap_or(usize::MAX),
                                        tier.kind(),
                                    ) {
                                        accum[quote_index].record(hit);
                                    }
                                }
                                from = at + needle.text.len().max(1);
                            }
                        }
                    }
                    MatchTier::ElidedExact
                    | MatchTier::ElidedWhitespace
                    | MatchTier::ElidedLigature
                    | MatchTier::ElidedFold
                    | MatchTier::ElidedDehyphenated => {
                        let Some(base) = tier.elided_base() else {
                            continue;
                        };
                        for &quote_index in active {
                            let Some(segments) = elided[quote_index].as_deref() else {
                                continue;
                            };
                            let (positions, _) =
                                find_elided_sequences_in_text(&joined, segments, base, usize::MAX);
                            for (start, length) in positions {
                                let start = usize::try_from(start).unwrap_or(usize::MAX);
                                let end = start
                                    .saturating_add(usize::try_from(length).unwrap_or(usize::MAX));
                                if let Some(hit) = page_break_hit(
                                    scope,
                                    run_id,
                                    &metas,
                                    &window,
                                    boundary,
                                    start,
                                    end,
                                    tier.kind(),
                                ) {
                                    accum[quote_index].record(hit);
                                }
                            }
                        }
                    }
                }

                // Kuyruğu pencere sonundan sınırla taşı.
                let keep = window.len().min(carry_limit);
                carry = window.split_off(window.len() - keep);
            }
        }
        Ok(())
    }

    /// Serbest metindeki tırnaklı alıntıları **seçili kararın** tam metninde
    /// doğrular (F12-T14j).
    ///
    /// Belge tarafındaki [`Database::check_in_text_quotes`] dosya belgelerini
    /// tarar; bu yol aynı çıkarımı ve **aynı katman sırasını** (`exact` →
    /// `whitespace` → `ligature` → `apostrophe` → `turkish_fold` → `dehyphenated` →
    /// `elided`) emsal
    /// tam metnine uygular: dilekçedeki birden çok alıntı tek turda, tek tek
    /// kopyalamadan denetlenir. Kapsam bilinçli olarak **tek emsaldir**;
    /// kütüphane geneli arama değildir (F12-T10 kapsam ayrımı).
    ///
    /// Güncel sürümde bulunamayan alıntı için kararın **eski tam metin
    /// sürümleri** taranır (en çok `MAX_HISTORICAL_VERSIONS_PER_PRECEDENT`).
    /// `not_found` hükmü “uydurma” demek değildir; tam metin yoksa sonuç
    /// `unverifiable` / `no_full_text` olur.
    pub fn check_in_text_quotes_in_precedent(
        &self,
        precedent_id: &str,
        input: &PrecedentInTextInput,
    ) -> Result<PrecedentInTextCheck, StorageError> {
        if self.get_precedent(precedent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let text_chars = input.text.chars().count();
        if text_chars > MAX_IN_TEXT_CHECK_CHARS {
            return Err(StorageError::QuoteTextTooLong);
        }
        let extracted = extract_quoted_spans(&input.text);
        if extracted.quotes.len() > MAX_IN_TEXT_QUOTES {
            return Err(StorageError::QuoteTooMany);
        }
        let (scopes, historical_truncated) = self.quote_scopes(precedent_id)?;
        let mut notes = vec![
            QUOTE_EXTRACTION_NOTE.to_string(),
            "Bulunamadı sonucu “uydurma” hükmü değildir: yalnız kararın taranan tam \
             metninde bulunmadığını söyler."
                .to_string(),
        ];
        if scopes.is_empty() {
            notes.push(
                "Kararın tam metni yok; alıntılar doğrulanamadı. Önce karar tam \
                 metnini ekleyin."
                    .to_string(),
            );
        } else {
            notes.push(format!(
                "Kararın {} metinli sürümü tarandı (güncel + en çok \
                 {MAX_HISTORICAL_VERSIONS_PER_PRECEDENT} eski sürüm); katman sırası \
                 birebir → boşluk → ligatür → harf → tire birleştirme → “…” parçalıdır.",
                scopes.len()
            ));
        }
        if extracted.quotes.is_empty() {
            notes.push("Metinde tırnaklı alıntı bulunamadı.".to_string());
        }
        let mut quotes = Vec::with_capacity(extracted.quotes.len());
        let mut counts = [0usize; 5];
        let mut dehyphenated = 0usize;
        let mut ligature = 0usize;
        let mut elided = 0usize;
        let mut found_in_old_version = 0usize;
        let mut versions_scanned = 0usize;
        let mut historical_versions_scanned = 0usize;
        for (index, span) in extracted.quotes.iter().enumerate() {
            let mut finding = PrecedentQuoteFinding {
                index,
                quote: span.text.clone(),
                state: "not_found".into(),
                reason: None,
                method: None,
                occurrences: 0,
                position: None,
                candidates: Vec::new(),
                found_in_old_version: false,
            };
            if scopes.is_empty() {
                finding.state = "unverifiable".into();
                finding.reason = Some("no_full_text".into());
                counts[4] += 1;
                quotes.push(finding);
                continue;
            }
            let quote_chars = span.text.chars().count();
            match find_quote_in_scopes(&scopes, &span.text, quote_chars, MAX_CANDIDATES) {
                None => {
                    finding.state = "not_found".into();
                    finding.reason = Some("not_found".into());
                    counts[3] += 1;
                    versions_scanned = versions_scanned.max(scopes.len());
                    historical_versions_scanned =
                        historical_versions_scanned.max(scopes.len().saturating_sub(1));
                }
                Some(hit) => {
                    let scope = &scopes[hit.scope_index];
                    let positions: Vec<PrecedentQuotePosition> = hit
                        .positions
                        .iter()
                        .map(|(start, length)| PrecedentQuotePosition {
                            version_id: hit.version_id.clone(),
                            is_current_version: hit.is_current,
                            char_start: *start,
                            char_end: start.saturating_add(*length),
                            source_text: source_slice(&scope.text, *start, *length),
                        })
                        .collect();
                    finding.method = Some(hit.method.into());
                    finding.occurrences = hit.occurrences;
                    finding.found_in_old_version = !hit.is_current;
                    if hit.occurrences == 1 {
                        finding.state = if hit.method == "exact" {
                            "exact".into()
                        } else {
                            "normalized".into()
                        };
                        finding.position = positions.first().cloned();
                    } else {
                        finding.state = "ambiguous".into();
                        finding.candidates = positions;
                    }
                    versions_scanned = versions_scanned.max(hit.versions_scanned);
                    historical_versions_scanned =
                        historical_versions_scanned.max(hit.historical_versions_scanned);
                    if finding.found_in_old_version {
                        found_in_old_version += 1;
                    }
                    if hit.method == "dehyphenated" {
                        dehyphenated += 1;
                    }
                    if hit.method == "ligature" {
                        ligature += 1;
                    }
                    if hit.method == "elided" {
                        elided += 1;
                    }
                    match finding.state.as_str() {
                        "exact" => counts[0] += 1,
                        "normalized" => counts[1] += 1,
                        _ => counts[2] += 1,
                    }
                }
            }
            quotes.push(finding);
        }
        if dehyphenated > 0 {
            notes.push(format!(
                "{dehyphenated} alıntı yalnız **tire ile bölünmüş kelime** \
                 birleştirilerek bulundu (kaynakta `kelime-\\nsatır` gibi); birebir \
                 sayılmaz, kaynak metin gösterilir."
            ));
        }
        if ligature > 0 {
            notes.push(format!(
                "{ligature} alıntı yalnız **bitişik yazı (ligatür)** açılarak bulundu \
                 (kaynakta `taraﬂar` gibi tek glif, alıntıda `taraflar`); birebir \
                 sayılmaz, kaynak metin gösterilir."
            ));
        }
        if elided > 0 {
            notes.push(format!(
                "{elided} alıntı **`…` ile kısaltılmış ya da başı/sonu kırpılmış**: gösterilen \
                 parçalar karar metninde **sırasıyla** bulundu; atlanan/kırpılan kısım \
                 denetlenmedi ve birebir sayılmaz — kaynak metin eşleşen aralık olarak \
                 gösterilir."
            ));
        }
        if found_in_old_version > 0 {
            notes.push(format!(
                "{found_in_old_version} alıntı yalnız **eski bir sürümde** bulundu: \
                 güncel tam metinde geçmiyor, karar metni güncellenmiş olabilir."
            ));
        }
        if historical_truncated {
            notes.push(format!(
                "En çok {MAX_HISTORICAL_VERSIONS_PER_PRECEDENT} eski sürüm tarandı; \
                 daha eski sürümler taranmadı — `bulunamadı` sonucu bu durumda \
                 kesin değildir."
            ));
        }
        Ok(PrecedentInTextCheck {
            precedent_id: precedent_id.to_string(),
            text_chars: i64::try_from(text_chars).unwrap_or(i64::MAX),
            spans_found: extracted.quotes.len(),
            skipped_short: extracted.skipped_short,
            skipped_long: extracted.skipped_long,
            unclosed: extracted.unclosed,
            quotes,
            exact: counts[0],
            normalized: counts[1],
            ambiguous: counts[2],
            not_found: counts[3],
            unverifiable: counts[4],
            dehyphenated,
            ligature,
            elided,
            found_in_old_version,
            versions_total: scopes.len(),
            versions_scanned,
            historical_versions_scanned,
            historical_truncated,
            notes,
            checked_at: now_iso(),
        })
    }

    /// Serbest metindeki tırnaklı alıntıları **bütün kütüphanenin** tam
    /// metinlerinde arar (F12-T14k/T14l).
    ///
    /// Tek emsal yolundan ([`Database::check_in_text_quotes_in_precedent`])
    /// farkı kapsamdır: “bu pasaj hangi kararlarımda geçiyor?” sorusunu
    /// yanıtlar; katman sırası **aynıdır** (`exact` → `whitespace` → `ligature`
    /// → `turkish_fold` → `dehyphenated` → `elided`) ve aynı `find_quote_in_scopes`
    /// işlevini kullanır. Önce her emsalin **güncel** metni taranır; güncel
    /// metinde bulunamayan alıntı için emsallerin **eski sürümleri** de
    /// taranır (emsal başına en çok `MAX_HISTORICAL_VERSIONS_PER_PRECEDENT`,
    /// toplam `MAX_LIBRARY_HISTORY_CHARS` karakter bütçesi). Eski sürümde
    /// bulunan eşleşme `found_in_old_version=true` taşır; aynı emsal iki kez
    /// eşleşmez (güncel metinde bulunan emsalin geçmişi yeniden taranmaz).
    ///
    /// Kütüphane `MAX_LIBRARY_PRECEDENTS` ile sınırlıdır; sınır aşılırsa ya da
    /// eski sürüm bütçesi tükenirse rapor `truncated`/`history_truncated`
    /// yazar ve eşleşmeyen alıntı `partial_scan` olur — «bulunamadı» kesin
    /// sayılmaz. Kalıcı yazma yoktur.
    pub fn check_in_text_quotes_across_library(
        &self,
        input: &PrecedentInTextInput,
    ) -> Result<LibraryInTextCheck, StorageError> {
        let text_chars = input.text.chars().count();
        if text_chars > MAX_IN_TEXT_CHECK_CHARS {
            return Err(StorageError::QuoteTextTooLong);
        }
        let extracted = extract_quoted_spans(&input.text);
        if extracted.quotes.len() > MAX_IN_TEXT_QUOTES {
            return Err(StorageError::QuoteTooMany);
        }

        // Tam metni olan emsaller deterministik sırada: mahkeme, daire, kimlik.
        let mut meta_stmt = self.conn().prepare(
            "SELECT p.id,p.court,p.chamber,p.docket_no,p.decision_no,p.decision_date,p.title
             FROM precedents p
             WHERE EXISTS (SELECT 1 FROM precedent_versions v
                           WHERE v.precedent_id=p.id AND v.full_text IS NOT NULL
                             AND length(trim(v.full_text))>0)
             ORDER BY p.court, p.id",
        )?;
        let meta_rows = meta_stmt.query_map([], |row| {
            Ok(LibraryPrecedentMeta {
                id: row.get(0)?,
                court: row.get(1)?,
                chamber: row.get(2)?,
                docket_no: row.get(3)?,
                decision_no: row.get(4)?,
                decision_date: row.get(5)?,
                title: row.get(6)?,
            })
        })?;
        let mut metas: Vec<LibraryPrecedentMeta> = Vec::new();
        for row in meta_rows {
            metas.push(row?);
        }
        let library_total = metas.len();
        let truncated = library_total > MAX_LIBRARY_PRECEDENTS;
        metas.truncate(MAX_LIBRARY_PRECEDENTS);

        let mut version_stmt = self.conn().prepare(
            "SELECT id,full_text FROM precedent_versions
              WHERE precedent_id=?1 AND full_text IS NOT NULL AND length(trim(full_text))>0
              ORDER BY acquired_at DESC, id DESC LIMIT 1",
        )?;
        let mut scanned_metas: Vec<LibraryPrecedentMeta> = Vec::with_capacity(metas.len());
        let mut scopes: Vec<TextScope> = Vec::with_capacity(metas.len());
        for meta in metas {
            let current: Option<(String, String)> = version_stmt
                .query_row([&meta.id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .optional()?;
            if let Some((version_id, text)) = current {
                scanned_metas.push(meta);
                scopes.push(TextScope {
                    version_id,
                    is_current: true,
                    text,
                });
            }
        }
        let scanned = scopes.len();

        let mut notes = vec![
            QUOTE_EXTRACTION_NOTE.to_string(),
            format!(
                "Kütüphane araması önce **güncel** tam metinlerde arar; orada bulunamayan \
                 alıntı için emsallerin **eski sürümleri** de taranır (emsal başına en çok \
                 {MAX_HISTORICAL_VERSIONS_PER_PRECEDENT} sürüm, toplam \
                 {MAX_LIBRARY_HISTORY_CHARS} karakter bütçesi)."
            ),
            "Sonuç metin eşleşmesidir: alıntının anlamı ve kaynağı ayrıca incelenir; \
             `bulunamadı` “uydurma” hükmü değildir."
                .to_string(),
        ];
        if scopes.is_empty() {
            notes.push("Kütüphanede tam metni olan emsal yok; alıntılar aranamadı.".to_string());
        }
        if truncated {
            notes.push(format!(
                "Kütüphanede {library_total} emsalin tam metni var; yalnız ilk \
                 {MAX_LIBRARY_PRECEDENTS} tanesi tarandı — `bulunamadı` sonucu bu \
                 durumda kesin değildir."
            ));
        }
        if extracted.quotes.is_empty() {
            notes.push("Metinde tırnaklı alıntı bulunamadı.".to_string());
        }

        // Birinci tur: her emsalin **güncel** metni. Eşleşen emsallerin kimliği
        // tutulur; ikinci turda yalnız hiç eşleşmeyen alıntı ve yalnız henüz
        // eşleşmemiş emsallerin geçmişi taranır (aynı emsal iki kez eşleşmez).
        struct PendingFinding {
            index: usize,
            quote: String,
            matches: Vec<LibraryQuoteMatch>,
            matched: BTreeSet<usize>,
            matched_precedents: i64,
            hidden_matches: i64,
            is_dehyphenated: bool,
            is_ligature: bool,
            is_elided: bool,
        }

        let mut pending: Vec<PendingFinding> = Vec::with_capacity(extracted.quotes.len());
        for (index, span) in extracted.quotes.iter().enumerate() {
            let quote_chars = span.text.chars().count();
            let mut finding = PendingFinding {
                index,
                quote: span.text.clone(),
                matches: Vec::with_capacity(2),
                matched: BTreeSet::new(),
                matched_precedents: 0,
                hidden_matches: 0,
                is_dehyphenated: false,
                is_ligature: false,
                is_elided: false,
            };
            for (scope_index, scope) in scopes.iter().enumerate() {
                let hit =
                    find_quote_in_scopes(std::slice::from_ref(scope), &span.text, quote_chars, 1);
                let Some(hit) = hit else {
                    continue;
                };
                finding.matched.insert(scope_index);
                finding.matched_precedents += 1;
                if hit.method == "dehyphenated" {
                    finding.is_dehyphenated = true;
                }
                if hit.method == "ligature" {
                    finding.is_ligature = true;
                }
                if hit.method == "elided" {
                    finding.is_elided = true;
                }
                if finding.matches.len() >= MAX_LIBRARY_MATCHES_PER_QUOTE {
                    finding.hidden_matches += 1;
                    continue;
                }
                let (start, length) = hit.positions.first().copied().unwrap_or((0, 0));
                let meta = &scanned_metas[scope_index];
                finding.matches.push(LibraryQuoteMatch {
                    precedent_id: meta.id.clone(),
                    court: meta.court.clone(),
                    chamber: meta.chamber.clone(),
                    docket_no: meta.docket_no.clone(),
                    decision_no: meta.decision_no.clone(),
                    decision_date: meta.decision_date.clone(),
                    title: meta.title.clone(),
                    version_id: hit.version_id,
                    found_in_old_version: false,
                    method: hit.method.into(),
                    occurrences: hit.occurrences,
                    char_start: start,
                    char_end: start.saturating_add(length),
                    source_text: source_slice(&scope.text, start, length),
                });
            }
            pending.push(finding);
        }

        // İkinci tur (F12-T14l): güncel metinde bulunamayan alıntı için
        // emsallerin **eski sürümleri**. Yükleme tek sefer: emsal başına en çok
        // `MAX_HISTORICAL_VERSIONS_PER_PRECEDENT` metin, toplam
        // `MAX_LIBRARY_HISTORY_CHARS` karakter bütçesi; sınır aşılırsa
        // `history_truncated` — eşleşmeyen alıntı `partial_scan`.
        let unresolved: Vec<usize> = pending
            .iter()
            .enumerate()
            .filter(|(_, finding)| finding.matched.is_empty())
            .map(|(index, _)| index)
            .collect();
        let mut history_scopes: Vec<(usize, TextScope)> = Vec::new();
        let mut history_truncated = false;
        let mut old_matches = 0i64;
        if !unresolved.is_empty() && !scopes.is_empty() {
            let mut history_stmt = self.conn().prepare(
                "SELECT id,full_text FROM precedent_versions
                  WHERE precedent_id=?1 AND full_text IS NOT NULL AND length(trim(full_text))>0
                    AND id<>?2
                  ORDER BY acquired_at DESC, id DESC LIMIT ?3",
            )?;
            // LIMIT bir fazla: altıncı satır daha eski sürümlerin taranmadığını
            // kanıtlar (sessiz kesme yok).
            let history_limit =
                i64::try_from(MAX_HISTORICAL_VERSIONS_PER_PRECEDENT + 1).unwrap_or(i64::MAX);
            let mut history_chars = 0usize;
            'precedents: for (meta_index, meta) in scanned_metas.iter().enumerate() {
                let current_version_id = &scopes[meta_index].version_id;
                let mut rows = history_stmt.query(rusqlite::params![
                    &meta.id,
                    current_version_id,
                    history_limit
                ])?;
                let mut taken = 0usize;
                while let Some(row) = rows.next()? {
                    if taken == MAX_HISTORICAL_VERSIONS_PER_PRECEDENT {
                        history_truncated = true;
                        break;
                    }
                    let version_id: String = row.get(0)?;
                    let text: String = row.get(1)?;
                    let chars = text.chars().count();
                    if history_chars.saturating_add(chars) > MAX_LIBRARY_HISTORY_CHARS {
                        history_truncated = true;
                        break 'precedents;
                    }
                    history_chars += chars;
                    taken += 1;
                    history_scopes.push((
                        meta_index,
                        TextScope {
                            version_id,
                            is_current: false,
                            text,
                        },
                    ));
                }
            }
            for pending_index in &unresolved {
                let finding = &mut pending[*pending_index];
                let quote_chars = finding.quote.chars().count();
                for (meta_index, scope) in &history_scopes {
                    if finding.matched.contains(meta_index) {
                        continue;
                    }
                    let hit = find_quote_in_scopes(
                        std::slice::from_ref(scope),
                        &finding.quote,
                        quote_chars,
                        1,
                    );
                    let Some(hit) = hit else {
                        continue;
                    };
                    finding.matched.insert(*meta_index);
                    finding.matched_precedents += 1;
                    old_matches += 1;
                    if hit.method == "dehyphenated" {
                        finding.is_dehyphenated = true;
                    }
                    if hit.method == "ligature" {
                        finding.is_ligature = true;
                    }
                    if hit.method == "elided" {
                        finding.is_elided = true;
                    }
                    if finding.matches.len() >= MAX_LIBRARY_MATCHES_PER_QUOTE {
                        finding.hidden_matches += 1;
                        continue;
                    }
                    let (start, length) = hit.positions.first().copied().unwrap_or((0, 0));
                    let meta = &scanned_metas[*meta_index];
                    finding.matches.push(LibraryQuoteMatch {
                        precedent_id: meta.id.clone(),
                        court: meta.court.clone(),
                        chamber: meta.chamber.clone(),
                        docket_no: meta.docket_no.clone(),
                        decision_no: meta.decision_no.clone(),
                        decision_date: meta.decision_date.clone(),
                        title: meta.title.clone(),
                        version_id: hit.version_id,
                        found_in_old_version: true,
                        method: hit.method.into(),
                        occurrences: hit.occurrences,
                        char_start: start,
                        char_end: start.saturating_add(length),
                        source_text: source_slice(&scope.text, start, length),
                    });
                }
            }
        }
        if !history_scopes.is_empty() {
            notes.push(format!(
                "{} eski sürüm metni tarandı; {old_matches} eşleşme yalnız **eski sürümde** \
                 bulundu — güncel metin değişmiş olabilir, kaynağı gözle doğrulayın.",
                history_scopes.len()
            ));
        }
        if history_truncated {
            notes.push(
                "Eski sürüm taraması sınıra takıldı (emsal başına sürüm sayısı ya da toplam \
                 karakter bütçesi); `bulunamadı` sonucu bu durumda kesin değildir."
                    .to_string(),
            );
        }

        let mut findings = Vec::with_capacity(pending.len());
        let mut counts = [0usize; 4]; // found · not_found · partial_scan · no_library_text
        let mut dehyphenated = 0usize;
        let mut ligature = 0usize;
        let mut elided = 0usize;
        for finding in pending {
            let state = if scopes.is_empty() {
                "no_library_text"
            } else if finding.matched_precedents > 0 {
                "found"
            } else if truncated || history_truncated {
                "partial_scan"
            } else {
                "not_found"
            };
            match state {
                "found" => counts[0] += 1,
                "not_found" => counts[1] += 1,
                "partial_scan" => counts[2] += 1,
                _ => counts[3] += 1,
            }
            if finding.is_dehyphenated {
                dehyphenated += 1;
            }
            if finding.is_ligature {
                ligature += 1;
            }
            if finding.is_elided {
                elided += 1;
            }
            findings.push(LibraryQuoteFinding {
                index: finding.index,
                quote: finding.quote,
                state: state.into(),
                matches: finding.matches,
                matched_precedents: finding.matched_precedents,
                hidden_matches: finding.hidden_matches,
            });
        }
        if dehyphenated > 0 {
            notes.push(format!(
                "{dehyphenated} alıntı yalnız **tire ile bölünmüş kelime** birleştirilerek \
                 bulundu; birebir sayılmaz, kaynak metin gösterilir."
            ));
        }
        if ligature > 0 {
            notes.push(format!(
                "{ligature} alıntı yalnız **bitişik yazı (ligatür)** açılarak bulundu; \
                 birebir sayılmaz, kaynak metin gösterilir."
            ));
        }
        if elided > 0 {
            notes.push(format!(
                "{elided} alıntı **`…` ile kısaltılmış ya da başı/sonu kırpılmış**: gösterilen \
                 parçalar emsal metninde **sırasıyla** bulundu; atlanan/kırpılan kısım \
                 denetlenmedi ve birebir sayılmaz — kaynak metin eşleşen aralık olarak \
                 gösterilir."
            ));
        }
        Ok(LibraryInTextCheck {
            text_chars: i64::try_from(text_chars).unwrap_or(i64::MAX),
            spans_found: extracted.quotes.len(),
            skipped_short: extracted.skipped_short,
            skipped_long: extracted.skipped_long,
            unclosed: extracted.unclosed,
            quotes: findings,
            found: counts[0],
            not_found: counts[1],
            partial_scan: counts[2],
            no_library_text: counts[3],
            library_total,
            scanned,
            truncated,
            history_scanned: history_scopes.len(),
            history_truncated,
            dehyphenated,
            ligature,
            elided,
            notes,
            checked_at: now_iso(),
        })
    }
}

/// Kütüphane geneli aramada taranan tek emsalin künyesi (iç kullanım).
struct LibraryPrecedentMeta {
    id: String,
    court: String,
    chamber: Option<String>,
    docket_no: Option<String>,
    decision_no: Option<String>,
    decision_date: Option<String>,
    title: Option<String>,
}

/// Kaynak metnin karakter aralığını **ham** hâliyle döndürür (`source_text`).
fn source_slice(text: &str, start: i64, length: i64) -> String {
    let start = usize::try_from(start).unwrap_or(usize::MAX);
    let length = usize::try_from(length).unwrap_or(0);
    text.chars().skip(start).take(length).collect()
}

/// Pencerede taşınan tek karakter: hangi sayfanın kaçıncı karakteri olduğu.
/// `page_index` `None` ise araya konan sayfa ayırıcısıdır (hiçbir sayfaya ait
/// değildir, eşleşmenin ucu olamaz).
#[derive(Clone)]
struct WindowChar {
    page_index: Option<i64>,
    char_in_page: i64,
    ch: char,
}

/// Pencerede tutulan sayfanın kimlikleri: sınırı aşan eşleşmenin **ilk**
/// sayfasının `page_id`/`segment_id` değerleri gerekir.
struct PageMeta {
    page_index: i64,
    page_id: String,
    segment_id: Option<String>,
    text_origin: String,
}

/// Pencere içindeki eşleşmeyi sayfa sınırını aşan bir konuma çevirir.
///
/// `boundary`, yeni sayfanın **ilk karakterinin** indeksidir (pencereye ayırıcı
/// eklendiyse ayırıcıdan sonra). Yalnız **iki tarafında da** gerçek sayfa
/// karakteri bulunan eşleşmeler kabul edilir: `start` sınırdan önce, `end`
/// sınırı **aşmış** olmalı (`end > boundary`). Aksi hâlde eşleşme tek sayfa
/// içindedir ve birinci taramada bulunmuştur (yinelenmez). Ayırıcı karakterin
/// kendisi (hiçbir sayfaya ait değil) eşleşmenin ucu olamaz.
// Gerekçe: pencere bağlamı (kapsam, kimlik, sınır, eşleşme) tek çağrıda
// taşınıyor; ara yapı kurmak kuralı iki yere kopyalardı.
#[allow(clippy::too_many_arguments)]
fn page_break_hit(
    scope: &VersionScope,
    run_id: &str,
    metas: &[PageMeta],
    window: &[WindowChar],
    boundary: usize,
    start: usize,
    end: usize,
    match_kind: &'static str,
) -> Option<RawHit> {
    if start >= boundary || end <= boundary || end > window.len() {
        return None;
    }
    let first = window.get(start)?;
    let last = window.get(end - 1)?;
    let start_page = first.page_index?;
    let end_page = last.page_index?;
    if start_page >= end_page {
        return None;
    }
    let meta = metas.iter().find(|meta| meta.page_index == start_page)?;
    let source_text: String = window[start..end].iter().map(|item| item.ch).collect();
    Some(RawHit {
        document_id: scope.document_id.clone(),
        document_name: scope.document_name.clone(),
        version_id: scope.version_id.clone().unwrap_or_default(),
        parse_run_id: run_id.to_string(),
        page_id: Some(meta.page_id.clone()),
        segment_id: meta.segment_id.clone(),
        page_index: start_page,
        char_start: first.char_in_page,
        end_page_index: end_page,
        char_end: last.char_in_page.checked_add(1)?,
        spans_pages: true,
        version_no: scope.version_no,
        is_current_version: scope.is_current,
        match_kind,
        text_origin: meta.text_origin.clone(),
        source_text,
    })
}

fn pending_quotes(accum: &[QuoteAccum]) -> Vec<usize> {
    accum
        .iter()
        .enumerate()
        .filter(|(_, found)| found.occurrences == 0)
        .map(|(index, _)| index)
        .collect()
}

/// Sadeleştirilmiş metindeki eşleşmenin kaynak metindeki karakter aralığını
/// verir. `needle_chars`, eşleşen iğnenin **sadeleştirilmiş** karakter sayısıdır.
fn source_span(mapped: &MappedText, at_byte: usize, needle_chars: usize) -> Option<(i64, i64)> {
    if needle_chars == 0 {
        return None;
    }
    let start_index = mapped.text[..at_byte].chars().count();
    let end_index = start_index.checked_add(needle_chars)?;
    let first = *mapped.map.get(start_index)?;
    let last = *mapped.map.get(end_index - 1)?;
    Some((
        i64::try_from(first).ok()?,
        i64::try_from(last.checked_add(1)?).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::precedents::{normalize_with_map, NewPrecedent, NewPrecedentText};
    use crate::{DatabaseKey, NewAnchor, NewPage};

    fn db() -> Database {
        Database::open_in_memory(&DatabaseKey::new([21u8; 32])).expect("db")
    }

    fn new_case(db: &Database, title: &str) -> String {
        db.create_case(&NewCase {
            title: title.into(),
            case_kind: CaseKind::Review,
            reference_code: None,
            jurisdiction_label: None,
            practice_area: None,
            sensitivity: crate::Sensitivity::Normal,
            default_ai_policy: crate::AiPolicy::Off,
        })
        .expect("case")
        .id
    }

    fn page(index: i64, text: &str) -> NewPage {
        NewPage {
            index,
            text: text.into(),
            width: Some(595.0),
            height: Some(842.0),
            origin: "native".into(),
            warnings: Vec::new(),
            rotation: 0,
            coordinate_space: Some("unrotated_cropbox_points".into()),
            page_label: None,
            blocks: Vec::new(),
        }
    }

    /// Belge + sürüm + başarılı ayrıştırma + sayfalar. Dönüş: (belge, sürüm).
    fn add_document(db: &Database, case_id: &str, name: &str, pages: &[&str]) -> (String, String) {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &document.id,
                case_id,
                &blob,
                &"d".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let stored: Vec<NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, text)| page(i64::try_from(index).unwrap_or(i64::MAX), text))
            .collect();
        db.store_pages(&run.id, case_id, &stored).expect("pages");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("finish");
        (document.id, version.id)
    }

    /// Var olan belgeye yeni sürüm + başarılı ayrıştırma ekler (sürüm numarası
    /// veritabanında artar; `documents.current_version_id` yeni sürüme döner).
    fn add_version_with_pages(
        db: &Database,
        case_id: &str,
        document_id: &str,
        pages: &[&str],
    ) -> String {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let version = db
            .add_version(
                document_id,
                case_id,
                &blob,
                &"f".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let stored: Vec<NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, text)| page(i64::try_from(index).unwrap_or(i64::MAX), text))
            .collect();
        db.store_pages(&run.id, case_id, &stored).expect("pages");
        db.finish_parse_run(&run.id, "succeeded", &[])
            .expect("finish");
        version.id
    }

    fn add_empty_document(db: &Database, case_id: &str, name: &str) -> String {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        db.add_version(
            &document.id,
            case_id,
            &blob,
            &"e".repeat(64),
            10,
            "application/pdf",
        )
        .expect("version");
        document.id
    }

    fn check(
        db: &Database,
        case_id: &str,
        text: &str,
        document_id: Option<&str>,
    ) -> InTextQuoteCheck {
        db.check_in_text_quotes(
            case_id,
            &InTextQuoteInput {
                text: text.into(),
                document_id: document_id.map(str::to_string),
            },
        )
        .expect("check")
    }

    #[test]
    fn extracts_typographic_quotes_with_content_positions() {
        let out = extract_quoted_spans("Davacı “kira bedelini ödemediğini” söyledi.");
        assert_eq!(out.quotes.len(), 1);
        let quote = &out.quotes[0];
        assert_eq!(quote.text, "kira bedelini ödemediğini");
        assert_eq!(quote.style, "typographic");
        let chars: Vec<char> = "Davacı “kira bedelini ödemediğini” söyledi."
            .chars()
            .collect();
        let sliced: String = chars[quote.start..quote.end].iter().collect();
        assert_eq!(sliced, quote.text);
    }

    #[test]
    fn extracts_straight_guillemet_and_german_quotes() {
        let out = extract_quoted_spans(
            "Bir \"düz tırnak alıntısı\" ve «köşeli alıntı metni» ile „alman tipi alıntı“ var.",
        );
        assert_eq!(out.quotes.len(), 3);
        let styles: Vec<&str> = out.quotes.iter().map(|q| q.style).collect();
        assert_eq!(styles, vec!["straight", "guillemet", "german"]);
        assert_eq!(out.quotes[2].text, "alman tipi alıntı");
    }

    #[test]
    fn apostrophes_are_not_quote_delimiters() {
        let out = extract_quoted_spans("İstanbul'da “kira ödenmedi” denildi; avukat’ın notu.");
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(out.quotes[0].text, "kira ödenmedi");
    }

    #[test]
    fn nested_quotes_keep_the_outer_span() {
        let text = "Mahkeme “kararında «iyi niyet» ilkesine atıf yaptı” dedi.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.quotes.len(), 1);
        let quote = &out.quotes[0];
        assert_eq!(quote.text, "kararında «iyi niyet» ilkesine atıf yaptı");
        assert_eq!(quote.style, "typographic");
        let chars: Vec<char> = text.chars().collect();
        let sliced: String = chars[quote.start..quote.end].iter().collect();
        assert_eq!(sliced, quote.text);
    }

    #[test]
    fn an_outer_quote_left_open_by_a_closed_inner_quote_is_counted() {
        let text = "“dış alıntı “iç alıntı” kapanmadı";
        let out = extract_quoted_spans(text);
        assert!(out.quotes.is_empty());
        assert_eq!(out.unclosed, 1);
    }

    #[test]
    fn quote_content_positions_skip_the_whitespace_inside_the_marks() {
        let text = "Mahkeme “   kira bedelini ödemedi   ” dedi.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.quotes.len(), 1);
        let quote = &out.quotes[0];
        assert_eq!(quote.text, "kira bedelini ödemedi");
        let chars: Vec<char> = text.chars().collect();
        let sliced: String = chars[quote.start..quote.end].iter().collect();
        assert_eq!(sliced, "kira bedelini ödemedi");
    }

    #[test]
    fn straight_quotes_may_wrap_typographic_quotes() {
        let text = "Daire \"kararın “esas” bölümünü\" istedi.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(out.quotes[0].text, "kararın “esas” bölümünü");
        assert_eq!(out.quotes[0].style, "straight");
    }

    #[test]
    fn a_stray_opening_quote_does_not_hide_later_quotes() {
        let text = "“kapanmayan bir söz ve sonra «bulunan alıntı» gelir.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.unclosed, 1);
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(out.quotes[0].text, "bulunan alıntı");
        assert_eq!(out.quotes[0].style, "guillemet");
    }

    #[test]
    fn german_quote_closes_at_typographic_opener_and_next_quote_is_separate() {
        let text = "„alman tipi alıntı“ ve “türk tipi alıntı” yan yana.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.quotes.len(), 2);
        assert_eq!(out.quotes[0].text, "alman tipi alıntı");
        assert_eq!(out.quotes[0].style, "german");
        assert_eq!(out.quotes[1].text, "türk tipi alıntı");
        assert_eq!(out.quotes[1].style, "typographic");
    }

    #[test]
    fn short_unclosed_and_long_spans_are_counted_not_returned() {
        let short = extract_quoted_spans("“kısa” ve “az”.");
        assert!(short.quotes.is_empty());
        assert_eq!(short.skipped_short, 2);

        let unclosed = extract_quoted_spans("“kapanmayan uzun bir alıntı metni");
        assert!(unclosed.quotes.is_empty());
        assert_eq!(unclosed.unclosed, 1);

        let long_text = "a".repeat(MAX_SPAN_CHARS + 1);
        let long = extract_quoted_spans(&format!("“{long_text}”"));
        assert!(long.quotes.is_empty());
        assert_eq!(long.skipped_long, 1);
    }

    #[test]
    fn single_typographic_quotes_are_extracted_and_apostrophe_does_not_close() {
        let text = "Mahkeme ‘iyi niyet’ ilkesine atıf yaptı.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.quotes.len(), 1);
        let quote = &out.quotes[0];
        assert_eq!(quote.text, "iyi niyet");
        assert_eq!(quote.style, "single-typographic");
        let chars: Vec<char> = text.chars().collect();
        let sliced: String = chars[quote.start..quote.end].iter().collect();
        assert_eq!(sliced, quote.text);
        assert_eq!(out.unclosed, 0);
    }

    #[test]
    fn turkish_apostrophe_inside_a_single_quote_does_not_close_it() {
        // `‘İstanbul’da yaşanan olay’`: ilk `’` `da` ekiyle bitişiktir, kesme
        // işaretidir; alıntıyı kapatmaz. Son `’` (ardından boşluk) kapatır.
        let out = extract_quoted_spans("Metinde ‘İstanbul’da yaşanan olay’ anlatıldı.");
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(out.quotes[0].text, "İstanbul’da yaşanan olay");
        assert_eq!(out.quotes[0].style, "single-typographic");
        assert_eq!(out.unclosed, 0);
    }

    #[test]
    fn straight_single_quotes_open_only_at_word_boundaries() {
        let out = extract_quoted_spans("İstanbul'da 'kira bedeli' ödenmedi; avukat’ın notu.");
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(out.quotes[0].text, "kira bedeli");
        assert_eq!(out.quotes[0].style, "single-straight");
        assert_eq!(out.unclosed, 0);

        // Alıntının içindeki kesme işareti alıntıyı bölmez.
        let possessive = extract_quoted_spans("'Anayasa'nın 2. maddesi' açık hüküm getirir.");
        assert_eq!(possessive.quotes.len(), 1);
        assert_eq!(possessive.quotes[0].text, "Anayasa'nın 2. maddesi");
        assert_eq!(possessive.unclosed, 0);
    }

    #[test]
    fn measure_marks_do_not_open_a_quote() {
        // Ölçü işareti (`15"`) bir alıntı açmaz; iki ölçü işareti arasındaki
        // metin sahte bir alıntı olarak rapora düşmemeli.
        let out = extract_quoted_spans(r#"Ekran 15" ve disk 2" kapasiteli."#);
        assert!(
            out.quotes.is_empty(),
            "ölçü işareti alıntı üretmemeli: {:?}",
            out.quotes
                .iter()
                .map(|q| q.text.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(out.unclosed, 0);
    }

    #[test]
    fn typographic_measure_marks_do_not_pair_into_a_quote() {
        // `15“ … 2”` da ölçüdür; açılış sayılırsa aradaki metin sahte alıntı olur.
        let out = extract_quoted_spans("Ekran 15“ ve disk 2” kapasiteli.");
        assert!(
            out.quotes.is_empty(),
            "tipografik ölçü işareti alıntı üretmemeli: {:?}",
            out.quotes
                .iter()
                .map(|q| q.text.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(out.unclosed, 0);
    }

    #[test]
    fn a_quote_never_opens_in_the_middle_of_a_word() {
        let out = extract_quoted_spans(r#"karar"alıntı metni burada" dedi"#);
        assert!(out.quotes.is_empty());
        assert_eq!(out.unclosed, 0);
    }

    #[test]
    fn quotes_still_open_after_punctuation_and_space() {
        for text in [
            r#"("alıntı metni burada")"#,
            "2020 \"alıntı metni burada\"",
            "…“alıntı metni burada”",
            "— «alıntı metni burada»",
        ] {
            let out = extract_quoted_spans(text);
            assert_eq!(out.quotes.len(), 1, "metin: {text}");
            assert_eq!(out.quotes[0].text, "alıntı metni burada", "metin: {text}");
            assert_eq!(out.unclosed, 0, "metin: {text}");
        }
    }

    #[test]
    fn a_measure_mark_does_not_make_a_report_finding() {
        let db = db();
        let case_id = new_case(&db, "Ölçü işareti");
        let (_document, _version) = add_document(
            &db,
            &case_id,
            "karar.pdf",
            &[r#"Ekran 15" ve disk 2" kapasiteli. “Gerçek alıntı metni burada geçer.”"#],
        );
        let report = check(
            &db,
            &case_id,
            r#"Ekran 15" ve disk 2" kapasiteli. “Gerçek alıntı metni burada geçer”"#,
            None,
        );
        assert_eq!(
            report.spans_found,
            1,
            "yalnız gerçek alıntı sayılmalı: {:?}",
            report
                .quotes
                .iter()
                .map(|q| q.quote.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(report.quotes.len(), 1);
        assert_eq!(report.quotes[0].quote, "Gerçek alıntı metni burada geçer");
        assert_eq!(report.quotes[0].state, "exact");
    }

    // ---- F12-T14p: `…` ile kısaltılmış alıntı ----

    #[test]
    fn elided_segments_require_two_parts_but_allow_edge_clipping() {
        assert_eq!(
            elided_segments("sözleşmenin feshi … kiracının temerrüdü"),
            Some(vec![
                "sözleşmenin feshi".to_string(),
                "kiracının temerrüdü".to_string()
            ])
        );
        // Kenar kırpması (F12-T14q): gösterilen tek parça yeterli, eşik yine 8.
        assert_eq!(
            elided_segments("… kiracının temerrüdü"),
            Some(vec!["kiracının temerrüdü".to_string()])
        );
        assert_eq!(
            elided_segments("sözleşmenin feshi …"),
            Some(vec!["sözleşmenin feshi".to_string()])
        );
        assert_eq!(
            elided_segments("...kiracının temerrüdü…"),
            Some(vec!["kiracının temerrüdü".to_string()])
        );
        // Kısa parça tek başına güvenilir değildir — iç kısaltmada da, kırpmada da.
        assert!(elided_segments("Mahkeme … kiracının temerrüdü").is_none());
        assert!(elided_segments("Mahkeme …").is_none());
        assert!(elided_segments("… Mahkeme").is_none());
        // Yalnız işaretlerden oluşan alıntı kısaltılmış sayılmaz.
        assert!(elided_segments("…").is_none());
        // Üç nokta ve köşeli biçim de kısaltma işaretidir.
        assert_eq!(
            elided_segments("sözleşmenin feshi ... kiracının temerrüdü").map(|parts| parts.len()),
            Some(2)
        );
        assert_eq!(
            elided_segments("sözleşmenin feshi […] kiracının temerrüdü").map(|parts| parts.len()),
            Some(2)
        );
    }

    #[test]
    fn elided_quote_parts_are_found_in_order_and_reported_as_elided() {
        let db = db();
        let case_id = new_case(&db, "Kısaltılmış alıntı");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "Mahkeme sözleşmenin haklı nedenle feshedilebileceğine ve kiracının kira \
               bedelini ödemekle temerrüde düştüğüne karar verdi.",
            ],
        );
        let report = check(
            &db,
            &case_id,
            "“sözleşmenin haklı nedenle feshedilebileceğine … kiracının kira bedelini \
             ödemekle temerrüde düştüğüne” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(finding.occurrences, 1);
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "elided");
        // Kaynak metin **atlanan kısmıyla** gösterilir: birebir olmadığı görünür.
        assert!(position
            .source_text
            .starts_with("sözleşmenin haklı nedenle feshedilebileceğine"));
        assert!(position.source_text.contains("ve kiracının kira bedelini"));
        assert!(position.source_text.ends_with("temerrüde düştüğüne"));
        // Birebir olmayan eşleşme kaynağa bağlanmaz: çekirdek reddeder.
        // (Arayüz `canBind` yalnız birebir tek konumda açılır.)
        let error = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_start
                    + i64::try_from(finding.quote.chars().count()).unwrap_or(i64::MAX),
                exact_quote: finding.quote.clone(),
            })
            .expect_err("elided alıntı bağlanmamalı");
        assert_eq!(error.code(), "E_QUOTE_NOT_FOUND");
        assert_eq!(report.elided, 1);
        assert_eq!(report.normalized, 1);
        assert!(report.notes.iter().any(|n| n.contains("kısaltılmış")));
    }

    #[test]
    fn elided_parts_must_appear_in_the_source_in_order() {
        let db = db();
        let case_id = new_case(&db, "Sıra");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["ikinci parça burada geçiyor ve sonra birinci parça burada geçiyor."],
        );
        let report = check(
            &db,
            &case_id,
            "“birinci parça burada … ikinci parça burada” ifadesi.",
            None,
        );
        assert_eq!(report.quotes[0].state, "not_found");
        assert_eq!(report.elided, 0);

        // Aynı iki parça **doğru sırada** geçerse eşleşir (kontrol).
        let case_ok = new_case(&db, "Sıra doğru");
        add_document(
            &db,
            &case_ok,
            "Karar",
            &["birinci parça burada geçiyor ve sonra ikinci parça burada geçiyor."],
        );
        let report = check(
            &db,
            &case_ok,
            "“birinci parça burada … ikinci parça burada” ifadesi.",
            None,
        );
        assert_eq!(report.quotes[0].state, "normalized");
        assert_eq!(report.elided, 1);
    }

    #[test]
    fn elided_quote_with_a_short_part_is_not_matched() {
        let db = db();
        let case_id = new_case(&db, "Kısa parça");
        add_document(&db, &case_id, "Karar", &["Mahkeme fesih kararını verdi."]);
        // "Mahkeme" (7) eşik altı: parça tek başına her kararda geçebilir.
        let report = check(&db, &case_id, "“Mahkeme … fesih kararını” ifadesi.", None);
        assert_eq!(report.quotes[0].state, "not_found");
        assert_eq!(report.elided, 0);
    }

    #[test]
    fn elided_parts_use_the_whitespace_layer() {
        let db = db();
        let case_id = new_case(&db, "Boşluk");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "Mahkeme sözleşmenin\nhaklı nedenle feshedilebileceğine ve kiracının \
               temerrüde düştüğüne karar verdi.",
            ],
        );
        let report = check(
            &db,
            &case_id,
            "“sözleşmenin haklı nedenle feshedilebileceğine … kiracının temerrüde düştüğüne” \
             ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "elided");
        assert!(
            position.source_text.contains('\n'),
            "kaynak ham hâli gösterilmeli: {:?}",
            position.source_text
        );
    }

    #[test]
    fn elided_parts_across_a_page_break_are_marked_spanning() {
        let db = db();
        let case_id = new_case(&db, "Sayfa sınırı");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "Mahkeme sözleşmenin haklı nedenle feshedilebileceğine",
                "ve kiracının temerrüde düştüğüne karar verdi.",
            ],
        );
        let report = check(
            &db,
            &case_id,
            "“sözleşmenin haklı nedenle feshedilebileceğine … kiracının temerrüde düştüğüne” \
             ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "elided");
        assert!(position.spans_pages);
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
    }

    #[test]
    fn repeated_elided_passage_is_ambiguous_with_elided_candidates() {
        let db = db();
        let case_id = new_case(&db, "Tekrar");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "sözleşmenin haklı nedenle feshedilebileceğine ve kiracının temerrüde \
               düştüğüne … sözleşmenin haklı nedenle feshedilebileceğine ve kiracının \
               temerrüde düştüğüne",
            ],
        );
        let report = check(
            &db,
            &case_id,
            "“sözleşmenin haklı nedenle feshedilebileceğine … kiracının temerrüde düştüğüne” \
             ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 2);
        assert!(finding
            .candidates
            .iter()
            .all(|candidate| candidate.match_kind == "elided"));
        assert_eq!(report.elided, 1);
    }

    #[test]
    fn a_literal_ellipsis_in_the_source_stays_exact() {
        let db = db();
        let case_id = new_case(&db, "Gerçek üç nokta");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["Karar metni … burada aynen geçiyor."],
        );
        let report = check(
            &db,
            &case_id,
            "“Karar metni … burada aynen geçiyor” ifadesi.",
            None,
        );
        assert_eq!(report.quotes[0].state, "exact");
        assert_eq!(report.elided, 0);
    }

    // ---- F12-T14q: başı/sonu `…` ile kırpılmış alıntı ----

    #[test]
    fn edge_clipped_quote_is_found_as_elided_and_never_bound() {
        let db = db();
        let case_id = new_case(&db, "Kırpık alıntı");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["Mahkeme sözleşmenin haklı nedenle feshedilebileceğine karar verdi."],
        );
        // `...` başta, `…` sonda: alıntı iki uçtan kırpılmış, gösterilen kısım bitişik.
        let report = check(
            &db,
            &case_id,
            "“...sözleşmenin haklı nedenle feshedilebileceğine…” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(finding.occurrences, 1);
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "elided");
        assert_eq!(
            position.source_text,
            "sözleşmenin haklı nedenle feshedilebileceğine"
        );
        assert!(!position.spans_pages);
        assert_eq!(report.elided, 1);
        assert_eq!(report.normalized, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("başı/sonu kırpılmış")));
        // Kırpık alıntı da kaynağa bağlanmaz: `…` taşıyan alıntı birebir bulunamaz.
        let error = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_start
                    + i64::try_from(finding.quote.chars().count()).unwrap_or(i64::MAX),
                exact_quote: finding.quote.clone(),
            })
            .expect_err("kırpık alıntı bağlanmamalı");
        assert_eq!(error.code(), "E_QUOTE_NOT_FOUND");
    }

    #[test]
    fn edge_clipping_is_recognized_on_either_end() {
        let db = db();
        let case_id = new_case(&db, "Kırpık iki uç");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "Mahkeme sözleşmenin haklı nedenle feshedilebileceğine karar verdi. Ayrıca \
                 kiracının temerrüde düştüğünü belirtti.",
            ],
        );
        let report = check(
            &db,
            &case_id,
            "“...sözleşmenin haklı nedenle feshedilebileceğine” ve “kiracının temerrüde \
             düştüğünü…” deniyor.",
            None,
        );
        assert_eq!(report.quotes.len(), 2);
        for finding in &report.quotes {
            assert_eq!(finding.state, "normalized");
            assert_eq!(
                finding.position.as_ref().expect("position").match_kind,
                "elided"
            );
        }
        assert_eq!(report.elided, 2);
    }

    #[test]
    fn edge_clipped_quote_with_a_short_remainder_is_not_found() {
        let db = db();
        let case_id = new_case(&db, "Kırpık kısa");
        add_document(&db, &case_id, "Karar", &["Mahkeme fesih kararını verdi."]);
        // "Mahkeme" (7) eşik altı: kırpık alıntı da tek kısa parçayla eşleşmez.
        let report = check(&db, &case_id, "“Mahkeme…” ifadesi.", None);
        assert_eq!(report.quotes[0].state, "not_found");
        assert_eq!(report.elided, 0);
    }

    #[test]
    fn repeated_edge_clipped_passage_is_ambiguous() {
        let db = db();
        let case_id = new_case(&db, "Kırpık tekrar");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "sözleşmenin haklı nedenle feshedilebileceğine burada geçiyor ve \
                 sözleşmenin haklı nedenle feshedilebileceğine tekrar geçiyor.",
            ],
        );
        let report = check(
            &db,
            &case_id,
            "“…sözleşmenin haklı nedenle feshedilebileceğine…” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 2);
        assert!(finding
            .candidates
            .iter()
            .all(|candidate| candidate.match_kind == "elided"));
        assert_eq!(report.elided, 1);
    }

    #[test]
    fn single_quotes_nest_inside_double_quotes_without_breaking_the_outer_span() {
        let text = "Mahkeme “kararında ‘iyi niyet’ ilkesine atıf yaptı” dedi.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(
            out.quotes[0].text,
            "kararında ‘iyi niyet’ ilkesine atıf yaptı"
        );
        assert_eq!(out.quotes[0].style, "typographic");
        assert_eq!(out.unclosed, 0);
    }

    #[test]
    fn unclosed_single_quote_does_not_hide_later_quotes() {
        let text = "‘kapanmayan tek tırnak ve sonra “sağlam alıntı metni” gelir.";
        let out = extract_quoted_spans(text);
        assert_eq!(out.quotes.len(), 1);
        assert_eq!(out.quotes[0].text, "sağlam alıntı metni");
        assert_eq!(out.unclosed, 1);
    }

    #[test]
    fn a_single_quote_whose_only_closer_carries_a_suffix_is_not_extracted() {
        // Tek `’` eke bitişik (`‘yeterli sebep’te`): kesme işareti sayılır,
        // kapanış yoktur. Alıntı **uydurulmaz**; `unclosed` dürüstçe sayılır.
        let out = extract_quoted_spans("Kararda ‘yeterli sebep’te ısrar edildi.");
        assert!(out.quotes.is_empty());
        assert_eq!(out.unclosed, 1);

        // Aynı açılış, kapanışı olmadığı için yığına **girmez**: sonraki
        // sağlam alıntı yine çıkarılır (sessiz yutma yok).
        let mixed = extract_quoted_spans("‘yeterli sebep’te ve “sağlam alıntı metni” geçer.");
        assert_eq!(mixed.quotes.len(), 1);
        assert_eq!(mixed.quotes[0].text, "sağlam alıntı metni");
        assert_eq!(mixed.unclosed, 1);
    }

    #[test]
    fn apostrophes_alone_do_not_open_or_close_quotes() {
        let out = extract_quoted_spans("İstanbul'da ve Anayasa’nın 2. maddesi; avukat’ın notu.");
        assert!(out.quotes.is_empty());
        assert_eq!(out.unclosed, 0);
        assert_eq!(out.skipped_short, 0);
    }

    #[test]
    fn single_typographic_quote_in_text_is_found_in_the_document() {
        let db = db();
        let case_id = new_case(&db, "İyi niyet");
        add_document(
            &db,
            &case_id,
            "Dilekçe",
            &["Mahkeme iyi niyet ilkesine açıkça atıf yaptı."],
        );
        let report = check(
            &db,
            &case_id,
            "Davacı ‘iyi niyet ilkesine’ dayanmaktadır.",
            None,
        );
        assert_eq!(report.quotes.len(), 1);
        assert_eq!(report.quotes[0].state, "exact");
        assert_eq!(report.exact, 1);
    }

    #[test]
    fn exact_quote_is_found_once_and_position_binds_an_anchor() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        let (_, version_id) = add_document(
            &db,
            &case_id,
            "Dava dilekçesi",
            &[
                "Mahkeme, kiracının kira bedelini ödemediğini tespit etti.",
                "İkinci sayfa başka bir metindir.",
            ],
        );
        let text = "Davacı “kira bedelini ödemediğini tespit etti” iddiasındadır.";
        let report = check(&db, &case_id, text, None);
        assert_eq!(report.spans_found, 1);
        assert_eq!(report.exact, 1);
        assert_eq!(report.ambiguous, 0);
        assert_eq!(report.pages_scanned, 2);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "exact");
        assert_eq!(finding.occurrences, 1);
        assert_eq!(finding.distinct_documents, 1);
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.document_version_id, version_id);
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 0);
        assert!(!position.spans_pages);
        assert_eq!(position.match_kind, "exact");
        assert_eq!(
            position.source_text,
            "kira bedelini ödemediğini tespit etti"
        );
        assert_eq!(position.char_start, 19);

        // Konum doğrudan anchor'a bağlanabilir: aynı doğrulama yeniden geçer.
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_end,
                exact_quote: position.source_text.clone(),
            })
            .expect("anchor");
        assert_eq!(anchor.exact_quote, finding.quote);
        assert_eq!(anchor.start_offset, 19);
        assert_eq!(anchor.end_offset, 19 + 37);
        let location = db.locate_anchor(&anchor.id).expect("locate").expect("row");
        assert!(location.quote_intact);
    }

    #[test]
    fn quote_in_two_documents_is_ambiguous_with_distinct_documents() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Dilekçe",
            &["Kiracı kira bedelini ödemedi ve temerrüde düştü."],
        );
        add_document(
            &db,
            &case_id,
            "Bilirkişi raporu",
            &["Kiracı kira bedelini ödemedi; bu nedenle tahliye istendi."],
        );
        let report = check(
            &db,
            &case_id,
            "“kira bedelini ödemedi” ifadesi geçiyor.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 2);
        assert_eq!(finding.distinct_documents, 2);
        assert!(finding.position.is_none());
        assert_eq!(finding.candidates.len(), 2);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("Birden fazla konumda")));
    }

    #[test]
    fn repeated_quote_on_one_page_is_ambiguous() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Tutanak",
            &["Kira bedeli ödenmedi. Kira bedeli ödenmedi. Kira bedeli ödenmedi."],
        );
        let report = check(&db, &case_id, "“kira bedeli ödenmedi” yazıyor.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 3);
        assert_eq!(finding.distinct_documents, 1);
        assert_eq!(finding.candidates.len(), 3);
    }

    #[test]
    fn whitespace_only_difference_is_reported_as_normalized_with_source_text() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["Mahkeme ödeme   yapılmadığını ve temerrüdün oluştuğunu tespit etti."],
        );
        let report = check(&db, &case_id, "“ödeme yapılmadığını” tespiti.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "whitespace");
        assert_eq!(position.source_text, "ödeme   yapılmadığını");
        assert_eq!(
            normalize_with_map(&position.source_text).text,
            normalize_with_map(&finding.quote).text
        );
        // Birebir olmayan eşleşme anchor'a bağlanmaz: çekirdek reddeder.
        let error = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_start
                    + i64::try_from(finding.quote.chars().count()).unwrap_or(i64::MAX),
                exact_quote: finding.quote.clone(),
            })
            .expect_err("normalized quote must not bind");
        assert_eq!(error.code(), "E_QUOTE_NOT_FOUND");
    }

    #[test]
    fn turkish_fold_is_a_separate_weaker_tier() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Karar", &["ÖDEME YAPILMADIĞI sabittir."]);
        let report = check(&db, &case_id, "“ödeme yapılmadığı” sabittir.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "turkish_fold");
        assert_eq!(position.source_text, "ÖDEME YAPILMADIĞI");
    }

    #[test]
    fn missing_quote_is_not_found_without_position() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Dilekçe", &["Kiracı kira bedelini ödedi."]);
        let report = check(
            &db,
            &case_id,
            "“tamamen uydurma bir pasaj burada” deniyor.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "not_found");
        assert_eq!(finding.occurrences, 0);
        assert!(finding.position.is_none());
        assert_eq!(report.not_found, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("“uydurma” hükmü değildir")));
    }

    #[test]
    fn document_scope_excludes_other_documents_and_says_so() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Dilekçe",
            &["Kiracı kira bedelini ödemediğini beyan etti."],
        );
        let (other_document, _) = add_document(
            &db,
            &case_id,
            "Rapor",
            &["Raporda ödeme yapıldığı belirtilmiştir."],
        );
        let report = check(
            &db,
            &case_id,
            "“kira bedelini ödemediğini” beyanı.",
            Some(&other_document),
        );
        assert_eq!(report.scope, "document");
        assert_eq!(
            report.scope_document_id.as_deref(),
            Some(other_document.as_str())
        );
        assert_eq!(report.quotes[0].state, "not_found");
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("Yalnız “Rapor” belgesinin güncel sürümü tarandı")));
    }

    #[test]
    fn document_from_another_case_is_rejected() {
        let db = db();
        let case_a = new_case(&db, "A");
        let case_b = new_case(&db, "B");
        let (document, _) = add_document(&db, &case_a, "Belge", &["metin"]);

        let error = db
            .check_in_text_quotes(
                &case_b,
                &InTextQuoteInput {
                    text: "“uzun bir alıntı metni” var.".into(),
                    document_id: Some(document),
                },
            )
            .expect_err("foreign document");
        assert_eq!(error.code(), "E_NOT_FOUND");
    }

    #[test]
    fn document_without_parse_is_counted_and_scan_still_works() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Dilekçe",
            &["Kiracı kira bedelini ödemedi ve temerrüt oluştu."],
        );
        add_empty_document(&db, &case_id, "Taranmamış belge");
        let report = check(&db, &case_id, "“kira bedelini ödemedi” ifadesi.", None);
        assert_eq!(report.documents_total, 2);
        assert_eq!(report.documents_with_text, 1);
        assert_eq!(report.documents_without_text, 1);
        assert_eq!(report.quotes[0].state, "exact");
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("1 belgenin güncel sürümünde ayrıştırılmış metin yok")));
    }

    #[test]
    fn no_text_scope_is_unverifiable_not_not_found() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_empty_document(&db, &case_id, "Taranmamış belge");
        let report = check(&db, &case_id, "“uzun bir alıntı metni” var.", None);
        assert_eq!(report.pages_scanned, 0);
        assert_eq!(report.quotes[0].state, "unverifiable");
        assert_eq!(report.quotes[0].reason.as_deref(), Some("no_text"));
        assert_eq!(report.unverifiable, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("Taranacak ayrıştırılmış metin bulunamadı")));
    }

    #[test]
    fn quote_spanning_two_pages_is_found_with_its_page_range() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "Kiracı kira bedelini",
                "ödemediği için tahliyeye karar verildi.",
            ],
        );
        let report = check(&db, &case_id, "“kira bedelini ödemediği” ifadesi.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(finding.occurrences, 1);
        assert_eq!(report.normalized, 1);
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
        assert!(position.spans_pages);
        assert_eq!(position.match_kind, "whitespace");
        assert_eq!(position.char_start, 7);
        assert_eq!(position.char_end, 9);
        assert_eq!(position.source_text, "kira bedelini\nödemediği");
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("sayfa sınırını aşarak bulundu")));
        // Sayfa sınırını aşan konum tek sayfalık anchor'a bağlanamaz: çekirdek
        // reddeder (arayüz de düğmeyi kapatır).
        let error = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                // Arayüz `charEnd` gönderir (son sayfadaki konum): tek segmentte
                // uzunluk tutmaz, çekirdek aralığı reddeder. Yani sayfa sınırını
                // aşan alıntı **tek segmentlik anchor olarak ifade edilemez**.
                end_offset: position.char_end,
                exact_quote: finding.quote.clone(),
            })
            .expect_err("sayfa sınırını aşan alıntı bağlanmamalı");
        assert_eq!(error.code(), "E_DB_SETTING");
    }

    #[test]
    fn exact_quote_that_carries_the_page_break_stays_exact_but_is_not_bindable() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Karar", &["Karar gerekçesi", "şöyledir"]);
        // Alıntı sayfa sonundaki satır sonunu **taşıyor**: katman `exact`.
        let report = check(&db, &case_id, "“Karar gerekçesi\nşöyledir” denildi.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "exact");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "exact");
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
        assert!(position.spans_pages);
        // Birebir olsa da tek sayfaya sığmadığı için bağlanamaz; "birebir tek
        // konumda" notu bu alıntıyı saymaz.
        assert!(!report
            .notes
            .iter()
            .any(|note| note.contains("kaynak bağlanabilir")));
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("tek sayfalık anchor'a bağlanmaz")));
    }

    #[test]
    fn cross_page_match_is_ambiguous_when_it_repeats_over_two_breaks() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Tutanak",
            &[
                "kira bedelini",
                "ödemediği birinci kayıt",
                "kira bedelini",
                "ödemediği ikinci kayıt",
            ],
        );
        let report = check(&db, &case_id, "“kira bedelini ödemediği” yazıyor.", None);
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 2);
        assert_eq!(finding.candidates.len(), 2);
        assert!(finding
            .candidates
            .iter()
            .all(|candidate| candidate.spans_pages));
        assert_eq!(finding.candidates[0].page_index, 0);
        assert_eq!(finding.candidates[1].page_index, 2);
    }

    #[test]
    fn cross_page_match_works_in_the_fold_tier() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["ÖDEME YAPILMADIĞINI", "tespit etti."],
        );
        let report = check(
            &db,
            &case_id,
            "“ödeme yapılmadığını tespit” sabittir.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "turkish_fold");
        assert!(position.spans_pages);
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
        assert_eq!(position.source_text, "ÖDEME YAPILMADIĞINI\ntespit");
    }

    #[test]
    fn text_without_quotes_returns_empty_report() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        add_document(&db, &case_id, "Belge", &["metin"]);
        let report = check(&db, &case_id, "Tırnaksız bir iddia metni.", None);
        assert_eq!(report.spans_found, 0);
        assert!(report.quotes.is_empty());
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("tırnaklı alıntı bulunamadı")));
    }

    #[test]
    fn too_many_quotes_and_too_long_text_are_rejected() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        let many = vec!["“alıntı metni bir”"; MAX_IN_TEXT_QUOTES + 1].join(" ");
        let error = db
            .check_in_text_quotes(
                &case_id,
                &InTextQuoteInput {
                    text: many,
                    document_id: None,
                },
            )
            .expect_err("too many");
        assert_eq!(error.code(), "E_QUOTE_TOO_MANY");

        let long = "a".repeat(MAX_IN_TEXT_CHECK_CHARS + 1);
        let error = db
            .check_in_text_quotes(
                &case_id,
                &InTextQuoteInput {
                    text: long,
                    document_id: None,
                },
            )
            .expect_err("too long");
        assert_eq!(error.code(), "E_QUOTE_TEXT");
    }

    #[test]
    fn unknown_case_is_not_found() {
        let db = db();
        let error = db
            .check_in_text_quotes(
                "bilinmeyen",
                &InTextQuoteInput {
                    text: "“uzun bir alıntı metni”".into(),
                    document_id: None,
                },
            )
            .expect_err("unknown case");
        assert_eq!(error.code(), "E_NOT_FOUND");
    }

    #[test]
    fn quote_missing_from_the_current_version_is_found_in_history() {
        let db = db();
        let case_id = new_case(&db, "Bilirkişi");
        let (document_id, first_version) = add_document(
            &db,
            &case_id,
            "Bilirkişi raporu",
            &["Raporun eski sürümünde kalan cümle burada."],
        );
        let current_version = add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Güncel metin, cümle çıkarıldı."],
        );

        let report = check(&db, &case_id, "Raporda “kalan cümle burada” yazıyor.", None);

        assert_eq!(report.exact, 1, "alıntı eski sürümde birebir bulunmalı");
        assert_eq!(report.not_found, 0);
        assert_eq!(report.found_in_old_version, 1);
        assert_eq!(report.historical_versions_scanned, 1);
        assert_eq!(report.versions_scanned, 2);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.document_version_id, first_version);
        assert_ne!(position.document_version_id, current_version);
        assert_eq!(position.version_no, 1);
        assert!(!position.is_current_version);
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("yalnız **eski** bir sürümde bulundu")));
    }

    #[test]
    fn historical_quote_binds_an_anchor_to_the_old_version() {
        let db = db();
        let case_id = new_case(&db, "Bilirkişi");
        let (document_id, first_version) = add_document(
            &db,
            &case_id,
            "Bilirkişi raporu",
            &["Raporun eski sürümünde kalan cümle burada."],
        );
        add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Güncel metin, cümle çıkarıldı."],
        );

        let report = check(&db, &case_id, "“kalan cümle burada”", None);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert!(!position.is_current_version);

        // Eski sürümün segmenti üzerinden anchor kurulabilir: kayıt hangi
        // sürümde doğrulandığını taşır, sessizce güncel sürüme yazılmaz.
        let anchor = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_end,
                exact_quote: position.source_text.clone(),
            })
            .expect("anchor");
        assert_eq!(anchor.document_version_id, first_version);
        assert_eq!(anchor.exact_quote, "kalan cümle burada");
        let location = db.locate_anchor(&anchor.id).expect("locate").expect("row");
        assert!(location.quote_intact);
    }

    #[test]
    fn current_version_wins_over_history_and_history_is_not_scanned() {
        let db = db();
        let case_id = new_case(&db, "Kira");
        let (document_id, _) =
            add_document(&db, &case_id, "Sözleşme", &["Karar gerekçesi şöyledir."]);
        add_version_with_pages(&db, &case_id, &document_id, &["Karar gerekçesi şöyledir."]);

        let report = check(
            &db,
            &case_id,
            "Metinde “Karar gerekçesi şöyledir” deniyor.",
            None,
        );

        assert_eq!(report.exact, 1);
        assert_eq!(
            report.quotes[0].occurrences, 1,
            "güncel sürüm kazandı, yinelenmez"
        );
        assert_eq!(report.found_in_old_version, 0);
        assert_eq!(
            report.historical_versions_scanned, 0,
            "güncel sürümde bulunan alıntı için eski sürümler taranmaz"
        );
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert!(position.is_current_version);
        assert_eq!(position.version_no, 2);
    }

    #[test]
    fn the_same_passage_in_two_old_versions_is_one_location_not_ambiguous() {
        let db = db();
        let case_id = new_case(&db, "Karar");
        let (document_id, first_version) =
            add_document(&db, &case_id, "Karar metni", &["Karar gerekçesi şöyledir."]);
        // Aynı pasajın düzeltilmemiş ikinci nüshası (v2): metin birebir aynı.
        let second_version =
            add_version_with_pages(&db, &case_id, &document_id, &["Karar gerekçesi şöyledir."]);
        // Güncel sürümde cümle çıkarıldı: alıntı yalnız eski sürümlerde.
        add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Güncel metin, cümle çıkarıldı."],
        );

        let report = check(&db, &case_id, "“Karar gerekçesi şöyledir”", None);

        assert_eq!(report.ambiguous, 0, "sürüm kopyası ayrı konum değildir");
        assert_eq!(report.exact, 1);
        let finding = &report.quotes[0];
        assert_eq!(
            finding.occurrences, 1,
            "aynı pasajın iki nüshası tek konumdur"
        );
        assert_eq!(finding.distinct_documents, 1);
        let position = finding.position.as_ref().expect("konum");
        assert_eq!(
            position.document_version_id, second_version,
            "en yeni eski sürüm seçilir"
        );
        assert_ne!(position.document_version_id, first_version);
        assert_eq!(position.version_no, 2);
        assert!(!position.is_current_version);
        assert_eq!(report.found_in_old_version, 1);
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("sürüm kopyaları ayrı konum sayılmadı")),
            "sürüm kopyası birleştirmesi dürüstçe yazılmalı: {:?}",
            report.notes
        );
    }

    #[test]
    fn an_ambiguous_quote_that_is_only_in_an_old_version_is_reported_as_old() {
        let db = db();
        let case_id = new_case(&db, "Karar");
        let (document_id, _) = add_document(
            &db,
            &case_id,
            "Karar metni",
            &["Karar gerekçesi şöyledir. Sonra yine: Karar gerekçesi şöyledir."],
        );
        add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Güncel metin, cümle çıkarıldı."],
        );

        let report = check(&db, &case_id, "“Karar gerekçesi şöyledir”", None);

        assert_eq!(
            report.ambiguous, 1,
            "aynı sürümde iki konum gerçekten belirsiz"
        );
        assert_eq!(report.quotes[0].occurrences, 2);
        assert_eq!(
            report.found_in_old_version, 1,
            "konum yok ama eşleşme yalnız eski sürümde"
        );
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("yalnız **eski** bir sürümde bulundu")));
    }

    #[test]
    fn exact_history_beats_normalized_current_version() {
        let db = db();
        let case_id = new_case(&db, "Karar");
        let (document_id, first_version) =
            add_document(&db, &case_id, "Karar metni", &["Karar gerekçesi şöyledir."]);
        // Güncel sürümde yalnız **boşluk farkıyla** geçiyor: `whitespace` katmanı.
        add_version_with_pages(
            &db,
            &case_id,
            &document_id,
            &["Karar  gerekçesi    şöyledir."],
        );

        let report = check(&db, &case_id, "“Karar gerekçesi şöyledir”", None);

        assert_eq!(report.exact, 1, "eski sürümdeki birebir eşleşme üstündür");
        assert_eq!(report.normalized, 0);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.match_kind, "exact");
        assert_eq!(position.version_no, 1);
        assert_eq!(position.document_version_id, first_version);
        assert!(!position.is_current_version);
    }

    #[test]
    fn document_scope_does_not_scan_another_documents_history() {
        let db = db();
        let case_id = new_case(&db, "Dosya");
        let (first_document, _) =
            add_document(&db, &case_id, "Dilekçe", &["Dilekçe güncel metni."]);
        let (second_document, _) = add_document(
            &db,
            &case_id,
            "Ek rapor",
            &["Ek raporda kalan cümle burada."],
        );
        add_version_with_pages(&db, &case_id, &second_document, &["Ek rapor güncel metni."]);

        let scoped = check(&db, &case_id, "“kalan cümle burada”", Some(&first_document));
        assert_eq!(scoped.not_found, 1, "başka belgenin eski sürümü taranmaz");
        assert_eq!(scoped.found_in_old_version, 0);

        let whole_case = check(&db, &case_id, "“kalan cümle burada”", None);
        assert_eq!(whole_case.exact, 1);
        assert_eq!(whole_case.found_in_old_version, 1);
    }

    #[test]
    fn old_versions_beyond_the_limit_are_reported_not_silently_skipped() {
        let db = db();
        let case_id = new_case(&db, "Dosya");
        let (document_id, _) = add_document(
            &db,
            &case_id,
            "Sözleşme",
            &["En eski sürümde kalan cümle burada."],
        );
        // v2..v6 eski sürümler (sınır 5), v7 güncel; alıntı yalnız v1'de.
        for index in 2..=MAX_HISTORICAL_VERSIONS_PER_DOCUMENT + 2 {
            let body = format!("Sürüm {index} gövde metni.");
            add_version_with_pages(&db, &case_id, &document_id, &[body.as_str()]);
        }

        let report = check(&db, &case_id, "“kalan cümle burada”", None);

        assert_eq!(report.not_found, 1, "v1 sınırın ötesinde: taranmadı");
        assert_eq!(
            report.historical_versions_scanned,
            MAX_HISTORICAL_VERSIONS_PER_DOCUMENT
        );
        assert!(report
            .notes
            .iter()
            .any(|note| note.contains("en çok 5 eski sürüm tarandı")));
    }

    #[test]
    fn quote_is_found_when_the_source_breaks_a_word_with_a_line_hyphen() {
        let db = db();
        let case_id = new_case(&db, "Satır sonu tiresi");
        add_document(
            &db,
            &case_id,
            "Karar.pdf",
            &["Mahkeme, davacının kira bedel-\nlini ödemediğini tespit etmiştir."],
        );

        let report = check(
            &db,
            &case_id,
            "Davacı “kira bedellini ödemediğini” söyledi.",
            None,
        );

        assert_eq!(report.not_found, 0, "tire ile bölünmüş kelime bulunamadı");
        assert_eq!(report.normalized, 1);
        assert_eq!(report.dehyphenated, 1);
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("tire ile bölünmüş kelime")),
            "notlar tire birleştirmesini yazmalı: {:?}",
            report.notes
        );
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.match_kind, "dehyphenated");
        assert!(!position.spans_pages);
        assert!(
            position.source_text.contains("bedel-\nlini"),
            "kaynak metin tire + satır sonunu göstermeli: {}",
            position.source_text
        );
    }

    /// F12-T14z — tipografik tire ailesi: kaynak satır sonunda `‐` (U+2010)
    /// ya da `‑` (U+2011) yazıyorsa alıntı `dehyphenated` katmanında bulunur;
    /// en tire (`–`) çizgi işaretidir, birleştirilmez.
    #[test]
    fn quote_with_a_typographic_line_hyphen_is_found() {
        let db = db();
        let case_id = new_case(&db, "Tipografik tire");
        add_document(
            &db,
            &case_id,
            "Karar.pdf",
            &["Mahkeme, davacının kira bedel\u{2010}\nlini ödemediğini tespit etmiştir."],
        );
        let report = check(
            &db,
            &case_id,
            "Davacı “kira bedellini ödemediğini” söyledi.",
            None,
        );
        assert_eq!(report.not_found, 0, "tipografik tire eşleşmeyi engelledi");
        assert_eq!(report.normalized, 1);
        assert_eq!(report.dehyphenated, 1);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.match_kind, "dehyphenated");
        assert!(
            position.source_text.contains("bedel\u{2010}\nlini"),
            "kaynak metin tire ailesini göstermeli: {}",
            position.source_text
        );

        // En tire (`–`) birleştirilmez: alıntı bulunamaz.
        let case2 = new_case(&db, "En tire");
        add_document(
            &db,
            &case2,
            "Karar.pdf",
            &["Mahkeme, davacının kira bedel\u{2013}\nlini ödemediğini tespit etmiştir."],
        );
        let missed = check(
            &db,
            &case2,
            "Davacı “kira bedellini ödemediğini” söyledi.",
            None,
        );
        assert_eq!(missed.not_found, 1, "en tire tire değildir");
    }

    /// Font kaynaklı bitişik yazı (`ﬂ`) **kendi katmanında** bulunur: birebir
    /// sayılmaz, nedeni notta yazılır, kaynak metin ligatürü gösterir.
    #[test]
    fn ligature_from_the_font_is_found_in_its_own_tier() {
        let db = db();
        let case_id = new_case(&db, "Ligatür");
        add_document(
            &db,
            &case_id,
            "Karar.pdf",
            &["Sözleşme taraﬂar arasında kurulmuştur."],
        );

        let report = check(&db, &case_id, "“taraflar arasında kurulmuştur”", None);

        assert_eq!(report.not_found, 0, "ligatür eşleşmeyi engelledi");
        assert_eq!(report.normalized, 1);
        assert_eq!(report.ligature, 1);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.match_kind, "ligature");
        assert_eq!(position.char_start, 9);
        assert!(
            position.source_text.contains('\u{FB02}'),
            "kaynak metin ligatürü göstermeli: {}",
            position.source_text
        );
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("bitişik yazı (ligatür)")),
            "notlar ligatürü yazmalı: {:?}",
            report.notes
        );
    }

    /// Sayfa metni satır sonuyla bitiyorsa (`worker` her satıra `\n` yazar)
    /// sayfa sonundaki tire **birleşir**; fazladan bir ayırıcı `\n` eklenmez.
    #[test]
    fn hyphen_at_page_end_is_joined_across_the_page_boundary() {
        let db = db();
        let case_id = new_case(&db, "Sayfa sonu tiresi");
        add_document(
            &db,
            &case_id,
            "Karar.pdf",
            &[
                "İtirazın reddine karar ve-\n",
                "rilmiştir. Hüküm tebliğ edildi.",
            ],
        );

        let report = check(&db, &case_id, "“reddine karar verilmiştir”", None);

        assert_eq!(report.not_found, 0, "sayfa sonu tiresi birleşmedi");
        assert_eq!(report.dehyphenated, 1);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.match_kind, "dehyphenated");
        assert!(position.spans_pages, "eşleşme sayfa sınırını aşmalı");
        assert_eq!(position.page_index, 0);
        assert_eq!(position.end_page_index, 1);
        assert!(
            position.source_text.contains("ve-"),
            "kaynak metin tireyi göstermeli: {}",
            position.source_text
        );
    }

    #[test]
    fn soft_hyphen_inside_a_word_is_not_a_difference() {
        let db = db();
        let case_id = new_case(&db, "Yumuşak tire");
        add_document(
            &db,
            &case_id,
            "Dilekçe.pdf",
            &["Davacı kira bedel\u{00AD}ini ödemedi."],
        );

        let report = check(&db, &case_id, "“kira bedelini ödemedi”", None);

        assert_eq!(report.not_found, 0, "yumuşak tire eşleşmeyi engelledi");
        assert_eq!(report.normalized, 1);
        assert_eq!(
            report.quotes[0]
                .position
                .as_ref()
                .expect("konum")
                .match_kind,
            "dehyphenated"
        );
    }

    #[test]
    fn a_real_hyphen_between_words_is_not_joined() {
        let db = db();
        let case_id = new_case(&db, "Gerçek tire");
        add_document(
            &db,
            &case_id,
            "Sözleşme.pdf",
            &["Taraflar kredi-kartı sözleşmesi imzalamıştır."],
        );

        let report = check(&db, &case_id, "“kredikartı sözleşmesi”", None);

        assert_eq!(
            report.not_found, 1,
            "satır sonu olmayan tire birleştirilmemeli"
        );
    }

    #[test]
    fn hyphenated_word_across_a_page_break_is_found_and_still_not_bindable() {
        let db = db();
        let case_id = new_case(&db, "Sayfa sınırında tire");
        add_document(
            &db,
            &case_id,
            "Bilirkişi.pdf",
            &[
                "Bilirkişi raporunda kira bedel-",
                "lini ödemediğini belirtmiştir.",
            ],
        );

        let report = check(&db, &case_id, "“kira bedellini ödemediğini”", None);

        assert_eq!(report.not_found, 0);
        assert_eq!(report.normalized, 1);
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.match_kind, "dehyphenated");
        assert!(position.spans_pages, "iki sayfaya yayılmalı");
    }

    #[test]
    fn a_hyphen_at_a_paragraph_break_is_not_joined() {
        let db = db();
        let case_id = new_case(&db, "Paragraf ayrımı");
        add_document(
            &db,
            &case_id,
            "Tutanak.pdf",
            &["Birinci madde-\n\nAşağıdaki hükümler uygulanır."],
        );

        let report = check(&db, &case_id, "“maddeaşağıdaki hükümler”", None);

        assert_eq!(
            report.not_found, 1,
            "paragraf ayrımındaki tire kelime birleştirmesi sayılmamalı"
        );
        assert_eq!(report.dehyphenated, 0);
    }

    #[test]
    fn exact_match_wins_over_a_dehyphenated_one() {
        let db = db();
        let case_id = new_case(&db, "Katman sırası");
        add_document(
            &db,
            &case_id,
            "Karar.pdf",
            &["Kira bedel-\nlini ödemedi. Sonra kira bedellini ödedi."],
        );

        let report = check(&db, &case_id, "“kira bedellini ödedi”", None);

        assert_eq!(report.exact, 1, "birebir eşleşme en güçlü katmanda kalmalı");
        assert_eq!(report.quotes[0].state, "exact");
    }

    /// Belge + sürüm + ayrıştırma; sayfalar **metin kaynağıyla** verilir.
    /// `origin="none"` = metin katmanı yok: `pages` satırı yazılır, metin
    /// segmenti yazılmaz (gerçek OCR'sız taramanın ürettiği durum).
    fn add_document_with_origins(
        db: &Database,
        case_id: &str,
        name: &str,
        pages: &[(&str, &str)],
    ) -> String {
        let blob = db
            .create_blob_record(&uuid::Uuid::now_v7().to_string(), 10, "aad")
            .expect("blob");
        let document = db
            .create_document(case_id, name, "pdf", None, "unknown")
            .expect("doc");
        let version = db
            .add_version(
                &document.id,
                case_id,
                &blob,
                &"a".repeat(64),
                10,
                "application/pdf",
            )
            .expect("version");
        let run = db
            .start_parse_run(&version.id, case_id, "pymupdf", "1.24", "pdf|ocr=false")
            .expect("run");
        let stored: Vec<NewPage> = pages
            .iter()
            .enumerate()
            .map(|(index, (text, origin))| NewPage {
                origin: (*origin).into(),
                warnings: if *origin == "none" {
                    vec!["metin katmanı yok".into()]
                } else {
                    Vec::new()
                },
                ..page(i64::try_from(index).unwrap_or(i64::MAX), text)
            })
            .collect();
        db.store_pages(&run.id, case_id, &stored).expect("pages");
        db.finish_parse_run(&run.id, "partial", &["metin katmanı yok".into()])
            .expect("finish");
        document.id
    }

    #[test]
    fn page_without_text_layer_is_not_counted_as_scanned() {
        let db = db();
        let case_id = new_case(&db, "Taranmış sayfa");
        add_document_with_origins(
            &db,
            &case_id,
            "Tapu taranmış.pdf",
            &[
                ("Kira bedeli ödenmedi ve temerrüt oluştu.", "native"),
                ("", "none"),
            ],
        );

        let report = check(&db, &case_id, "“Kira bedeli ödenmedi”", None);

        assert_eq!(report.quotes[0].state, "exact");
        assert_eq!(report.pages_scanned, 1, "yalnız metinli sayfa arandı");
        assert_eq!(
            report.pages_without_text, 1,
            "metin katmanı olmayan sayfa 'tarandı' sayılmamalı"
        );
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("1 sayfanın metin katmanı yok")));

        // Aynı kapsamda bulunamayan alıntı "uydurma" değildir: metinsiz sayfa
        // var, alıntı orada geçiyor olabilir — rapor bunu ayrı söyler.
        let missing = check(&db, &case_id, "“tamamen başka bir cümle burada”", None);
        assert_eq!(missing.quotes[0].state, "not_found");
        assert_eq!(missing.pages_without_text, 1);
    }

    #[test]
    fn scanned_document_without_any_text_layer_is_unverifiable_not_not_found() {
        let db = db();
        let case_id = new_case(&db, "OCR'sız tarama");
        add_document_with_origins(
            &db,
            &case_id,
            "Taranmış vekaletname.pdf",
            &[("", "none"), ("   ", "none")],
        );

        let report = check(&db, &case_id, "“uzun bir alıntı metni” var.", None);

        assert_eq!(report.documents_total, 1);
        assert_eq!(
            report.documents_with_text, 0,
            "hiç metinli sayfası olmayan belge metinli sayılmamalı"
        );
        assert_eq!(report.documents_without_text, 1);
        assert_eq!(report.pages_scanned, 0, "aranacak sayfa yok");
        assert_eq!(report.pages_without_text, 2);
        assert_eq!(
            report.quotes[0].state, "unverifiable",
            "hiçbir sayfa okunamadıysa 'Bulunamadı' hükmü verilmez"
        );
        assert_eq!(report.quotes[0].reason.as_deref(), Some("no_text"));
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("2 sayfanın metin katmanı yok")));
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("ayrıştırılmış metin yok")));
    }

    #[test]
    fn scanned_document_counts_as_without_text_beside_a_readable_one() {
        let db = db();
        let case_id = new_case(&db, "Karışık kapsam");
        add_document(
            &db,
            &case_id,
            "Dilekçe.pdf",
            &["Kira bedeli ödenmedi ve temerrüt oluştu."],
        );
        add_document_with_origins(&db, &case_id, "Ekran görüntüsü.pdf", &[("", "none")]);

        let report = check(&db, &case_id, "“Kira bedeli ödenmedi”", None);

        assert_eq!(report.quotes[0].state, "exact");
        assert_eq!(report.documents_total, 2);
        assert_eq!(report.documents_with_text, 1);
        assert_eq!(report.documents_without_text, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("1 belgenin güncel sürümünde ayrıştırılmış metin yok")));
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("1 sayfanın metin katmanı yok")));
    }

    // ---- F12-T14j: metin içi alıntılar seçili **kararın** tam metninde ----

    fn new_precedent(db: &Database, docket: &str) -> crate::precedents::PrecedentRow {
        db.create_precedent(&NewPrecedent {
            court: "Yargıtay".into(),
            chamber: Some("1. Hukuk Dairesi".into()),
            docket_no: Some(docket.into()),
            decision_no: Some("2025/77".into()),
            application_no: None,
            decision_date: Some("2025-06-01".into()),
            title: Some("Kira alacağı".into()),
            source_url: Some("https://example.invalid/karar".into()),
            user_tags: vec![],
            legal_issue: None,
        })
        .expect("emsal")
    }

    fn add_precedent_text(db: &Database, precedent_id: &str, text: &str) -> String {
        db.add_precedent_text(
            precedent_id,
            &NewPrecedentText {
                source_kind: "paste".into(),
                text: text.into(),
                document_version_id: None,
            },
        )
        .expect("tam metin")
        .id
    }

    const KARAR_METNI: &str =
        "Yargıtay 1. Hukuk Dairesi\n\nMahkeme, itirazın reddine karar verdi.\nGerekçede ödeme yapılmadığı belirtildi.";

    #[test]
    fn in_text_quotes_are_checked_against_the_selected_precedent_only() {
        let db = db();
        let p = new_precedent(&db, "2024/300");
        let version = add_precedent_text(&db, &p.id, KARAR_METNI);
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: "Dilekçede “itirazın reddine karar verdi” ve “uydurma bir pasaj burada yok” yazıyor."
                        .into(),
                },
            )
            .expect("denetim");
        assert_eq!(report.spans_found, 2);
        assert_eq!(report.quotes[0].state, "exact");
        assert_eq!(report.quotes[0].method.as_deref(), Some("exact"));
        let position = report.quotes[0].position.as_ref().expect("konum");
        assert_eq!(position.source_text, "itirazın reddine karar verdi");
        assert_eq!(position.version_id, version);
        assert!(position.is_current_version);
        assert_eq!(report.quotes[1].state, "not_found");
        assert_eq!(report.quotes[1].reason.as_deref(), Some("not_found"));
        assert!(report.quotes[1].position.is_none());
        assert_eq!(report.exact, 1);
        assert_eq!(report.not_found, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("“uydurma” hükmü değildir")));
    }

    #[test]
    fn precedent_in_text_check_reports_each_normalized_tier() {
        let cases = [
            ("ödeme   yapılmadığı", "whitespace"),
            ("taraﬂar arasında sözleşme", "ligature"),
            ("kelime-\nsatır", "dehyphenated"),
        ];
        for (source, method) in cases {
            let db = db();
            let p = new_precedent(&db, "2024/301");
            add_precedent_text(&db, &p.id, source);
            let quote = match method {
                "whitespace" => "ödeme yapılmadığı",
                "ligature" => "taraflar arasında sözleşme",
                _ => "kelimesatır",
            };
            let report = db
                .check_in_text_quotes_in_precedent(
                    &p.id,
                    &PrecedentInTextInput {
                        text: format!("Metinde “{quote}” geçiyor."),
                    },
                )
                .expect("denetim");
            assert_eq!(
                report.quotes[0].method.as_deref(),
                Some(method),
                "kaynak {source:?} için katman"
            );
            assert_eq!(report.quotes[0].state, "normalized");
        }
        let db = db();
        let p = new_precedent(&db, "2024/302");
        add_precedent_text(&db, &p.id, "ÖDEME YAPILMADIĞI sabittir.");
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: "Metinde “ödeme yapılmadığı” geçiyor.".into(),
                },
            )
            .expect("denetim");
        assert_eq!(report.quotes[0].method.as_deref(), Some("turkish_fold"));
        assert_eq!(report.quotes[0].state, "normalized");
        assert_eq!(report.normalized, 1);
    }

    #[test]
    fn repeated_passage_inside_one_decision_is_ambiguous_with_candidates() {
        let db = db();
        let p = new_precedent(&db, "2024/303");
        add_precedent_text(
            &db,
            &p.id,
            "itirazın reddine karar verdi. itirazın reddine karar verdi. itirazın reddine karar verdi.",
        );
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: "“itirazın reddine karar verdi” cümlesi üç kez geçiyor.".into(),
                },
            )
            .expect("denetim");
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "ambiguous");
        assert_eq!(finding.occurrences, 3);
        assert!(finding.position.is_none());
        assert_eq!(finding.candidates.len(), 3);
        assert!(!finding.found_in_old_version);
        assert_eq!(report.ambiguous, 1);
    }

    #[test]
    fn quote_only_in_an_old_version_is_reported_with_that_version() {
        let db = db();
        let p = new_precedent(&db, "2024/304");
        let old = add_precedent_text(
            &db,
            &p.id,
            "Eski nüsha: kira bedellini ödemeyen kiracı hakkında tahliye kararı.",
        );
        add_precedent_text(&db, &p.id, "Yeni nüsha: itirazın reddine karar verildi.");
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: "“kira bedellini ödemeyen kiracı” pasajına dayanılmıştır.".into(),
                },
            )
            .expect("denetim");
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "exact");
        assert!(finding.found_in_old_version);
        let position = finding.position.as_ref().expect("konum");
        assert_eq!(position.version_id, old);
        assert!(!position.is_current_version);
        assert_eq!(report.found_in_old_version, 1);
        assert_eq!(report.versions_scanned, 2);
        assert_eq!(report.historical_versions_scanned, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("yalnız **eski bir sürümde** bulundu")));
    }

    #[test]
    fn in_text_and_single_quote_paths_agree_on_tier_order() {
        let db = db();
        let p = new_precedent(&db, "2024/305");
        let old = add_precedent_text(&db, &p.id, "Karar: itirazın reddine karar verdi.");
        add_precedent_text(&db, &p.id, "Karar: itirazın  reddine karar verdi.");
        let quote = "itirazın reddine karar verdi";
        let single = db.verify_precedent_quote(&p.id, quote).expect("tek");
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: format!("“{quote}” deniyor."),
                },
            )
            .expect("metin içi");
        let finding = &report.quotes[0];
        // Eski sürümdeki **birebir** eşleşme, günceldeki boşluk farkını yener.
        assert_eq!(single.method.as_deref(), Some("exact"));
        assert_eq!(finding.method.as_deref(), Some("exact"));
        assert_eq!(single.version_id.as_deref(), Some(old.as_str()));
        assert_eq!(
            finding.position.as_ref().map(|p| p.version_id.as_str()),
            Some(old.as_str())
        );
        assert_eq!(single.occurrences, finding.occurrences);
        assert_eq!(
            single.versions_scanned,
            i64::try_from(report.versions_scanned).unwrap()
        );
    }

    #[test]
    fn precedent_in_text_elided_quote_matches_the_single_quote_path() {
        let db = db();
        let p = new_precedent(&db, "2024/320");
        add_precedent_text(
            &db,
            &p.id,
            "Karar: sözleşmenin haklı nedenle feshedilebileceğine ve kiracının kira \
             bedelini ödemekle temerrüde düştüğüne hükmedildi.",
        );
        let quote = "sözleşmenin haklı nedenle feshedilebileceğine … kiracının kira bedelini \
                     ödemekle temerrüde düştüğüne";
        let single = db.verify_precedent_quote(&p.id, quote).expect("tek alıntı");
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: format!("“{quote}” deniyor."),
                },
            )
            .expect("metin içi");
        let finding = &report.quotes[0];
        assert_eq!(single.method.as_deref(), Some("elided"));
        assert_eq!(finding.method.as_deref(), Some("elided"));
        assert_eq!(finding.state, "normalized");
        assert_eq!(single.occurrences, finding.occurrences);
        assert_eq!(report.elided, 1);
        assert!(report.notes.iter().any(|n| n.contains("kısaltılmış")));
        // Kaynak konumu, atlanan kısmı **içeren** ham metni gösterir.
        let position = finding.position.as_ref().expect("position");
        assert!(position.source_text.contains("ve kiracının kira"));
    }

    #[test]
    fn precedent_in_text_elided_quote_requires_the_parts_in_order() {
        let db = db();
        let p = new_precedent(&db, "2024/323");
        add_precedent_text(
            &db,
            &p.id,
            "Karar: ikinci parça burada geçiyor ve sonra birinci parça burada geçiyor.",
        );
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: "“birinci parça burada … ikinci parça burada” deniyor.".into(),
                },
            )
            .expect("metin içi");
        assert_eq!(report.quotes[0].state, "not_found");
        assert_eq!(report.elided, 0);
    }

    #[test]
    fn precedent_in_text_edge_clipped_quote_matches_the_single_quote_path() {
        let db = db();
        let p = new_precedent(&db, "2024/324");
        add_precedent_text(
            &db,
            &p.id,
            "Karar: sözleşmenin haklı nedenle feshedilebileceğine ve kiracının kira \
             bedelini ödemekle temerrüde düştüğüne hükmedildi.",
        );
        let quote = "…kiracının kira bedelini ödemekle temerrüde düştüğüne";
        let single = db.verify_precedent_quote(&p.id, quote).expect("tek alıntı");
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: format!("“{quote}” deniyor."),
                },
            )
            .expect("metin içi");
        let finding = &report.quotes[0];
        assert_eq!(single.method.as_deref(), Some("elided"));
        assert_eq!(finding.method.as_deref(), Some("elided"));
        assert_eq!(finding.state, "normalized");
        assert_eq!(single.occurrences, finding.occurrences);
        assert_eq!(report.elided, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("başı/sonu kırpılmış")));
        // Kırpılan uç kaynakta yok; eşleşen aralık yalnız gösterilen kısmı kapsar.
        let position = finding.position.as_ref().expect("position");
        assert_eq!(
            position.source_text,
            "kiracının kira bedelini ödemekle temerrüde düştüğüne"
        );
    }

    #[test]
    fn precedent_without_full_text_cannot_verify_in_text_quotes() {
        let db = db();
        let p = new_precedent(&db, "2024/306");
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: "“itirazın reddine karar verdi” deniyor.".into(),
                },
            )
            .expect("denetim");
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "unverifiable");
        assert_eq!(finding.reason.as_deref(), Some("no_full_text"));
        assert_ne!(finding.state, "not_found");
        assert_eq!(report.unverifiable, 1);
        assert_eq!(report.versions_total, 0);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("Kararın tam metni yok")));
    }

    #[test]
    fn precedent_in_text_check_enforces_its_limits_and_scope() {
        let db = db();
        let p = new_precedent(&db, "2024/307");
        add_precedent_text(&db, &p.id, KARAR_METNI);
        let missing = db
            .check_in_text_quotes_in_precedent(
                "olmayan-emsal",
                &PrecedentInTextInput {
                    text: "“itirazın reddine karar verdi”".into(),
                },
            )
            .expect_err("olmayan emsal");
        assert_eq!(missing.code(), "E_NOT_FOUND");

        let mut many = String::new();
        for index in 0..31 {
            many.push_str(&format!("“alıntı numarası {index} burada” "));
        }
        let error = db
            .check_in_text_quotes_in_precedent(&p.id, &PrecedentInTextInput { text: many })
            .expect_err("31 alıntı");
        assert_eq!(error.code(), "E_QUOTE_TOO_MANY");

        let long = "a".repeat(MAX_IN_TEXT_CHECK_CHARS + 1);
        let error = db
            .check_in_text_quotes_in_precedent(&p.id, &PrecedentInTextInput { text: long })
            .expect_err("uzun metin");
        assert_eq!(error.code(), "E_QUOTE_TEXT");

        let short = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: "“kısa” alıntı.".into(),
                },
            )
            .expect("kısa");
        assert_eq!(short.spans_found, 0);
        assert_eq!(short.skipped_short, 1);
        assert!(short
            .notes
            .iter()
            .any(|n| n.contains("tırnaklı alıntı bulunamadı")));
    }

    // ---- F12-T14k: metin içi alıntılar **kütüphane genelinde** ----

    #[test]
    fn library_search_reports_every_matching_precedent_and_keeps_scope_honest() {
        let db = db();
        let p1 = new_precedent(&db, "2024/400");
        add_precedent_text(&db, &p1.id, KARAR_METNI);
        let p2 = new_precedent(&db, "2024/401");
        add_precedent_text(
            &db,
            &p2.id,
            "Başka karar: itirazın  reddine karar verdi (boşluk farkı).",
        );
        // Tam metni olmayan emsal taranmaz ve «yok» sayılmaz.
        let p3 = new_precedent(&db, "2024/402");

        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "Dilekçede “itirazın reddine karar verdi” ve “kütüphanede olmayan pasaj” geçiyor."
                    .into(),
            })
            .expect("kütüphane araması");

        assert_eq!(report.spans_found, 2);
        assert_eq!(report.library_total, 2, "yalnız tam metinli emsaller");
        assert_eq!(report.scanned, 2);
        assert!(!report.truncated);

        let found = &report.quotes[0];
        assert_eq!(found.state, "found");
        assert_eq!(found.matched_precedents, 2);
        assert_eq!(found.matches.len(), 2);
        assert_eq!(found.hidden_matches, 0);
        let ids: Vec<&str> = found
            .matches
            .iter()
            .map(|m| m.precedent_id.as_str())
            .collect();
        assert!(ids.contains(&p1.id.as_str()) && ids.contains(&p2.id.as_str()));
        assert!(!ids.contains(&p3.id.as_str()));
        let exact = found
            .matches
            .iter()
            .find(|m| m.precedent_id == p1.id)
            .expect("p1 eşleşmesi");
        assert_eq!(exact.method, "exact");
        assert_eq!(exact.source_text, "itirazın reddine karar verdi");
        assert_eq!(exact.decision_date.as_deref(), Some("2025-06-01"));
        let normalized = found
            .matches
            .iter()
            .find(|m| m.precedent_id == p2.id)
            .expect("p2 eşleşmesi");
        assert_eq!(
            normalized.method, "whitespace",
            "boşluk farkı ayrı katman olarak raporlanır"
        );
        // Sıra deterministik: aynı sorgu aynı sırayı verir.
        let again = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“itirazın reddine karar verdi”".into(),
            })
            .expect("tekrar");
        assert_eq!(
            again.quotes[0]
                .matches
                .iter()
                .map(|m| m.precedent_id.as_str())
                .collect::<Vec<_>>(),
            ids
        );

        let missing = &report.quotes[1];
        assert_eq!(missing.state, "not_found");
        assert_eq!(missing.matched_precedents, 0);
        assert!(missing.matches.is_empty());
        assert_eq!(report.found, 1);
        assert_eq!(report.not_found, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("güncel") && n.contains("eski sürümler")));
    }

    /// F12-T14l: güncel metinde bulunamayan alıntı için emsallerin **eski
    /// sürümleri** de taranır; eşleşme eski sürüm olarak işaretlenir ve aynı
    /// emsal iki kez eşleşmez.
    #[test]
    fn library_search_falls_back_to_historical_versions_and_marks_the_match() {
        let db = db();
        let p = new_precedent(&db, "2024/410");
        let old_version =
            add_precedent_text(&db, &p.id, "Eski nüsha: kira bedellini ödemeyen kiracı.");
        let current_version =
            add_precedent_text(&db, &p.id, "Yeni nüsha: itirazın reddine karar verildi.");
        let input = PrecedentInTextInput {
            text: "“kira bedellini ödemeyen kiracı” pasajına dayanılmıştır.".into(),
        };

        let library = db
            .check_in_text_quotes_across_library(&input)
            .expect("kütüphane");
        assert_eq!(library.quotes[0].state, "found");
        assert_eq!(library.quotes[0].matched_precedents, 1);
        assert_eq!(
            library.quotes[0].matches.len(),
            1,
            "aynı emsal iki kez eşleşmez"
        );
        let hit = &library.quotes[0].matches[0];
        assert_eq!(hit.precedent_id, p.id);
        assert!(hit.found_in_old_version, "eşleşme eski sürümde");
        assert_eq!(hit.version_id, old_version, "güncel değil eski sürüm");
        assert_ne!(hit.version_id, current_version);
        assert_eq!(hit.method, "exact");
        assert_eq!(hit.source_text, "kira bedellini ödemeyen kiracı");
        assert_eq!(library.scanned, 1, "emsal başına tek güncel metin");
        assert_eq!(library.history_scanned, 1);
        assert!(!library.history_truncated);
        assert!(library
            .notes
            .iter()
            .any(|n| n.contains("eski sürüm") && n.contains("1 eşleşme")));
        // Tek emsal yolu aynı alıntıyı yine bulur (tek yol, aynı katman).
        let single = db
            .check_in_text_quotes_in_precedent(&p.id, &input)
            .expect("tek emsal");
        assert_eq!(single.quotes[0].state, "exact");
        assert!(single.quotes[0].found_in_old_version);
    }

    /// Güncel metinde bulunan alıntı için geçmiş **hiç** taranmaz; eşleşme
    /// güncel sürümü gösterir (aynı emsal için iki satır doğmaz).
    #[test]
    fn library_search_prefers_the_current_text_and_skips_history_when_found() {
        let db = db();
        let p = new_precedent(&db, "2024/411");
        add_precedent_text(&db, &p.id, "Eski nüsha: itirazın reddine karar verdi.");
        let current = add_precedent_text(&db, &p.id, "Güncel nüsha: itirazın reddine karar verdi.");
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“itirazın reddine karar verdi”".into(),
            })
            .expect("kütüphane");
        assert_eq!(report.quotes[0].state, "found");
        assert_eq!(report.quotes[0].matches.len(), 1);
        assert_eq!(report.quotes[0].matches[0].version_id, current);
        assert!(!report.quotes[0].matches[0].found_in_old_version);
        assert_eq!(
            report.history_scanned, 0,
            "güncel metinde bulunan emsalin geçmişi taranmaz"
        );
        assert!(!report.history_truncated);
    }

    /// Aynı emsalde alıntı **birden çok eski sürümde** geçse bile tek satır
    /// yazılır: en yeni eşleşen eski sürüm gösterilir (deterministik sıra).
    #[test]
    fn library_search_reports_one_match_per_precedent_newest_historical_first() {
        let db = db();
        let p = new_precedent(&db, "2024/414");
        let older = add_precedent_text(&db, &p.id, "Birinci nüsha: itirazın reddine karar verdi.");
        let newer = add_precedent_text(&db, &p.id, "İkinci nüsha: itirazın reddine karar verdi.");
        add_precedent_text(&db, &p.id, "Güncel nüsha: bambaşka bir metin.");
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“itirazın reddine karar verdi”".into(),
            })
            .expect("kütüphane");
        assert_eq!(report.quotes[0].state, "found");
        assert_eq!(report.quotes[0].matches.len(), 1);
        assert_eq!(report.quotes[0].matched_precedents, 1);
        assert_eq!(report.quotes[0].matches[0].version_id, newer);
        assert_ne!(report.quotes[0].matches[0].version_id, older);
        assert!(report.quotes[0].matches[0].found_in_old_version);
    }

    /// Emsal başına **5** eski sürüm sınırı: altıncı (en eski) taranmaz ve
    /// rapor «bulunamadı» demez — `partial_scan` + dürüst not.
    #[test]
    fn library_search_caps_historical_versions_per_precedent_and_stays_uncertain() {
        let db = db();
        let p = new_precedent(&db, "2024/412");
        // En eski nüsha alıntıyı taşır; üstüne beş ara nüsha + güncel metin.
        add_precedent_text(&db, &p.id, "En eski nüsha: kira bedellini ödemeyen kiracı.");
        for index in 0..5 {
            add_precedent_text(&db, &p.id, &format!("Ara nüsha {index}: başka bir metin."));
        }
        add_precedent_text(&db, &p.id, "Güncel nüsha: itirazın reddine karar verildi.");
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“kira bedellini ödemeyen kiracı”".into(),
            })
            .expect("kütüphane");
        assert_eq!(report.quotes[0].state, "partial_scan");
        assert!(report.history_truncated, "altıncı sürüm taranmadı");
        assert_eq!(report.history_scanned, 5);
        assert_eq!(report.not_found, 0, "kesin «bulunamadı» yok");
        assert_eq!(report.partial_scan, 1);
        assert!(report.notes.iter().any(|n| n.contains("sınıra takıldı")));
    }

    /// Toplam karakter bütçesi (`MAX_LIBRARY_HISTORY_CHARS`): bütçeyi aşan
    /// eski sürüm taranmaz; alıntı orada olsa bile hüküm `partial_scan` olur
    /// (sessiz «bulunamadı» yok).
    #[test]
    fn library_search_budget_limits_history_scan_and_keeps_not_found_uncertain() {
        let db = db();
        let p = new_precedent(&db, "2024/413");
        // En eski sürüm alıntıyı taşır ama bütçe yetmez; araya büyük (alıntısız)
        // nüsha girer.
        add_precedent_text(&db, &p.id, "Eski nüsha: kira bedellini ödemeyen kiracı.");
        let filler = "d".repeat(MAX_LIBRARY_HISTORY_CHARS);
        add_precedent_text(&db, &p.id, &filler);
        add_precedent_text(&db, &p.id, "Güncel nüsha: itirazın reddine karar verildi.");
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“kira bedellini ödemeyen kiracı”".into(),
            })
            .expect("kütüphane");
        assert!(report.history_truncated, "bütçe aşıldı");
        assert_eq!(
            report.history_scanned, 1,
            "yalnız bütçeye sığan nüsha tarandı"
        );
        assert_eq!(report.quotes[0].state, "partial_scan");
        assert_eq!(report.not_found, 0, "kesin «bulunamadı» yok");
    }

    #[test]
    fn library_search_without_any_full_text_is_unverifiable_not_not_found() {
        let db = db();
        let p = new_precedent(&db, "2024/420");
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“itirazın reddine karar verdi” deniyor.".into(),
            })
            .expect("kütüphane");
        assert_eq!(report.library_total, 0);
        assert_eq!(report.scanned, 0);
        assert_eq!(report.no_library_text, 1);
        assert_eq!(report.quotes[0].state, "no_library_text");
        assert_ne!(report.quotes[0].state, "not_found");
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("tam metni olan emsal yok")));
        // Emsal var ama tam metni yok: yine «kütüphane metni yok».
        assert!(db.get_precedent(&p.id).expect("emsal").is_some());
    }

    #[test]
    fn library_search_marks_a_capped_scan_as_partial_and_counts_hidden_matches() {
        let db = db();
        // Sınır + 1 emsal: biri taranmaz. Alıntı A hepsinde var → bulunur;
        // alıntı B hiçbirinde yok → sınır yüzünden «kesin yok» denemez.
        for index in 0..=MAX_LIBRARY_PRECEDENTS {
            let p = new_precedent(&db, &format!("2024/5{index:03}"));
            add_precedent_text(
                &db,
                &p.id,
                "Karar metni: itirazın reddine karar verildi ve sonuçlandı.",
            );
        }
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“itirazın reddine karar verildi” · “hiçbir kararda olmayan pasaj bu”".into(),
            })
            .expect("kütüphane");
        assert_eq!(report.library_total, MAX_LIBRARY_PRECEDENTS + 1);
        assert_eq!(report.scanned, MAX_LIBRARY_PRECEDENTS);
        assert!(report.truncated);
        assert_eq!(report.quotes[0].state, "found");
        assert_eq!(
            report.quotes[0].matched_precedents,
            i64::try_from(MAX_LIBRARY_PRECEDENTS).unwrap()
        );
        assert_eq!(
            report.quotes[0].matches.len(),
            MAX_LIBRARY_MATCHES_PER_QUOTE
        );
        assert_eq!(
            report.quotes[0].hidden_matches,
            i64::try_from(MAX_LIBRARY_PRECEDENTS - MAX_LIBRARY_MATCHES_PER_QUOTE).unwrap()
        );
        assert_eq!(report.quotes[1].state, "partial_scan");
        assert_ne!(report.quotes[1].state, "not_found");
        assert!(report.notes.iter().any(|n| n.contains("kesin değildir")));
    }

    #[test]
    fn library_search_enforces_the_same_text_and_quote_limits() {
        let db = db();
        let mut many = String::new();
        for index in 0..31 {
            many.push_str(&format!("“alıntı numarası {index} burada” "));
        }
        let error = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput { text: many })
            .expect_err("31 alıntı");
        assert_eq!(error.code(), "E_QUOTE_TOO_MANY");
        let long = "a".repeat(MAX_IN_TEXT_CHECK_CHARS + 1);
        let error = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput { text: long })
            .expect_err("uzun metin");
        assert_eq!(error.code(), "E_QUOTE_TEXT");
    }

    #[test]
    fn library_search_finds_elided_quotes_and_keeps_other_precedents_clean() {
        let db = db();
        let p1 = new_precedent(&db, "2024/321");
        add_precedent_text(
            &db,
            &p1.id,
            "Karar: sözleşmenin haklı nedenle feshedilebileceğine ve kiracının kira \
             bedelini ödemekle temerrüde düştüğüne hükmedildi.",
        );
        let p2 = new_precedent(&db, "2024/322");
        add_precedent_text(&db, &p2.id, "Başka karar: kira bedeli ödenmedi.");
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“sözleşmenin haklı nedenle feshedilebileceğine … kiracının kira \
                       bedelini ödemekle temerrüde düştüğüne” geçiyor."
                    .into(),
            })
            .expect("kütüphane");
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "found");
        assert_eq!(finding.matched_precedents, 1);
        assert_eq!(finding.matches[0].precedent_id, p1.id);
        assert_eq!(finding.matches[0].method, "elided");
        assert_eq!(report.elided, 1);
        assert!(report.notes.iter().any(|n| n.contains("kısaltılmış")));
    }

    #[test]
    fn library_search_finds_edge_clipped_quotes() {
        let db = db();
        let p1 = new_precedent(&db, "2024/325");
        add_precedent_text(
            &db,
            &p1.id,
            "Karar: sözleşmenin haklı nedenle feshedilebileceğine ve kiracının kira \
             bedelini ödemekle temerrüde düştüğüne hükmedildi.",
        );
        let p2 = new_precedent(&db, "2024/326");
        add_precedent_text(&db, &p2.id, "Başka karar: kira bedeli ödenmedi.");
        let report = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: "“...kiracının kira bedelini ödemekle temerrüde düştüğüne” geçiyor.".into(),
            })
            .expect("kütüphane");
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "found");
        assert_eq!(finding.matched_precedents, 1);
        assert_eq!(finding.matches[0].precedent_id, p1.id);
        assert_eq!(finding.matches[0].method, "elided");
        assert_eq!(report.elided, 1);
        assert!(report
            .notes
            .iter()
            .any(|n| n.contains("başı/sonu kırpılmış")));
    }

    // ---- F12-T14r: kesme işareti yazımı (’ ↔ ') alıntıyı düşürmez ----

    const APOSTROPHE_SOURCE: &str =
        "Anayasa’nın 2. maddesinde sözleşme özgürlüğü güvence altındadır.";

    #[test]
    fn apostrophe_variants_are_matched_without_letter_changes() {
        let db = db();
        let case_id = new_case(&db, "Kesme işareti");
        add_document(&db, &case_id, "Karar", &[APOSTROPHE_SOURCE]);
        let report = check(
            &db,
            &case_id,
            "“Anayasa'nın 2. maddesinde sözleşme özgürlüğü” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(finding.occurrences, 1);
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "apostrophe");
        assert_eq!(
            position.source_text,
            "Anayasa’nın 2. maddesinde sözleşme özgürlüğü"
        );
        assert_eq!(report.normalized, 1);
        assert_eq!(report.exact, 0);
        // Kesme işareti eşleşmesi birebir değildir: anchor yolu kanıt ister.
        let error = db
            .create_anchor(&NewAnchor {
                case_id: case_id.clone(),
                document_version_id: position.document_version_id.clone(),
                parse_run_id: position.parse_run_id.clone(),
                segment_id: position.segment_id.clone().expect("segment"),
                start_offset: position.char_start,
                end_offset: position.char_start
                    + i64::try_from(finding.quote.chars().count()).unwrap_or(i64::MAX),
                exact_quote: finding.quote.clone(),
            })
            .expect_err("kesme işareti farkı bağlanmamalı");
        assert_eq!(error.code(), "E_QUOTE_NOT_FOUND");
    }

    #[test]
    fn apostrophe_variants_are_equated_in_both_directions() {
        let db = db();
        let case_id = new_case(&db, "Kesme ters yön");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["Anayasa'nın 2. maddesinde sözleşme özgürlüğü güvence altındadır."],
        );
        let report = check(
            &db,
            &case_id,
            "“Anayasa’nın 2. maddesinde sözleşme özgürlüğü” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(
            finding.position.as_ref().expect("position").match_kind,
            "apostrophe"
        );
    }

    #[test]
    fn apostrophe_folding_combines_with_soft_hyphen_join() {
        let db = db();
        let case_id = new_case(&db, "Kesme + tire");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["Anayasa’nın sözleşme özgürlü-\ngü güvence altındadır."],
        );
        let report = check(
            &db,
            &case_id,
            "“Anayasa'nın sözleşme özgürlüğü güvence altındadır” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(
            finding.position.as_ref().expect("position").match_kind,
            "dehyphenated"
        );
    }

    #[test]
    fn apostrophe_folding_combines_with_letter_folding() {
        let db = db();
        let case_id = new_case(&db, "Kesme + harf");
        add_document(
            &db,
            &case_id,
            "Karar",
            &["anayasa’nın 2. maddesinde sözleşme özgürlüğü güvence altındadır."],
        );
        let report = check(
            &db,
            &case_id,
            "“Anayasa'nın 2. maddesinde sözleşme özgürlüğü” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(
            finding.position.as_ref().expect("position").match_kind,
            "turkish_fold"
        );
    }

    #[test]
    fn elided_quote_with_apostrophe_variants_is_found() {
        let db = db();
        let case_id = new_case(&db, "Kesme + kırpık");
        add_document(&db, &case_id, "Karar", &[APOSTROPHE_SOURCE]);
        let report = check(
            &db,
            &case_id,
            "“...Anayasa'nın 2. maddesinde sözleşme özgürlüğü güvence altındadır” deniyor.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "elided");
        assert_eq!(
            position.source_text,
            APOSTROPHE_SOURCE.trim_end_matches('.')
        );
        assert_eq!(report.elided, 1);
    }

    #[test]
    fn page_spanning_quote_with_apostrophe_variants_is_found() {
        let db = db();
        let case_id = new_case(&db, "Kesme + sayfa sınırı");
        add_document(
            &db,
            &case_id,
            "Karar",
            &[
                "Anayasa’nın 2. maddesinde sözleşme",
                "özgürlüğü güvence altındadır.",
            ],
        );
        let report = check(
            &db,
            &case_id,
            "“Anayasa'nın 2. maddesinde sözleşme özgürlüğü güvence altındadır” ifadesi.",
            None,
        );
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        let position = finding.position.as_ref().expect("position");
        assert_eq!(position.match_kind, "apostrophe");
        assert!(position.spans_pages);
    }

    #[test]
    fn apostrophes_are_not_deleted_only_equated() {
        let db = db();
        let case_id = new_case(&db, "Kesme silinmez");
        add_document(&db, &case_id, "Karar", &[APOSTROPHE_SOURCE]);
        // Kesme işareti hiç yazılmamış alıntı, kaynaktaki `’` ile eşitlenmez.
        let report = check(
            &db,
            &case_id,
            "“Anayasanın 2. maddesinde sözleşme özgürlüğü” ifadesi.",
            None,
        );
        assert_eq!(report.quotes[0].state, "not_found");
        assert_eq!(report.normalized, 0);
    }

    #[test]
    fn precedent_paths_report_the_apostrophe_method() {
        let db = db();
        let p = new_precedent(&db, "2024/327");
        add_precedent_text(&db, &p.id, APOSTROPHE_SOURCE);
        let quote = "Anayasa'nın 2. maddesinde sözleşme özgürlüğü";
        let single = db.verify_precedent_quote(&p.id, quote).expect("tek alıntı");
        assert_eq!(single.method.as_deref(), Some("apostrophe"));
        let report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: format!("“{quote}” deniyor."),
                },
            )
            .expect("metin içi");
        let finding = &report.quotes[0];
        assert_eq!(finding.state, "normalized");
        assert_eq!(finding.method.as_deref(), Some("apostrophe"));
        assert_eq!(single.occurrences, finding.occurrences);
        // Harf farkı kesme işaretiyle birleşirse katman `turkish_fold` olur —
        // emsal yolu da aynı sonucu vermeli (tek kural, üç yol).
        let lowered = "anayasa'nın 2. maddesinde sözleşme özgürlüğü";
        let fold_single = db
            .verify_precedent_quote(&p.id, lowered)
            .expect("harf + kesme");
        assert_eq!(fold_single.method.as_deref(), Some("turkish_fold"));
        let fold_report = db
            .check_in_text_quotes_in_precedent(
                &p.id,
                &PrecedentInTextInput {
                    text: format!("“{lowered}” deniyor."),
                },
            )
            .expect("harf + kesme metin içi");
        assert_eq!(
            fold_report.quotes[0].method.as_deref(),
            Some("turkish_fold")
        );
        let library = db
            .check_in_text_quotes_across_library(&PrecedentInTextInput {
                text: format!("“{quote}” deniyor."),
            })
            .expect("kütüphane");
        assert_eq!(library.quotes[0].state, "found");
        assert_eq!(library.quotes[0].matches[0].method.as_str(), "apostrophe");
    }
}
