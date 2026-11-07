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

import os
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

#: Ölçülen taban (2026-11-07, macOS + CI): 45 testin 40'ı koşar, 5'i gerçek
#: worker/tesseract ister ve `#[ignore]` taşır.
BASELINE_PASSED = 40
BASELINE_IGNORED = 5

#: `--ignored` koşusunun tabanı: beş test de **gerçekten** koşmalı. Bu koşu
#: macOS'ta depo venv'i (pymupdf/pytesseract) + tesseract ister; CI'da `desktop`
#: işi ikisini de kurar.
IGNORED_BASELINE = 5
#: Yoksayılan koşu normal testleri filtreler; sayı tabanla aynı olmalı.
IGNORED_FILTERED = BASELINE_PASSED

#: Test kendi kendine atladığını yazarsa: motor yok, hiçbir şey ölçülmedi.
SKIP_MARKER = "test atlandı"

#: `--ignored` koşusunda koşması beklenen testler (öz test fixture'ı da kullanır).
IGNORED_NAMES = (
    "commands::tests::retrying_a_failed_parse_job_really_parses_the_document",
    "commands::tests::retry_reuses_the_recorded_ocr_setting_on_a_scanned_document",
    "commands::tests::redaction_runs_off_the_vault_lock_and_reports_pages",
    "commands::tests::redaction_cancel_stops_the_worker_and_writes_nothing",
    "commands::tests::two_parses_run_concurrently_on_a_real_vault",
)

RUNNING = re.compile(r"^running (\d+) tests?$", re.MULTILINE)
RESULT = re.compile(
    r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
    r"(\d+) measured; (\d+) filtered out",
    re.MULTILINE,
)
TEST_START = re.compile(r"^test (\S+) \.\.\. ", re.MULTILINE)


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


def check_ignored(log: str, expected: int = IGNORED_BASELINE) -> list[str]:
    """`--ignored` koşusunun günlüğünü denetler; boş liste = yeşil.

    Bu beş test **gerçek** worker'ı (pymupdf + tesseract) ölçer. Üç ayrı yalanı
    kapatır: (a) test motoru bulamayıp hiçbir şey ölçmeden geçti (`E_SKIPPED`),
    (b) koşu daraltıldı/`--ignored` düştü (`E_FILTER`), (c) sayı tabandan az ya
    da fazla (`E_BASELINE`). 2026-11-06'da bu testler ilk kez koşulduğunda
    **0 geçti / 5 düştü**; sayı kapısı olmadan "5 yeşil" görünebiliyordu.
    """
    errors: list[str] = []
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
    if SKIP_MARKER in log:
        errors.append(
            f"E_SKIPPED: günlükte '{SKIP_MARKER}' var — test motoru bulamayıp "
            "hiçbir şey ölçmeden geçmiş (DIM_REQUIRE_WORKER=1 verilmemiş ya da "
            "venv/tesseract kurulmamış)"
        )
    if ignored != 0:
        errors.append(
            f"E_IGNORED: yoksayılan koşuda {ignored} test hâlâ yoksayıldı "
            "— `#[ignore]` kaldırılmadı ya da `--ignored` düştü"
        )
    if measured != 0:
        errors.append(f"E_FILTER: {measured} ölçüm testi koştu (bench beklenmiyor)")
    if filtered != IGNORED_FILTERED:
        errors.append(
            f"E_FILTER: {filtered} test filtrelendi, {IGNORED_FILTERED} beklenirdi "
            "— `--ignored` bayrağı düşmüş olabilir (koşu daraltılmış sayılmaz)"
        )
    if running != passed:
        errors.append(
            f"E_COUNT: 'running {running} tests' ile özet ({passed} passed) uyuşmuyor"
        )
    started = TEST_START.findall(log)
    missing = [name for name in IGNORED_NAMES if name not in started]
    if missing:
        errors.append(
            f"E_CENSUS: beklenen test koşmadı: {', '.join(missing)} — ad değişmiş "
            "ya da test dosyadan çıkarılmış olabilir"
        )
    if len(started) != expected:
        errors.append(
            f"E_BASELINE: {len(started)} test başladı, taban {expected} "
            "— test sessizce kaybolmuş olabilir"
        )
    if passed != expected:
        errors.append(
            f"E_BASELINE: {passed} test koştu, taban {expected} — bilinçli "
            "değişiklikse `IGNORED_BASELINE` güncellenmeli"
        )
    return errors


def report_ignored(errors: list[str]) -> int:
    if errors:
        print("Masaüstü kabuk gerçek worker testleri KIRMIZI:")
        for error in errors:
            print(" -", error)
        return 1
    print(
        "Masaüstü kabuk gerçek worker testleri yeşil · "
        f"{IGNORED_BASELINE} koştu · 0 düştü · 0 atlandı "
        f"(motor: pymupdf + tesseract)"
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


def ignored_log(
    passed: int = IGNORED_BASELINE,
    running: int | None = None,
    filtered: int = IGNORED_FILTERED,
    ignored: int = 0,
    extra: str = "",
) -> str:
    names = IGNORED_NAMES[:passed]
    lines = "".join(f"test {name} ... ok\n" for name in names)
    return (
        "     Running unittests src/lib.rs (target/debug/deps/dim_desktop_lib-abc)\n"
        f"\nrunning {IGNORED_BASELINE if running is None else running} tests\n"
        f"{lines}{extra}"
        f"\ntest result: ok. {passed} passed; 0 failed; {ignored} ignored; 0 measured; "
        f"{filtered} filtered out; finished in 27.47s\n"
    )


def self_test_ignored() -> int:
    """`--ignored` modunun kural başına fixture'ı: temiz yeşil, kusurlu kırmızı."""
    cases: list[tuple[str, str, str | None]] = [
        ("temiz gerçek-worker koşusu", ignored_log(), None),
        (
            "bir test kayıp (4 koştu)",
            ignored_log(passed=IGNORED_BASELINE - 1, running=IGNORED_BASELINE - 1),
            "E_BASELINE",
        ),
        (
            "test moturu bulamayıp atlandı",
            ignored_log(
                extra="test commands::tests::two_parses ... python+pymupdf yok; test atlandı\nok\n"
            ),
            "E_SKIPPED",
        ),
        (
            "düşen test",
            ignored_log().replace(
                f"test result: ok. {IGNORED_BASELINE} passed; 0 failed",
                f"test result: FAILED. {IGNORED_BASELINE - 1} passed; 1 failed",
            ),
            "E_TEST_FAILED",
        ),
        (
            "`--ignored` düştü (hiç filtrelenmedi)",
            ignored_log(filtered=0),
            "E_FILTER",
        ),
        (
            "hâlâ yoksayılan test var",
            ignored_log(ignored=1),
            "E_IGNORED",
        ),
        (
            "özet yok (derleme kırıldı)",
            "error[E0433]: failed to resolve\ncargo test exited with 101\n",
            "E_LOG",
        ),
    ]

    failures: list[str] = []
    for name, log, expected in cases:
        errors = check_ignored(log)
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
        print("Masaüstü kabuk gerçek worker kapısı öz testi KIRMIZI:")
        for failure in failures:
            print(" -", failure)
        return 1
    print(
        f"Masaüstü kabuk gerçek worker kapısı öz testi yeşil "
        f"({len(cases)} kural: atlama/kayıp/düşen/filtre/yoksayılan/biçim)"
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


def run_cargo_ignored() -> tuple[int, str]:
    """`--ignored` koşusu: `DIM_REQUIRE_WORKER=1` ile atlama **yasak**."""
    cargo = cargo_binary()
    if cargo is None:
        return 1, "cargo bulunamadı (PATH ya da ~/.cargo/bin)"
    env = dict(os.environ)
    env["DIM_REQUIRE_WORKER"] = "1"
    proc = subprocess.run(
        [
            cargo,
            "test",
            "--manifest-path",
            str(DESKTOP / "Cargo.toml"),
            "--lib",
            "--",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        env=env,
    )
    return proc.returncode, proc.stdout + proc.stderr


def main(argv: list[str]) -> int:
    args = argv[1:]
    ignored = "--ignored" in args
    if "--self-test" in args:
        return self_test_ignored() if ignored else self_test()
    if "--log" in args:
        index = args.index("--log")
        if index + 1 >= len(args):
            print("--log bir yol ister")
            return 1
        path = Path(args[index + 1])
        if not path.exists():
            print(f"günlük bulunamadı: {path}")
            return 1
        log = path.read_text(encoding="utf-8")
        print(log, end="")
        return report_ignored(check_ignored(log)) if ignored else report(check(log))
    if ignored:
        code, log = run_cargo_ignored()
        print(log, end="")
        errors = check_ignored(log)
        if code != 0 and not any(e.startswith("E_TEST_FAILED") for e in errors):
            errors.insert(0, f"E_LOG: cargo test {code} ile düştü")
        return report_ignored(errors)
    code, log = run_cargo()
    print(log, end="")
    errors = check(log)
    if code != 0 and not any(e.startswith("E_TEST_FAILED") for e in errors):
        errors.insert(0, f"E_LOG: cargo test {code} ile düştü")
    return report(errors)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
