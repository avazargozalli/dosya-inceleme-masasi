//! §17.3 **kaynak bütçesi**: worker başına CPU, bellek, çıktı, açık dosya ve süre
//! sınırları; **ayrı hata kodu** ile raporlanır.
//!
//! Şartname: *"Worker başına CPU zamanı, bellek, çıktı boyutu, açık dosya sayısı ve
//! süre bütçesi tanımlanır. Salt bir 'timeout' değeri bütün belgeler için
//! kullanılmaz; render, OCR, redaksiyon ve AI için ayrı profil vardır. Bütçe aşımı
//! ayrı hata kodu taşır."*
//!
//! Bu modül **politikadır**: sınırlar tek yerde tanımlıdır, uygulaması
//! `worker.rs`tedir (POSIX kabuk zarfı + bellek gözcüsü + çıktı sayacı). Kural
//! kopyalanmaz: Tauri katmanı yalnız `WorkerKind` seçer.
//!
//! Başlangıç değerleri (F15 ölçümüyle değişebilir; §17.3 bunu açıkça söyler):
//! - render / metin çıkarma: küçük iş, dar bellek
//! - OCR: en pahalı iş (sayfa başına rasterleştirme) → en geniş süre/bellek
//! - redaksiyon: belge kopyası + doğrulama → geniş çıktı
//! - AI: süreç yok; ölçülen şey **duvar saati + yanıt boyutu** (model belleği
//!   çekirdek hedefinden ayrı raporlanır)
//!
//! Sıfır (`0`) "bu boyut uygulanmaz" demektir; sıfır sınır "sınırsız" diye
//! yorumlanmaz, uygulanmaz.

use std::time::Duration;

/// MiB → bayt. Sınırlar MiB cinsinden yazılır, karşılaştırma baytla yapılır.
pub const MIB: u64 = 1024 * 1024;

/// Worker türü. §17.3 dört profil adı verir (render, OCR, redaksiyon, AI);
/// metin çıkarma beşinci profildir çünkü en sık koşan yoldur ve OCR'dan
/// belirgin biçimde ucuzdur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkerKind {
    /// Sayfa önizlemesi (PNG) ve rapor çıktısı.
    Render,
    /// Metin çıkarma / envanter (arama indeksinin girdisi).
    Extract,
    /// OCR açık metin çıkarma (sayfa başına rasterleştirme).
    Ocr,
    /// Metin ve bölge redaksiyonu + bağımsız doğrulama.
    Redaction,
    /// Model çağrısı (yerel ya da bulut); süreç yok, sınır duvar saati + çıktı.
    Ai,
}

impl WorkerKind {
    /// Arayüzde listelenen bütün profiller; sıra ağırlık sırasıdır.
    pub const ALL: [Self; 5] = [
        Self::Render,
        Self::Extract,
        Self::Ocr,
        Self::Redaction,
        Self::Ai,
    ];

    /// Kararlı, küçük harfli kimlik (denetim kaydı ve arayüz anahtarı).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Extract => "extract",
            Self::Ocr => "ocr",
            Self::Redaction => "redaction",
            Self::Ai => "ai",
        }
    }

    /// Kullanıcıya gösterilen ad.
    #[must_use]
    pub const fn label_tr(self) -> &'static str {
        match self {
            Self::Render => "Sayfa önizleme",
            Self::Extract => "Metin çıkarma",
            Self::Ocr => "OCR",
            Self::Redaction => "Redaksiyon",
            Self::Ai => "AI çağrısı",
        }
    }
}

/// Bütçenin hangi boyutu aşıldı. Her boyut **aynı** hata kodunu
/// (`E_WORKER_BUDGET`) taşır ama kullanıcıya farklı cümle söyler ve testler
/// boyutu ayırt eder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDimension {
    /// Süreç CPU süresi (POSIX `ulimit -t`; aşımda `SIGXCPU`).
    Cpu,
    /// Yerleşik bellek (gözcü örneklemesi; macOS/Linux).
    Memory,
    /// İstek başına yanıt baytı.
    Output,
    /// Açık dosya tanıtıcısı sayısı (POSIX `ulimit -n`).
    OpenFiles,
    /// Duvar saati (süre bütçesi).
    Time,
}

impl BudgetDimension {
    /// Kararlı kimlik (denetim/arayüz).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Output => "output",
            Self::OpenFiles => "open_files",
            Self::Time => "time",
        }
    }

    /// Kullanıcıya gösterilen boyut adı.
    #[must_use]
    pub const fn label_tr(self) -> &'static str {
        match self {
            Self::Cpu => "CPU süresi",
            Self::Memory => "bellek",
            Self::Output => "çıktı boyutu",
            Self::OpenFiles => "açık dosya sayısı",
            Self::Time => "süre",
        }
    }

    /// Sınırı birimiyle yazar (`120 sn`, `1024 MiB`, `256 dosya`).
    #[must_use]
    pub fn limit_text(self, limit: u64) -> String {
        match self {
            Self::Cpu | Self::Time => format!("{limit} sn"),
            Self::Memory | Self::Output => format!("{} MiB", limit / MIB),
            Self::OpenFiles => format!("{limit} dosya"),
        }
    }
}

/// Bütçe aşımının ayrıntısı: hangi boyut, hangi sınır. Hata metni buradan
/// üretilir; çağıran sınırı yeniden hesaplamaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetBreak {
    pub dimension: BudgetDimension,
    pub limit: u64,
}

impl std::fmt::Display for BudgetBreak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (sınır {})",
            self.dimension.label_tr(),
            self.dimension.limit_text(self.limit)
        )
    }
}

/// Tek bir worker işinin kaynak bütçesi. Bütün boyutlar **mutlak** sınırdır;
/// "kalan süre" gibi türetilmiş değer taşımaz (ölçüm tek yerde).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerBudget {
    pub kind: WorkerKind,
    /// POSIX `ulimit -t` saniye. `0` → CPU sınırı uygulanmaz.
    pub cpu_seconds: u64,
    /// Yerleşik bellek tavanı (bayt). `0` → gözcü koşmaz.
    pub memory_bytes: u64,
    /// İstek başına yanıt baytı tavanı. `0` → çıktı sayılmaz.
    pub output_bytes: u64,
    /// POSIX `ulimit -n`. `0` → sınır uygulanmaz.
    pub open_files: u32,
    /// Duvar saati bütçesi (mevcut `DEFAULT_TIMEOUT` yerine geçer).
    pub wall_clock: Duration,
}

impl WorkerBudget {
    /// Türün başlangıç profili. Değerler §17.3'ün "başlangıç ayarları"
    /// çerçevesindedir: F15 ölçümü geldiğinde burada değişir, çağıranlar değişmez.
    #[must_use]
    pub const fn for_kind(kind: WorkerKind) -> Self {
        match kind {
            WorkerKind::Render => Self {
                kind,
                cpu_seconds: 30,
                memory_bytes: 1024 * MIB,
                output_bytes: 24 * MIB,
                open_files: 128,
                wall_clock: Duration::from_secs(60),
            },
            WorkerKind::Extract => Self {
                kind,
                cpu_seconds: 120,
                memory_bytes: 1024 * MIB,
                output_bytes: 32 * MIB,
                open_files: 256,
                wall_clock: Duration::from_secs(300),
            },
            WorkerKind::Ocr => Self {
                kind,
                cpu_seconds: 600,
                memory_bytes: 2048 * MIB,
                output_bytes: 48 * MIB,
                open_files: 512,
                wall_clock: Duration::from_secs(900),
            },
            WorkerKind::Redaction => Self {
                kind,
                cpu_seconds: 300,
                memory_bytes: 2048 * MIB,
                output_bytes: 64 * MIB,
                open_files: 256,
                wall_clock: Duration::from_secs(600),
            },
            WorkerKind::Ai => Self {
                kind,
                // Model çağrısı yerel CPU tüketmez: CPU sınırı uygulanmaz.
                cpu_seconds: 0,
                memory_bytes: 512 * MIB,
                output_bytes: 8 * MIB,
                open_files: 32,
                wall_clock: Duration::from_secs(120),
            },
        }
    }

    /// Sınırı aşan boyutun sınır değeri (hata metni ve rapor için tek kaynak).
    #[must_use]
    pub const fn limit_for(&self, dimension: BudgetDimension) -> u64 {
        match dimension {
            BudgetDimension::Cpu => self.cpu_seconds,
            BudgetDimension::Memory => self.memory_bytes,
            BudgetDimension::Output => self.output_bytes,
            BudgetDimension::OpenFiles => self.open_files as u64,
            BudgetDimension::Time => self.wall_clock.as_secs(),
        }
    }

    /// POSIX `sh` zarfı: sınırları **çocuk sürece** uygular, sonra süreci
    /// `exec` ile devralır (pid değişmez, boru sahibi değişmez).
    ///
    /// `exec "$0" "$@"` biçimi bilinçli: program ve argümanlar **kabuk
    /// yorumlamasına** girmez (enjeksiyon yok), yalnız konumsal parametre olur.
    /// Sınır uygulanamazsa süreç hiç başlamaz (`exit 126`): sessizce sınırsız
    /// çalışan bir worker "bütçeli" sayılmaz.
    #[must_use]
    pub fn limit_script(&self) -> String {
        let mut script = String::new();
        if self.cpu_seconds > 0 {
            script.push_str(&format!("ulimit -t {} || exit 126; ", self.cpu_seconds));
        }
        if self.open_files > 0 {
            script.push_str(&format!("ulimit -n {} || exit 126; ", self.open_files));
        }
        script.push_str("exec \"$0\" \"$@\"");
        script
    }
}

/// Bir iş boyunca **gözlenen** kullanım. Sayaçlar tek yönlüdür: tepe bellek ve
/// toplam çıktı yalnız artar.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BudgetUsage {
    /// İstek başına toplam yanıt baytı.
    pub output_bytes: u64,
    /// Gözlenen en yüksek yerleşik bellek.
    pub peak_memory_bytes: u64,
    /// İlk aşılan boyut; sonraki boyutlar onu **ezmez** (ilk neden raporlanır).
    pub violated: Option<BudgetDimension>,
}

impl BudgetUsage {
    /// Yanıt baytını sayar; sınır aşıldıysa ihlali işaretler.
    pub fn note_output(&mut self, bytes: u64, limit: u64) -> Option<BudgetDimension> {
        self.output_bytes = self.output_bytes.saturating_add(bytes);
        if limit > 0 && self.output_bytes > limit {
            self.mark(BudgetDimension::Output);
        }
        self.violated
    }

    /// Bellek örneğini kaydeder; tepeyi günceller, sınır aşıldıysa işaretler.
    pub fn note_memory(&mut self, bytes: u64, limit: u64) -> Option<BudgetDimension> {
        self.peak_memory_bytes = self.peak_memory_bytes.max(bytes);
        if limit > 0 && bytes > limit {
            self.mark(BudgetDimension::Memory);
        }
        self.violated
    }

    /// Yalnız tepeyi günceller (ihlal işaretlemez). İş **bittikten sonra** alınan
    /// örnekte kullanılır: artık durdurulacak bir iş yok, ama "0 MiB" yazmak da
    /// yanlış olurdu.
    pub fn note_peak(&mut self, bytes: u64) {
        self.peak_memory_bytes = self.peak_memory_bytes.max(bytes);
    }

    /// İşi sıfırlar (yeni istek); tepe bellek **korunur** (süreç aynı).
    pub fn reset_output(&mut self) {
        self.output_bytes = 0;
    }

    /// Bir boyutu ihlal olarak işaretler (ilk neden korunur). Süreç sinyaliyle
    /// (`SIGXCPU`) veya duvar saatiyle tespit edilen ihlallerde çağıran kullanır:
    /// sayaç değil, **karar** burada tek yerde tutulur.
    pub fn mark_violation(&mut self, dimension: BudgetDimension) -> Option<BudgetDimension> {
        self.mark(dimension);
        self.violated
    }

    fn mark(&mut self, dimension: BudgetDimension) {
        if self.violated.is_none() {
            self.violated = Some(dimension);
        }
    }
}

/// İş bitince arayüze/denetime taşınan özet. Yalnız **sayı** taşır: belge adı,
/// yol veya içerik yok.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetReport {
    pub budget: WorkerBudget,
    /// Süreç başlatmadan bu yana geçen süre.
    pub elapsed: Duration,
    /// İstek başına toplam yanıt baytı (son istek).
    pub output_bytes: u64,
    /// Gözlenen tepe yerleşik bellek (ölçülemediyse `0`).
    pub peak_memory_bytes: u64,
    /// Aşılan boyut (varsa).
    pub violated: Option<BudgetDimension>,
}

impl BudgetReport {
    /// Bütçe aşımı olduysa hata ayrıntısı.
    #[must_use]
    pub fn budget_break(&self) -> Option<BudgetBreak> {
        self.violated.map(|dimension| BudgetBreak {
            dimension,
            limit: self.budget.limit_for(dimension),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profile_defines_all_five_limits() {
        for kind in WorkerKind::ALL {
            let budget = WorkerBudget::for_kind(kind);
            assert_eq!(budget.kind, kind);
            assert!(
                budget.wall_clock.as_secs() > 0,
                "{}: süre bütçesi",
                kind.code()
            );
            assert!(budget.memory_bytes > 0, "{}: bellek bütçesi", kind.code());
            assert!(budget.output_bytes > 0, "{}: çıktı bütçesi", kind.code());
            assert!(budget.open_files > 0, "{}: dosya bütçesi", kind.code());
            // AI süreç değil: CPU sınırı bilinçli olarak yok.
            if kind != WorkerKind::Ai {
                assert!(budget.cpu_seconds > 0, "{}: CPU bütçesi", kind.code());
            }
        }
    }

    #[test]
    fn profiles_are_ordered_by_weight_and_not_interchangeable() {
        let render = WorkerBudget::for_kind(WorkerKind::Render);
        let extract = WorkerBudget::for_kind(WorkerKind::Extract);
        let ocr = WorkerBudget::for_kind(WorkerKind::Ocr);
        assert!(
            render.wall_clock < extract.wall_clock,
            "önizleme çıkarmadan kısa"
        );
        assert!(extract.wall_clock < ocr.wall_clock, "OCR en uzun");
        assert!(render.memory_bytes <= extract.memory_bytes);
        assert!(extract.memory_bytes < ocr.memory_bytes);
        // Aynı boyutlar türle birlikte taşınır: profil tek kaynaktır.
        assert_eq!(ocr, WorkerBudget::for_kind(WorkerKind::Ocr));
        assert_ne!(extract, ocr);
    }

    #[test]
    fn limit_script_applies_limits_then_execs_without_shell_expansion() {
        let budget = WorkerBudget::for_kind(WorkerKind::Extract);
        let script = budget.limit_script();
        assert!(script.contains("ulimit -t 120"), "script: {script}");
        assert!(script.contains("ulimit -n 256"), "script: {script}");
        assert!(script.ends_with("exec \"$0\" \"$@\""), "script: {script}");
        // Sınır uygulanamazsa süreç başlamaz: sessiz sınırsız çalışma yok.
        assert_eq!(script.matches("exit 126").count(), 2, "script: {script}");
        // CPU sırası dosya sınırından önce gelir.
        assert!(
            script.find("ulimit -t").unwrap() < script.find("ulimit -n").unwrap(),
            "script: {script}"
        );
    }

    #[test]
    fn ai_profile_has_no_cpu_limit_in_the_script() {
        let script = WorkerBudget::for_kind(WorkerKind::Ai).limit_script();
        assert!(!script.contains("ulimit -t"), "script: {script}");
        assert!(script.contains("ulimit -n 32"), "script: {script}");
    }

    #[test]
    fn zero_limits_are_not_applied() {
        let mut budget = WorkerBudget::for_kind(WorkerKind::Render);
        budget.cpu_seconds = 0;
        budget.open_files = 0;
        assert_eq!(budget.limit_script(), "exec \"$0\" \"$@\"");
    }

    #[test]
    fn output_counter_flags_only_after_the_limit_is_crossed() {
        let mut usage = BudgetUsage::default();
        assert_eq!(usage.note_output(100, 200), None);
        assert_eq!(usage.output_bytes, 100);
        // Tam sınır aşım değildir.
        assert_eq!(usage.note_output(100, 200), None);
        assert_eq!(usage.output_bytes, 200);
        assert_eq!(usage.note_output(1, 200), Some(BudgetDimension::Output));
        assert_eq!(usage.output_bytes, 201);
        // Sıfır sınır "sınırsız" değil, "uygulanmaz".
        let mut unbounded = BudgetUsage::default();
        assert_eq!(unbounded.note_output(10 * MIB, 0), None);
        assert_eq!(unbounded.violated, None);
    }

    #[test]
    fn memory_counter_keeps_peak_and_first_violation_wins() {
        let mut usage = BudgetUsage::default();
        assert_eq!(usage.note_memory(100 * MIB, 256 * MIB), None);
        assert_eq!(usage.peak_memory_bytes, 100 * MIB);
        // Küçük örnek tepeyi düşürmez.
        assert_eq!(usage.note_memory(50 * MIB, 256 * MIB), None);
        assert_eq!(usage.peak_memory_bytes, 100 * MIB);
        assert_eq!(
            usage.note_memory(300 * MIB, 256 * MIB),
            Some(BudgetDimension::Memory)
        );
        assert_eq!(usage.peak_memory_bytes, 300 * MIB);
        // Sonradan çıktı taşsa bile ilk neden (bellek) korunur.
        assert_eq!(
            usage.note_output(10 * MIB, MIB),
            Some(BudgetDimension::Memory)
        );
    }

    #[test]
    fn reset_output_clears_the_request_counter_but_keeps_the_peak() {
        let mut usage = BudgetUsage::default();
        usage.note_memory(200 * MIB, 1024 * MIB);
        usage.note_output(5 * MIB, 0);
        assert_eq!(usage.output_bytes, 5 * MIB);
        usage.reset_output();
        assert_eq!(usage.output_bytes, 0);
        assert_eq!(usage.peak_memory_bytes, 200 * MIB);
        assert_eq!(usage.violated, None);
    }

    #[test]
    fn mark_violation_keeps_the_first_cause() {
        let mut usage = BudgetUsage::default();
        assert_eq!(
            usage.mark_violation(BudgetDimension::Cpu),
            Some(BudgetDimension::Cpu)
        );
        // Sonraki ihlal ilk nedeni ezmez (rapor tek neden taşır).
        assert_eq!(
            usage.mark_violation(BudgetDimension::Memory),
            Some(BudgetDimension::Cpu)
        );
        assert_eq!(usage.violated, Some(BudgetDimension::Cpu));
    }

    #[test]
    fn limit_text_uses_the_dimension_unit() {
        assert_eq!(BudgetDimension::Cpu.limit_text(120), "120 sn");
        assert_eq!(BudgetDimension::Time.limit_text(900), "900 sn");
        assert_eq!(
            BudgetDimension::Memory.limit_text(2 * 1024 * MIB),
            "2048 MiB"
        );
        assert_eq!(BudgetDimension::Output.limit_text(24 * MIB), "24 MiB");
        assert_eq!(BudgetDimension::OpenFiles.limit_text(256), "256 dosya");
    }

    #[test]
    fn limit_for_matches_the_profile_and_the_error_text_is_specific() {
        let budget = WorkerBudget::for_kind(WorkerKind::Ocr);
        assert_eq!(budget.limit_for(BudgetDimension::Cpu), 600);
        assert_eq!(budget.limit_for(BudgetDimension::Memory), 2048 * MIB);
        assert_eq!(budget.limit_for(BudgetDimension::OpenFiles), 512);
        assert_eq!(budget.limit_for(BudgetDimension::Time), 900);

        let report = BudgetReport {
            budget,
            elapsed: Duration::from_secs(3),
            output_bytes: 0,
            peak_memory_bytes: 3 * 1024 * MIB,
            violated: Some(BudgetDimension::Memory),
        };
        let break_down = report.budget_break().expect("ihlal");
        assert_eq!(break_down.dimension, BudgetDimension::Memory);
        assert_eq!(break_down.to_string(), "bellek (sınır 2048 MiB)");

        let clean = BudgetReport {
            violated: None,
            ..report
        };
        assert!(clean.budget_break().is_none());
    }

    #[test]
    fn kind_codes_and_labels_are_stable_and_unique() {
        let mut codes: Vec<&str> = WorkerKind::ALL.iter().map(|kind| kind.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), WorkerKind::ALL.len());
        for kind in WorkerKind::ALL {
            assert!(!kind.label_tr().is_empty());
            assert_eq!(kind.code(), kind.code().to_lowercase());
        }
        // §17.3'ün adı geçen dört profil eksiksiz.
        for code in ["render", "ocr", "redaction", "ai"] {
            assert!(
                WorkerKind::ALL.iter().any(|kind| kind.code() == code),
                "eksik profil: {code}"
            );
        }
    }
}
