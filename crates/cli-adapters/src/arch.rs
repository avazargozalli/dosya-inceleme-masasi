//! F10-T01 — yürütülebilir **mimari uyum** denetimi.
//!
//! Yürütülebilir dosyanın başlık baytları okunur ve biçim + mimari çıkarılır:
//! Mach-O (thin/fat), ELF, PE ve `#!` betiği. **Süreç başlatılmaz**, dosya
//! çalıştırılmaz, ağ kullanılmaz; yalnız ilk 4 KiB (PE başlığı daha ilerideyse
//! o bölüm) okunur.
//!
//! Karşılaştırma uygulamanın **kendi** mimarisiyle yapılır
//! (`std::env::consts::ARCH` — süreç hangi mimari için derlendiyse o; Rosetta
//! altında çalışan x86_64 süreç kendini x86_64 bildirir). Sonuçlar:
//! `same` / `different` / `fat` / `unknown`.
//!
//! Betiklerde (`#!/usr/bin/env node` gibi) dosyanın kendisi mimari taşımaz:
//! yorumlayıcı (shebang) çözülür ve **onun** mimarisi raporlanır. Bu, "betik
//! natively çalışır mı" sorusunun dürüst cevabıdır; betiğin kendisi için
//! mimari uydurulmaz.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::find_on_path;

/// Başlık için okunan bayt sayısı. PE başlığı daha ilerideyse ayrıca okunur.
pub const HEADER_READ_BYTES: usize = 4096;
/// Mach-O fat başlığında kabul edilen en çok dilim (bozuk dosya koruması).
pub const MAX_FAT_SLICES: u32 = 20;
/// PE `e_lfanew` için üst sınır; daha büyük değer bozuk kabul edilir.
pub const MAX_PE_OFFSET: u64 = 1024 * 1024;
/// Shebang satırından okunan en çok karakter.
const MAX_SCRIPT_LINE: usize = 256;

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    MachO,
    Elf,
    Pe,
    /// `#!` ile başlayan betik; mimari yorumlayıcıdan gelir.
    Script,
    Unknown,
}

impl BinaryFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MachO => "mach_o",
            Self::Elf => "elf",
            Self::Pe => "pe",
            Self::Script => "script",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MachO => "Mach-O",
            Self::Elf => "ELF",
            Self::Pe => "PE",
            Self::Script => "betik (shebang)",
            Self::Unknown => "bilinmiyor",
        }
    }
}

/// Başlıktan okunan mimari. Tanınmayan makine numaraları `Other` olarak
/// **olduğu gibi** taşınır; bilinen bir mimariye çevrilmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryArch {
    X86_64,
    X86,
    Aarch64,
    Arm,
    Arm64_32,
    Riscv64,
    PowerPc,
    PowerPc64,
    Other(u32),
}

impl BinaryArch {
    #[must_use]
    pub fn as_str(self) -> String {
        match self {
            Self::X86_64 => "x86_64".to_string(),
            Self::X86 => "x86".to_string(),
            Self::Aarch64 => "aarch64".to_string(),
            Self::Arm => "arm".to_string(),
            Self::Arm64_32 => "arm64_32".to_string(),
            Self::Riscv64 => "riscv64".to_string(),
            Self::PowerPc => "powerpc".to_string(),
            Self::PowerPc64 => "powerpc64".to_string(),
            Self::Other(n) => format!("other({n})"),
        }
    }
}

/// Uygulamanın kendi mimarisi (`std::env::consts::ARCH`), kanonik etiketle.
#[must_use]
pub fn app_arch_label() -> String {
    match std::env::consts::ARCH {
        "x86_64" => BinaryArch::X86_64.as_str(),
        "aarch64" => BinaryArch::Aarch64.as_str(),
        "x86" | "i686" => BinaryArch::X86.as_str(),
        "arm" => BinaryArch::Arm.as_str(),
        "riscv64" => BinaryArch::Riscv64.as_str(),
        "powerpc" => BinaryArch::PowerPc.as_str(),
        "powerpc64" => BinaryArch::PowerPc64.as_str(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchComparison {
    /// Tek mimari ve uygulamayla aynı.
    Same,
    /// Tek mimari ama uygulamadan farklı (ör. arm64 CLI, x86_64 uygulama).
    Different,
    /// Mach-O evrensel (çok dilimli) ikili; ana mimariyi içerip içermediği
    /// `contains_app_arch` ile ayrıca raporlanır.
    Fat,
    /// Başlık okunamadı, betiğin yorumlayıcısı bulunamadı ya da mimari yok.
    Unknown,
}

impl ArchComparison {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Different => "different",
            Self::Fat => "fat",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Same => "aynı mimari",
            Self::Different => "farklı mimari",
            Self::Fat => "evrensel ikili",
            Self::Unknown => "mimari doğrulanamadı",
        }
    }
}

/// Betiğin yorumlayıcısı (shebang) ve onun mimarisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptInterpreter {
    /// Shebang'de yazılan komut (`node`, `/bin/sh`).
    pub command: String,
    /// Çözülen yol; bulunamadıysa `None`.
    pub path: Option<PathBuf>,
    /// Sembolik bağ çözümü.
    pub resolved: Option<PathBuf>,
    pub format: BinaryFormat,
    pub architectures: Vec<BinaryArch>,
    pub comparison: ArchComparison,
    pub contains_app_arch: Option<bool>,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchReport {
    pub path: PathBuf,
    /// Sembolik bağlar çözüldükten sonra okunan gerçek dosya.
    pub resolved: Option<PathBuf>,
    pub symlink: bool,
    pub format: BinaryFormat,
    pub architectures: Vec<BinaryArch>,
    /// Uygulamanın (sürecin) mimarisi.
    pub app_arch: String,
    pub comparison: ArchComparison,
    /// Evrensel ikilide ana mimarinin bulunup bulunmadığı; diğerlerinde
    /// `comparison` ile aynı anlamı taşır.
    pub contains_app_arch: Option<bool>,
    pub interpreter: Option<ScriptInterpreter>,
    /// Tanınmadıysa neden kodu (`magic_unrecognized`, `interpreter_unresolved`, …).
    pub reason: Option<&'static str>,
    pub header_bytes: usize,
}

impl ArchReport {
    /// Mimari uyum rozeti için kısa etiket.
    #[must_use]
    pub fn summary(&self) -> String {
        match self.comparison {
            ArchComparison::Fat => {
                let list = self
                    .architectures
                    .iter()
                    .map(|a| a.as_str())
                    .collect::<Vec<_>>()
                    .join(" + ");
                match self.contains_app_arch {
                    Some(true) => format!("evrensel: {list} (uygulama mimarisi var)"),
                    Some(false) => format!("evrensel: {list} (uygulama mimarisi yok)"),
                    None => format!("evrensel: {list}"),
                }
            }
            ArchComparison::Same | ArchComparison::Different => {
                let list = self
                    .architectures
                    .iter()
                    .map(|a| a.as_str())
                    .collect::<Vec<_>>()
                    .join(" + ");
                format!("{list} · {}", self.comparison.label())
            }
            ArchComparison::Unknown => self.reason.map_or_else(
                || "mimari doğrulanamadı".to_string(),
                |r| format!("mimari doğrulanamadı ({r})"),
            ),
        }
    }
}

/// Bir yolu inceler: biçim, mimari listesi ve uygulama mimarisiyle karşılaştırma.
/// Dosya çalıştırılmaz; hata durumunda `reason` kodu döner, panik yok.
#[must_use]
pub fn inspect_path(path: &Path) -> ArchReport {
    inspect_inner(path, 0)
}

fn inspect_inner(path: &Path, depth: usize) -> ArchReport {
    let app_arch = app_arch_label();
    let mut report = ArchReport {
        path: path.to_path_buf(),
        resolved: None,
        symlink: false,
        format: BinaryFormat::Unknown,
        architectures: Vec::new(),
        app_arch: app_arch.clone(),
        comparison: ArchComparison::Unknown,
        contains_app_arch: None,
        interpreter: None,
        reason: None,
        header_bytes: 0,
    };

    let Ok(link_meta) = std::fs::symlink_metadata(path) else {
        report.reason = Some("not_found");
        return report;
    };
    report.symlink = link_meta.file_type().is_symlink();
    let Ok(resolved) = std::fs::canonicalize(path) else {
        report.reason = Some("not_found");
        return report;
    };
    if !resolved.is_file() {
        report.reason = Some("not_a_file");
        return report;
    }
    report.resolved = Some(resolved.clone());

    let Ok(mut file) = File::open(&resolved) else {
        report.reason = Some("read_error");
        return report;
    };
    let mut buf = vec![0u8; HEADER_READ_BYTES];
    let Ok(read) = file.read(&mut buf) else {
        report.reason = Some("read_error");
        return report;
    };
    buf.truncate(read);
    report.header_bytes = read;

    let pe_tail = read_pe_tail(&mut file, &buf, read);
    let parsed = parse_header(&buf, pe_tail.as_deref());
    match parsed {
        Parsed::Known {
            format,
            architectures,
        } => {
            report.format = format;
            report.architectures = architectures;
        }
        Parsed::Unknown { reason } => {
            report.reason = Some(reason);
        }
    }

    if report.format == BinaryFormat::Script {
        apply_interpreter(&mut report, &buf, depth);
    }

    let fat = report.format == BinaryFormat::MachO && report.architectures.len() > 1;
    // Betikte karar yorumlayıcıdan gelir (evrensel yorumlayıcı → `fat`);
    // burada yeniden hesaplanırsa o bilgi kaybolur.
    if report.format != BinaryFormat::Script && !report.architectures.is_empty() {
        let (comparison, contains) = compare(&report.architectures, fat, &app_arch);
        report.comparison = comparison;
        report.contains_app_arch = contains;
    }

    if depth > 0 && report.format == BinaryFormat::Script {
        report.reason = Some("nested_script");
    }

    report
}

/// Shebang'i çözer, yorumlayıcıyı inceler ve sonucu rapora taşır.
fn apply_interpreter(report: &mut ArchReport, buf: &[u8], depth: usize) {
    let Some(line) = shebang_line(buf) else {
        report.reason = Some("empty_shebang");
        return;
    };
    let Some(command) = interpreter_command(&line) else {
        report.reason = Some("empty_shebang");
        return;
    };
    let Some(path) = interpreter_path(&command) else {
        report.interpreter = Some(ScriptInterpreter {
            command,
            path: None,
            resolved: None,
            format: BinaryFormat::Unknown,
            architectures: Vec::new(),
            comparison: ArchComparison::Unknown,
            contains_app_arch: None,
            reason: Some("interpreter_unresolved"),
        });
        report.reason = Some("interpreter_unresolved");
        return;
    };

    let inner = if depth > 1 {
        ArchReport {
            path: path.clone(),
            resolved: None,
            symlink: false,
            format: BinaryFormat::Unknown,
            architectures: Vec::new(),
            app_arch: report.app_arch.clone(),
            comparison: ArchComparison::Unknown,
            contains_app_arch: None,
            interpreter: None,
            reason: Some("interpreter_chain"),
            header_bytes: 0,
        }
    } else {
        inspect_inner(&path, depth + 1)
    };

    report.architectures = inner.architectures.clone();
    report.comparison = inner.comparison;
    report.contains_app_arch = inner.contains_app_arch;
    if inner.reason.is_some() {
        report.reason = inner.reason;
    }
    report.interpreter = Some(ScriptInterpreter {
        command,
        path: Some(path),
        resolved: inner.resolved,
        format: inner.format,
        architectures: inner.architectures,
        comparison: inner.comparison,
        contains_app_arch: inner.contains_app_arch,
        reason: inner.reason,
    });
}

fn compare(
    architectures: &[BinaryArch],
    fat: bool,
    app_arch: &str,
) -> (ArchComparison, Option<bool>) {
    if architectures.is_empty() {
        return (ArchComparison::Unknown, None);
    }
    let contains = architectures.iter().any(|arch| arch.as_str() == app_arch);
    if fat {
        return (ArchComparison::Fat, Some(contains));
    }
    if contains {
        (ArchComparison::Same, Some(true))
    } else {
        (ArchComparison::Different, Some(false))
    }
}

enum Parsed {
    Known {
        format: BinaryFormat,
        architectures: Vec<BinaryArch>,
    },
    Unknown {
        reason: &'static str,
    },
}

fn parse_header(buf: &[u8], pe_tail: Option<&[u8]>) -> Parsed {
    if buf.is_empty() {
        return Parsed::Unknown {
            reason: "empty_file",
        };
    }
    if buf.len() < 4 {
        return Parsed::Unknown {
            reason: "header_too_short",
        };
    }
    if buf[..4] == ELF_MAGIC {
        return parse_elf(buf);
    }
    if let Some(parsed) = parse_macho(buf) {
        return parsed;
    }
    if buf.starts_with(b"MZ") {
        return parse_pe(buf, pe_tail);
    }
    if buf.starts_with(b"#!") {
        return Parsed::Known {
            format: BinaryFormat::Script,
            architectures: Vec::new(),
        };
    }
    Parsed::Unknown {
        reason: "magic_unrecognized",
    }
}

fn parse_macho(buf: &[u8]) -> Option<Parsed> {
    let magic: [u8; 4] = buf.get(..4)?.try_into().ok()?;
    let (big_endian, fat, fat64) = match magic {
        [0xCF, 0xFA, 0xED, 0xFE] => (false, false, false),
        [0xFE, 0xED, 0xFA, 0xCF] => (true, false, false),
        [0xCE, 0xFA, 0xED, 0xFE] => (false, false, false),
        [0xFE, 0xED, 0xFA, 0xCE] => (true, false, false),
        [0xCA, 0xFE, 0xBA, 0xBE] => (true, true, false),
        [0xBE, 0xBA, 0xFE, 0xCA] => (false, true, false),
        [0xCA, 0xFE, 0xBA, 0xBF] => (true, true, true),
        [0xBF, 0xBA, 0xFE, 0xCA] => (false, true, true),
        _ => return None,
    };
    if fat {
        return Some(parse_fat(buf, big_endian, fat64));
    }
    let Some(cputype) = u32_at(buf, 4, big_endian) else {
        return Some(Parsed::Unknown {
            reason: "header_too_short",
        });
    };
    Some(Parsed::Known {
        format: BinaryFormat::MachO,
        architectures: vec![macho_arch(cputype)],
    })
}

fn parse_fat(buf: &[u8], big_endian: bool, fat64: bool) -> Parsed {
    let Some(count) = u32_at(buf, 4, big_endian) else {
        return Parsed::Unknown {
            reason: "fat_truncated",
        };
    };
    if count == 0 {
        return Parsed::Unknown {
            reason: "fat_empty",
        };
    }
    if count > MAX_FAT_SLICES {
        return Parsed::Unknown {
            reason: "fat_too_many",
        };
    }
    let entry = if fat64 { 32 } else { 20 };
    let needed = 8 + count as usize * entry;
    if buf.len() < needed {
        return Parsed::Unknown {
            reason: "fat_truncated",
        };
    }
    let mut architectures: Vec<BinaryArch> = Vec::new();
    for index in 0..count as usize {
        let Some(cputype) = u32_at(buf, 8 + index * entry, big_endian) else {
            return Parsed::Unknown {
                reason: "fat_truncated",
            };
        };
        let arch = macho_arch(cputype);
        if !architectures.contains(&arch) {
            architectures.push(arch);
        }
    }
    if architectures.is_empty() {
        return Parsed::Unknown {
            reason: "fat_empty",
        };
    }
    Parsed::Known {
        format: BinaryFormat::MachO,
        architectures,
    }
}

fn parse_elf(buf: &[u8]) -> Parsed {
    if buf.len() < 20 {
        return Parsed::Unknown {
            reason: "header_too_short",
        };
    }
    let big_endian = match buf[5] {
        1 => false,
        2 => true,
        _ => {
            return Parsed::Unknown {
                reason: "elf_unknown_endian",
            }
        }
    };
    if buf[4] != 1 && buf[4] != 2 {
        return Parsed::Unknown {
            reason: "elf_unknown_class",
        };
    }
    let Some(machine) = u16_at(buf, 18, big_endian) else {
        return Parsed::Unknown {
            reason: "header_too_short",
        };
    };
    if machine == 0 {
        return Parsed::Unknown {
            reason: "elf_machine_none",
        };
    }
    Parsed::Known {
        format: BinaryFormat::Elf,
        architectures: vec![elf_arch(machine)],
    }
}

fn parse_pe(buf: &[u8], tail: Option<&[u8]>) -> Parsed {
    if buf.len() < 0x40 {
        return Parsed::Unknown {
            reason: "header_too_short",
        };
    }
    let Some(offset) = u32_at(buf, 0x3c, false) else {
        return Parsed::Unknown {
            reason: "header_too_short",
        };
    };
    let offset = u64::from(offset);
    if offset > MAX_PE_OFFSET {
        return Parsed::Unknown {
            reason: "pe_offset_too_large",
        };
    }
    let header: &[u8] = if offset + 24 <= buf.len() as u64 {
        &buf[offset as usize..]
    } else if let Some(tail) = tail {
        tail
    } else {
        return Parsed::Unknown {
            reason: "pe_signature_missing",
        };
    };
    if header.len() < 24 || !header.starts_with(b"PE\0\0") {
        return Parsed::Unknown {
            reason: "pe_signature_missing",
        };
    }
    let Some(machine) = u16_at(header, 4, false) else {
        return Parsed::Unknown {
            reason: "pe_signature_missing",
        };
    };
    if machine == 0 {
        return Parsed::Unknown {
            reason: "pe_machine_none",
        };
    }
    Parsed::Known {
        format: BinaryFormat::Pe,
        architectures: vec![pe_arch(machine)],
    }
}

/// PE başlığı ilk okumanın dışındaysa (`e_lfanew` > 4 KiB) yalnız o bölümü
/// okur. Başlık tamponda ya da sınır dışıysa `None`.
fn read_pe_tail(file: &mut File, buf: &[u8], len: usize) -> Option<Vec<u8>> {
    if buf.len() < 0x40 || !buf.starts_with(b"MZ") {
        return None;
    }
    let offset = u64::from(u32_at(buf, 0x3c, false)?);
    if offset + 24 <= len as u64 {
        return None;
    }
    if offset > MAX_PE_OFFSET {
        return None;
    }
    let mut tail = vec![0u8; 24];
    file.seek(SeekFrom::Start(offset)).ok()?;
    let read = file.read(&mut tail).ok()?;
    tail.truncate(read);
    Some(tail)
}

fn macho_arch(cputype: u32) -> BinaryArch {
    match cputype {
        0x0100_0007 => BinaryArch::X86_64,
        7 => BinaryArch::X86,
        0x0100_000C => BinaryArch::Aarch64,
        12 => BinaryArch::Arm,
        0x0200_000C => BinaryArch::Arm64_32,
        18 => BinaryArch::PowerPc,
        0x0100_0012 => BinaryArch::PowerPc64,
        other => BinaryArch::Other(other),
    }
}

fn elf_arch(machine: u16) -> BinaryArch {
    match machine {
        62 => BinaryArch::X86_64,
        3 => BinaryArch::X86,
        183 => BinaryArch::Aarch64,
        40 => BinaryArch::Arm,
        243 => BinaryArch::Riscv64,
        20 => BinaryArch::PowerPc,
        21 => BinaryArch::PowerPc64,
        other => BinaryArch::Other(u32::from(other)),
    }
}

fn pe_arch(machine: u16) -> BinaryArch {
    match machine {
        0x8664 => BinaryArch::X86_64,
        0x014c => BinaryArch::X86,
        0xAA64 => BinaryArch::Aarch64,
        0x01c0 | 0x01c2 | 0x01c4 => BinaryArch::Arm,
        other => BinaryArch::Other(u32::from(other)),
    }
}

fn shebang_line(buf: &[u8]) -> Option<String> {
    if !buf.starts_with(b"#!") {
        return None;
    }
    let end = buf
        .iter()
        .position(|byte| *byte == b'\n' || *byte == b'\r')
        .unwrap_or(buf.len());
    let limit = end.min(MAX_SCRIPT_LINE + 2);
    let line = String::from_utf8_lossy(&buf[2..limit]).trim().to_string();
    if line.is_empty() {
        None
    } else {
        Some(line)
    }
}

/// Shebang satırından çalıştırılacak komutu çıkarır: `/usr/bin/env -S node`
/// → `node`, `/bin/sh -e` → `/bin/sh`. `env` değişken atamaları ve bayraklar
/// atlanır; ilk gerçek komut alınır.
fn interpreter_command(line: &str) -> Option<String> {
    let mut tokens = line.split_whitespace();
    let first = tokens.next()?;
    let is_env = first.rsplit('/').next().is_some_and(|last| last == "env");
    if !is_env {
        return Some(first.to_string());
    }
    tokens
        .find(|token| !token.starts_with('-') && !(token.contains('=') && !token.contains('/')))
        .map(str::to_string)
}

fn interpreter_path(command: &str) -> Option<PathBuf> {
    if command.contains('/') {
        let path = PathBuf::from(command);
        if path.is_file() {
            Some(path)
        } else {
            None
        }
    } else {
        find_on_path(command)
    }
}

fn u16_at(buf: &[u8], offset: usize, big_endian: bool) -> Option<u16> {
    let bytes = buf.get(offset..offset + 2)?;
    let pair = [bytes[0], bytes[1]];
    Some(if big_endian {
        u16::from_be_bytes(pair)
    } else {
        u16::from_le_bytes(pair)
    })
}

fn u32_at(buf: &[u8], offset: usize, big_endian: bool) -> Option<u32> {
    let bytes = buf.get(offset..offset + 4)?;
    let quad = [bytes[0], bytes[1], bytes[2], bytes[3]];
    Some(if big_endian {
        u32::from_be_bytes(quad)
    } else {
        u32::from_le_bytes(quad)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("yaz");
        path
    }

    /// 64-bit Mach-O başlığı: `CF FA ED FE` + cputype (little-endian) + dolgu.
    fn macho_thin(cputype: u32) -> Vec<u8> {
        let mut bytes = vec![0xCF, 0xFA, 0xED, 0xFE, 0, 0, 0, 0];
        bytes[4..8].copy_from_slice(&cputype.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 64]);
        bytes
    }

    /// Evrensel (fat) Mach-O: `CA FE BA BE` + dilim sayısı + 20 baytlık kayıtlar.
    fn macho_fat(cputypes: &[u32]) -> Vec<u8> {
        let mut bytes = vec![0xCA, 0xFE, 0xBA, 0xBE];
        bytes.extend_from_slice(&(cputypes.len() as u32).to_be_bytes());
        for cpu in cputypes {
            bytes.extend_from_slice(&cpu.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
            bytes.extend_from_slice(&0u32.to_be_bytes());
        }
        bytes
    }

    fn elf(machine: u16) -> Vec<u8> {
        let mut bytes = vec![
            0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        bytes.extend_from_slice(&machine.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 64]);
        bytes
    }

    fn pe(machine: u16, lfanew: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; lfanew as usize + 128];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3c..0x40].copy_from_slice(&lfanew.to_le_bytes());
        let at = lfanew as usize;
        bytes[at..at + 4].copy_from_slice(b"PE\0\0");
        bytes[at + 4..at + 6].copy_from_slice(&machine.to_le_bytes());
        bytes
    }

    #[test]
    fn macho_thin_reads_architecture() {
        let tmp = tempfile::tempdir().expect("tmp");
        let arm = write(tmp.path(), "arm", &macho_thin(0x0100_000C));
        let intel = write(tmp.path(), "intel", &macho_thin(0x0100_0007));
        let arm_report = inspect_path(&arm);
        assert_eq!(arm_report.format, BinaryFormat::MachO);
        assert_eq!(arm_report.architectures, vec![BinaryArch::Aarch64]);
        assert_eq!(arm_report.comparison.as_str(), expected_single("aarch64"));
        let intel_report = inspect_path(&intel);
        assert_eq!(intel_report.architectures, vec![BinaryArch::X86_64]);
        assert_eq!(intel_report.comparison.as_str(), expected_single("x86_64"));
    }

    /// Tek mimarili ikilide beklenen karşılaştırma; testler ana makineye bağlı
    /// kalmadan aynı kuralı doğrular.
    fn expected_single(arch: &str) -> &'static str {
        if app_arch_label() == arch {
            "same"
        } else {
            "different"
        }
    }

    #[test]
    fn macho_fat_lists_slices_and_flags_fat() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = write(
            tmp.path(),
            "universal",
            &macho_fat(&[0x0100_000C, 0x0100_0007]),
        );
        let report = inspect_path(&path);
        assert_eq!(report.format, BinaryFormat::MachO);
        assert_eq!(
            report.architectures,
            vec![BinaryArch::Aarch64, BinaryArch::X86_64]
        );
        assert_eq!(report.comparison, ArchComparison::Fat);
        assert_eq!(report.contains_app_arch, Some(true));
        assert!(report.summary().contains("evrensel"));
    }

    #[test]
    fn fat_header_truncated_is_reported() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut bytes = macho_fat(&[0x0100_000C, 0x0100_0007]);
        bytes.truncate(16); // dilim kayıtları eksik
        let path = write(tmp.path(), "broken", &bytes);
        let report = inspect_path(&path);
        assert_eq!(report.format, BinaryFormat::Unknown);
        assert_eq!(report.reason, Some("fat_truncated"));
        assert_eq!(report.comparison, ArchComparison::Unknown);
    }

    #[test]
    fn elf_reads_machine() {
        let tmp = tempfile::tempdir().expect("tmp");
        let intel = write(tmp.path(), "elf-intel", &elf(62));
        let arm = write(tmp.path(), "elf-arm", &elf(183));
        let intel_report = inspect_path(&intel);
        assert_eq!(intel_report.format, BinaryFormat::Elf);
        assert_eq!(intel_report.architectures, vec![BinaryArch::X86_64]);
        let arm_report = inspect_path(&arm);
        assert_eq!(arm_report.format, BinaryFormat::Elf);
        assert_eq!(arm_report.architectures, vec![BinaryArch::Aarch64]);
    }

    #[test]
    fn pe_reads_machine() {
        let tmp = tempfile::tempdir().expect("tmp");
        let amd64 = write(tmp.path(), "app.exe", &pe(0x8664, 0x80));
        let arm64 = write(tmp.path(), "app-arm.exe", &pe(0xAA64, 0x80));
        let amd64_report = inspect_path(&amd64);
        assert_eq!(amd64_report.format, BinaryFormat::Pe);
        assert_eq!(amd64_report.architectures, vec![BinaryArch::X86_64]);
        let arm64_report = inspect_path(&arm64);
        assert_eq!(arm64_report.architectures, vec![BinaryArch::Aarch64]);
    }

    #[test]
    fn pe_header_beyond_first_read_is_followed() {
        let tmp = tempfile::tempdir().expect("tmp");
        // e_lfanew ilk 4 KiB'in dışında: yalnız o bölüm okunur.
        let path = write(tmp.path(), "far.exe", &pe(0x8664, 6000));
        let report = inspect_path(&path);
        assert_eq!(report.format, BinaryFormat::Pe);
        assert_eq!(report.architectures, vec![BinaryArch::X86_64]);
        assert_eq!(report.reason, None);
    }

    #[test]
    fn pe_offset_too_large_is_rejected() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut bytes = pe(0x8664, 0x80);
        bytes[0x3c..0x40].copy_from_slice(&(3 * 1024 * 1024u32).to_le_bytes());
        let path = write(tmp.path(), "bozuk.exe", &bytes);
        let report = inspect_path(&path);
        assert_eq!(report.format, BinaryFormat::Unknown);
        assert_eq!(report.reason, Some("pe_offset_too_large"));
    }

    #[test]
    fn script_resolves_interpreter_architecture() {
        let tmp = tempfile::tempdir().expect("tmp");
        let interpreter = write(tmp.path(), "node", &macho_thin(0x0100_000C));
        let script = write(
            tmp.path(),
            "claude",
            format!("#!{}\nconsole.log('x')\n", interpreter.display()).as_bytes(),
        );
        let report = inspect_path(&script);
        assert_eq!(report.format, BinaryFormat::Script);
        let interp = report.interpreter.as_ref().expect("yorumlayıcı");
        assert_eq!(interp.command, interpreter.to_string_lossy());
        assert_eq!(interp.format, BinaryFormat::MachO);
        assert_eq!(interp.architectures, vec![BinaryArch::Aarch64]);
        // Betiğin kendi mimarisi yok; sonuç yorumlayıcıdan gelir.
        assert_eq!(report.architectures, vec![BinaryArch::Aarch64]);
        assert_eq!(report.comparison, interp.comparison);
        assert_eq!(report.reason, None);
    }

    #[test]
    fn script_with_universal_interpreter_reports_fat() {
        let tmp = tempfile::tempdir().expect("tmp");
        let interpreter = write(
            tmp.path(),
            "python",
            &macho_fat(&[0x0100_000C, 0x0100_0007]),
        );
        let script = write(
            tmp.path(),
            "kimi",
            format!("#!{}\n", interpreter.display()).as_bytes(),
        );
        let report = inspect_path(&script);
        assert_eq!(report.format, BinaryFormat::Script);
        // Betiğin kendi biçimi betik; ama çalışan yorumlayıcı evrensel.
        assert_eq!(report.comparison, ArchComparison::Fat);
        assert_eq!(report.contains_app_arch, Some(true));
        assert!(report.summary().contains("evrensel"));
    }

    #[test]
    fn script_with_unresolved_interpreter_is_unknown() {
        let tmp = tempfile::tempdir().expect("tmp");
        let script = write(tmp.path(), "kimi", b"#!/nonexistent/interpreter\n");
        let report = inspect_path(&script);
        assert_eq!(report.format, BinaryFormat::Script);
        assert_eq!(report.comparison, ArchComparison::Unknown);
        assert_eq!(report.reason, Some("interpreter_unresolved"));
        let interp = report.interpreter.as_ref().expect("kayıt");
        assert_eq!(interp.path, None);
    }

    #[test]
    fn env_flags_and_assignments_are_skipped() {
        assert_eq!(
            interpreter_command("/usr/bin/env -S node --experimental").as_deref(),
            Some("node")
        );
        assert_eq!(
            interpreter_command("/usr/bin/env FOO=1 node").as_deref(),
            Some("node")
        );
        assert_eq!(
            interpreter_command("/bin/sh -e").as_deref(),
            Some("/bin/sh")
        );
        assert_eq!(interpreter_command("/usr/bin/env -S"), None);
    }

    #[test]
    fn unknown_magic_is_reported() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = write(tmp.path(), "not-a-binary", b"merhaba dunya\n");
        let report = inspect_path(&path);
        assert_eq!(report.format, BinaryFormat::Unknown);
        assert_eq!(report.reason, Some("magic_unrecognized"));
        assert_eq!(report.comparison, ArchComparison::Unknown);
    }

    #[test]
    fn empty_file_is_reported() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = write(tmp.path(), "empty", b"");
        let report = inspect_path(&path);
        assert_eq!(report.format, BinaryFormat::Unknown);
        assert_eq!(report.reason, Some("empty_file"));
        assert_eq!(report.header_bytes, 0);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_followed_and_reported() {
        let tmp = tempfile::tempdir().expect("tmp");
        let target = write(tmp.path(), "real", &macho_thin(0x0100_0007));
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).expect("bağ");
        let report = inspect_path(&link);
        assert!(report.symlink);
        let canonical = std::fs::canonicalize(&target).expect("kanonik");
        assert_eq!(report.resolved.as_deref(), Some(canonical.as_path()));
        assert_eq!(report.architectures, vec![BinaryArch::X86_64]);
    }

    #[test]
    fn missing_path_is_reported() {
        let tmp = tempfile::tempdir().expect("tmp");
        let report = inspect_path(&tmp.path().join("yok"));
        assert_eq!(report.reason, Some("not_found"));
        assert_eq!(report.format, BinaryFormat::Unknown);
    }

    #[test]
    fn current_executable_matches_app_architecture() {
        let exe = std::env::current_exe().expect("current_exe");
        let report = inspect_path(&exe);
        assert_ne!(
            report.format,
            BinaryFormat::Unknown,
            "sebep: {:?}",
            report.reason
        );
        assert!(
            matches!(
                report.comparison,
                ArchComparison::Same | ArchComparison::Fat
            ),
            "karşılaştırma: {:?}",
            report.comparison
        );
        assert_eq!(report.contains_app_arch, Some(true));
        assert_eq!(report.app_arch, app_arch_label());
    }

    #[test]
    fn app_arch_label_is_canonical() {
        let label = app_arch_label();
        assert!(!label.is_empty());
        for known in ["x86_64", "aarch64", "arm", "x86", "riscv64"] {
            assert_ne!(label, format!("other({known})"));
        }
    }
}
