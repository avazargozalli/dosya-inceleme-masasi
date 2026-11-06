#!/usr/bin/env python3
"""Masaüstü kabuk test kapısı.

`apps/desktop/src-tauri` testleri bugüne dek **hiçbir yerde koşmuyordu**:
CI'daki `masaüstü kabuk` işi yalnız `cargo clippy --all-targets` çalıştırıyordu
— test ikilisi **derleniyor** ama **koşmuyordu**. Bu yüzden `commands_tests.rs`
(32 test; 27'si worker'sız) ve `parse_pool.rs` (6 test) hiç yürütülmemiş, her
turda "dürüst sınır" olarak yazılmıştı.

Bu betik `cargo test --lib` çıktısını okur ve **kayıtlı tabana** karşı doğrular:

1. **Günlük biçimi:** tek `running N tests` satırı + tek `test result:` özeti.
   Derleme kırıldıysa ya da ikili hiç koşmadıysa "yeşil" görünmez.
2. **Düşen test yok:** `0 failed`. Özet satırı `ok` değilse kırmızı.
3. **Filtre yok:** `filtered out` sıfır olmalı; ad filtresiyle daraltılmış koşu
   tam koşu sayılmaz.
4. **Kaynak sayımı:** `#[test]` sayısı = koşan + yoksayılan. Test silinirse
   (dosyadan çıkarılırsa) kırmızı — sayı sessizce düşemez.
5. **Taban:** koşan/yoksayılan sayıları `BASELINE_PASSED`/`BASELINE_IGNORED`
   ile birebir. Yeni test eklemek ya da bir testi `#[ignore]`a almak bilinçli
   bir tek satırlık taban güncellemesi gerektirir; kazara yoksayma kırmızıdır.

`--self-test` gerçek cargo koşusu olmadan bu kuralları fixture günlüklerle
sınar: temiz günlük **yeşil**, her kusurlu günlük **kırmızı** olmalı.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DESKTOP = ROOT / "apps" / "desktop" / "src-tauri"

#: `cargo test --lib` bu iki dosyadaki testleri koşar (başka `#[cfg(test)]`
#: modülü yok — yeni modül eklenirse buraya da eklenmeli).
SOURCES = (
    DESKTOP / "src" / "commands_tests.rs",
    DESKTOP / "src" / "parse_pool.rs",
)

#: Ölçülen taban (2026-11-06, macOS + CI): 38 testin 33'ü koşar, 5'i gerçek
#: worker/tesseract ister ve `#[ignore]` taşır.
BASELINE_PASSED = 33
BASELINE_IGNORED = 5

RUNNING = re.compile(r"^running (\d+) tests?$", re.MULTILINE)
RESULT = re.compile(
    r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
    r"(\d+) measured; (\d+) filtered out",
    re.MULTILINE,
)


def census() -> tuple[int, int]:
    """Kaynakta `#[test]` ve `#[ignore]` sayısı."""
    total = 0
    ignored = 0
    for path in SOURCES:
        text = path.read_text(encoding="utf-8")
        total += len(re.findall(r"^\s*#\[test\]", text, re.MULTILINE))
        ignored += len(re.findall(r"^\s*#\[ignore", text, re.MULTILINE))
    return total, ignored


def check(log: str, counts: tuple[int, int] | None = None) -> list[str]:
    """Günlüğü denetler; boş liste = yeşil."""
    errors: list[str] = []
    total, ignored_decl = counts if counts is not None else census()

    runnings = RUNNING.findall(log)
    results = RESULT.findall(log)
    if len(runnings) != 1:
        errors.append(
            f"E_LOG: günlükte tek 'running N tests' satırı beklenir, "
            f"{len(runnings)} bulundu (derleme kırılmış ya da ikili koşmamış olabilir)"
        )
    if len(results) != 1:
        errors.append(
            f"E_LOG: günlükte tek 'test result' özeti beklenir, "
            f"{len(results)} bulundu (koşu tamamlanmamış olabilir)"
        )
    if errors:
        return errors

    status, passed, failed, ignored, measured, filtered = results[0]
    passed, failed, ignored = int(passed), int(failed), int(ignored)
    measured, filtered = int(measured), int(filtered)
    running = int(runnings[0])

    if failed != 0:
        errors.append(f"E_TEST_FAILED: {failed} test düştü (passed {passed})")
    if status != "ok":
        errors.append(f"E_TEST_FAILED: özet durumu '{status}', 'ok' beklenirdi")
    if filtered != 0:
        errors.append(
            f"E_FILTER: {filtered} test filtrelendi — ad filtresiyle daraltılmış "
            "koşu tam koşu sayılmaz"
        )
    if measured != 0:
        errors.append(f"E_FILTER: {measured} ölçüm testi koştu (bench beklenmiyor)")
    if running != passed + ignored:
        errors.append(
            f"E_COUNT: 'running {running} tests' ile özet ({passed} passed + "
            f"{ignored} ignored) uyuşmuyor"
        )
    if total != passed + ignored:
        errors.append(
            f"E_CENSUS: kaynakta {total} test var, koşan+yoksayılan {passed + ignored} "
            "— test silinmiş ya da yeni test tabana işlenmemiş"
        )
    if ignored_decl != ignored:
        errors.append(
            f"E_CENSUS: kaynakta {ignored_decl} `#[ignore]` var, koşuda {ignored} "
            "yoksayıldı"
        )
    if passed != BASELINE_PASSED:
        errors.append(
            f"E_BASELINE: {passed} test koştu, taban {BASELINE_PASSED} — bilinçli "
            "değişiklikse `BASELINE_PASSED` güncellenmeli"
        )
    if ignored != BASELINE_IGNORED:
        errors.append(
            f"E_BASELINE: {ignored} test yoksayıldı, taban {BASELINE_IGNORED} — "
            "bir test kazara `#[ignore]` almış olabilir"
        )
    return errors


def report(errors: list[str]) -> int:
    if errors:
        print("Masaüstü kabuk test kapısı KIRMIZI:")
        for error in errors:
            print(" -", error)
        return 1
    print(
        "Masaüstü kabuk test kapısı yeşil · "
        f"{BASELINE_PASSED} koştu · 0 düştü · {BASELINE_IGNORED} yoksayıldı "
        f"(kaynak {BASELINE_PASSED + BASELINE_IGNORED} test)"
    )
    return 0


def clean_log(passed: int = BASELINE_PASSED, ignored: int = BASELINE_IGNORED) -> str:
    total = passed + ignored
    return (
        "   Compiling dim-desktop v0.1.0\n"
        f"     Running unittests src/lib.rs (target/debug/deps/dim_desktop_lib-abc)\n"
        f"\nrunning {total} tests\n"
        "test commands::tests::empty_path_rejected ... ok\n"
        f"\ntest result: ok. {passed} passed; 0 failed; {ignored} ignored; "
        "0 measured; 0 filtered out; finished in 7.50s\n"
    )


def self_test() -> int:
    """Kural başına bir fixture: temiz yeşil, kusurlu kırmızı."""
    base_census = (BASELINE_PASSED + BASELINE_IGNORED, BASELINE_IGNORED)
    cases: list[tuple[str, str, tuple[int, int], str | None]] = [
        ("temiz günlük", clean_log(), base_census, None),
        (
            "düşen test",
            clean_log().replace(
                f"test result: ok. {BASELINE_PASSED} passed; 0 failed",
                f"test result: FAILED. {BASELINE_PASSED - 1} passed; 1 failed",
            ),
            base_census,
            "E_TEST_FAILED",
        ),
        (
            "sessizce atlanan test (running düştü)",
            clean_log(passed=BASELINE_PASSED - 1).replace(
                f"running {BASELINE_PASSED + BASELINE_IGNORED - 1} tests",
                f"running {BASELINE_PASSED + BASELINE_IGNORED} tests",
            ),
            base_census,
            "E_COUNT",
        ),
        (
            "kaynakta silinen test",
            clean_log(),
            (BASELINE_PASSED + BASELINE_IGNORED - 1, BASELINE_IGNORED),
            "E_CENSUS",
        ),
        (
            "kaynaktan kalkan #[ignore]",
            clean_log(),
            (BASELINE_PASSED + BASELINE_IGNORED, BASELINE_IGNORED - 1),
            "E_CENSUS",
        ),
        (
            "test kazara #[ignore] aldı",
            clean_log(passed=BASELINE_PASSED - 1, ignored=BASELINE_IGNORED + 1),
            (BASELINE_PASSED + BASELINE_IGNORED, BASELINE_IGNORED + 1),
            "E_BASELINE",
        ),
        (
            "ad filtresiyle daraltılmış koşu",
            clean_log(passed=3, ignored=0).replace(
                "0 filtered out", f"{BASELINE_PASSED + BASELINE_IGNORED - 3} filtered out"
            ),
            base_census,
            "E_FILTER",
        ),
        (
            "derleme kırıldı (özet yok)",
            "error[E0433]: failed to resolve\ncargo test exited with 101\n",
            base_census,
            "E_LOG",
        ),
        (
            "iki özet (çift koşu)",
            clean_log() + clean_log(),
            base_census,
            "E_LOG",
        ),
    ]

    failures: list[str] = []
    for name, log, counts, expected in cases:
        errors = check(log, counts)
        codes = " ".join(errors)
        if expected is None:
            if errors:
                failures.append(f"{name}: yeşil beklenirdi, kırmızı: {codes}")
            else:
                print(f"ok: {name} → kapı açık")
        else:
            if not any(error.startswith(expected) for error in errors):
                failures.append(f"{name}: {expected} beklenirdi, gelen: {codes or 'yeşil'}")
            else:
                print(f"ok: {name} → {expected}")

    if failures:
        print("Masaüstü kabuk test kapısı öz testi KIRMIZI:")
        for failure in failures:
            print(" -", failure)
        return 1
    print(
        f"Masaüstü kabuk test kapısı öz testi yeşil "
        f"({len(cases)} kural: biçim/düşen/filtre/sayım/kaynak/taban)"
    )
    return 0


def cargo_binary() -> str | None:
    found = shutil.which("cargo")
    if found:
        return found
    fallback = Path.home() / ".cargo" / "bin" / "cargo"
    return str(fallback) if fallback.exists() else None


def run_cargo() -> tuple[int, str]:
    cargo = cargo_binary()
    if cargo is None:
        return 1, "cargo bulunamadı (PATH ya da ~/.cargo/bin)"
    proc = subprocess.run(
        [cargo, "test", "--manifest-path", str(DESKTOP / "Cargo.toml"), "--lib"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    return proc.returncode, proc.stdout + proc.stderr


def main(argv: list[str]) -> int:
    args = argv[1:]
    if "--self-test" in args:
        return self_test()
    if "--log" in args:
        index = args.index("--log")
        if index + 1 >= len(args):
            print("--log bir yol ister")
            return 1
        path = Path(args[index + 1])
        if not path.exists():
            print(f"günlük bulunamadı: {path}")
            return 1
        print(path.read_text(encoding="utf-8"), end="")
        return report(check(path.read_text(encoding="utf-8")))
    code, log = run_cargo()
    print(log, end="")
    errors = check(log)
    if code != 0 and not any(e.startswith("E_TEST_FAILED") for e in errors):
        errors.insert(0, f"E_LOG: cargo test {code} ile düştü")
    return report(errors)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
