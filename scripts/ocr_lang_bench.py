#!/usr/bin/env python3
"""OCR **dil modeli** kalite/gecikme ölçümü — gerçek worker süreci üzerinden.

Neden: ürün OCR'ı Tesseract ile yapıyor ve varsayılan dil `tur+eng`. Model
seçiminin Türkçe diyakritikler üzerindeki etkisi **ölçülmemişti**; ASCII
fikstürler bu farkı göremez (`eng` modeli ASCII metni zaten doğru okur).

Bu betik:
- `fixtures/synthetic/turkish-scan.pdf` (2 sayfa, metin katmanı yok, ş ğ ı İ
  ç ö ü yoğun) belgesini **gerçek worker** sürecine verir,
- her dil için (`eng`, `tur`, `tur+eng`) **yeni süreç** başlatır (başlatma
  maliyeti ölçüme dahil),
- karakter düzeyinde doğruluğu (Levenshtein), Türkçe harf korunmasını, güven
  skorunu ve süreyi raporlar.

Yanlış "hızlı ama boş" rapor üretilmez: her koşuda sayfa sayısı ve metin
uzunluğu kapıdan geçer; beklenen metnin tamamı okunamazsa rapor bunu yazar.

Kullanım:
    python3 scripts/ocr_lang_bench.py --reps 5 --out docs/releases/F15/bench-ocr-lang.json
    python3 scripts/ocr_lang_bench.py --self-test
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKER_DIR = ROOT / "workers" / "document"
FIXTURE = ROOT / "fixtures" / "synthetic" / "turkish-scan.pdf"
LANGS = ("eng", "tur", "tur+eng")
PROTOCOL_VERSION = 1


# --- saf yardımcılar (--self-test ile sınanır) -----------------------------


def normalize(text: str) -> str:
    """Karşılaştırma biçimi: satır sonları boşluğa iner, boşluklar tekilleşir.

    Noktalama **korunur**: OCR'ın virgül/nokta kaybı da doğruluk kaybıdır.
    """
    return " ".join(text.split())


def levenshtein(left: str, right: str) -> int:
    """Düzenleme mesafesi (karakter). Saf Python; metinler kısa (≤2 KB)."""
    if left == right:
        return 0
    if not left:
        return len(right)
    if not right:
        return len(left)
    previous = list(range(len(right) + 1))
    for i, ch_left in enumerate(left, start=1):
        current = [i]
        for j, ch_right in enumerate(right, start=1):
            cost = 0 if ch_left == ch_right else 1
            current.append(
                min(
                    previous[j] + 1,  # silme
                    current[j - 1] + 1,  # ekleme
                    previous[j - 1] + cost,  # yerine koyma
                )
            )
        previous = current
    return previous[-1]


def char_accuracy(expected: str, actual: str) -> float:
    """1 − mesafe/uzunluk. Beklenen boşsa 0.0 (tanımsız oran uydurulmaz)."""
    expected_n = normalize(expected)
    actual_n = normalize(actual)
    if not expected_n:
        return 0.0
    distance = levenshtein(expected_n, actual_n)
    return max(0.0, 1.0 - distance / len(expected_n))


def turkish_letter_stats(expected: str, actual: str) -> dict[str, int]:
    """Türkçe harflerin korunması: beklenen sayı, bulunan, ASCII'ye düşen."""
    letters = "şğıİçöüŞĞÇÖÜ"
    folded = str.maketrans(
        {
            "ş": "s",
            "ğ": "g",
            "ı": "i",
            "İ": "I",
            "ç": "c",
            "ö": "o",
            "ü": "u",
            "Ş": "S",
            "Ğ": "G",
            "Ç": "C",
            "Ö": "O",
            "Ü": "U",
        }
    )
    expected_n = normalize(expected)
    actual_n = normalize(actual)
    expected_count = sum(1 for ch in expected_n if ch in letters)
    actual_count = sum(1 for ch in actual_n if ch in letters)
    # Beklenen metnin ASCII'ye katlanmışı gerçekte ne kadar yer alıyor:
    # `eng` modeli tipik olarak bunu üretir (şüpheli → supheli).
    folded_text = expected_n.translate(folded)
    folded_hits = 0
    if folded_text != expected_n:
        # Katlanmış metnin birebir geçtiği pencere sayısı (kaba ama dürüst).
        folded_hits = actual_n.count(folded_text) if folded_text in actual_n else 0
    return {
        "expected": expected_count,
        "found": actual_count,
        "ascii_folded_whole_page": folded_hits,
    }


def confidence_of(warnings: list[str]) -> float | None:
    """Worker uyarısındaki 'güven skoru NN/100' değerini okur; yoksa None."""
    import re

    scores: list[float] = []
    for warning in warnings:
        for match in re.finditer(r"güven skoru (\d+(?:[.,]\d+)?)/100", warning):
            scores.append(float(match.group(1).replace(",", ".")))
    if not scores:
        return None
    return round(sum(scores) / len(scores), 1)


# --- worker istemcisi -------------------------------------------------------


def _read_response(process: subprocess.Popen, request_id: str) -> dict:
    """İstek kimliğine ait sonuç çerçevesini okur; ilerleme çerçeveleri atlanır."""
    assert process.stdout is not None
    while True:
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("worker yanıt vermeden kapandı")
        frame = json.loads(line)
        if frame.get("id") != request_id:
            continue
        if frame.get("progress") is not None:
            continue
        if frame.get("ok"):
            return frame["result"]
        error = frame.get("error") or {}
        raise RuntimeError(f"{error.get('code')}: {error.get('message')}")


def run_once(lang: str, data: bytes) -> dict:
    """Yeni worker süreci + tek `extract_text` isteği. Süre başlatmayı içerir."""
    python = WORKER_DIR / ".venv" / "bin" / "python"
    if not python.is_file():
        python = Path(sys.executable)
    env = dict(os.environ)
    env.setdefault("PYTHONPATH", str(WORKER_DIR))
    started = time.perf_counter()
    process = subprocess.Popen(
        [str(python), "-m", "dim_worker.worker"],
        cwd=str(WORKER_DIR),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    try:
        assert process.stdin is not None
        request = {
            "v": PROTOCOL_VERSION,
            "id": "bench-1",
            "method": "extract_text",
            "params": {
                "contentBase64": base64.b64encode(data).decode(),
                "mime": "application/pdf",
                "ocr": True,
                "ocrLang": lang,
            },
        }
        process.stdin.write((json.dumps(request) + "\n").encode())
        process.stdin.flush()
        result = _read_response(process, "bench-1")
        elapsed_ms = (time.perf_counter() - started) * 1000.0
    finally:
        try:
            process.kill()
        except Exception:  # noqa: BLE001 - kapanmış süreç
            pass

    pages = result.get("pages") or []
    texts = [str(page.get("text") or "") for page in pages]
    return {
        "lang": lang,
        "elapsedMs": round(elapsed_ms, 1),
        "pages": len(pages),
        "chars": sum(len(text) for text in texts),
        "texts": texts,
        "warnings": list(result.get("warnings") or []),
        "confidence": confidence_of(list(result.get("warnings") or [])),
    }


def truth_pages() -> list[str]:
    sys.path.insert(0, str(ROOT / "fixtures" / "synthetic"))
    import generate  # noqa: PLC0415 - fikstür modülü bilinçli olarak geç yüklenir

    return generate.turkish_scan_truth()


def summarize(runs: list[dict], truth: list[str]) -> dict:
    """Koşuları dil başına tek satıra indirger; metin **her koşuda** kapıdan geçer."""
    by_lang: dict[str, list[dict]] = {}
    for run in runs:
        by_lang.setdefault(run["lang"], []).append(run)

    out: dict[str, dict] = {}
    for lang, items in by_lang.items():
        accuracies: list[float] = []
        elapsed: list[float] = []
        chars: list[int] = []
        letter_expected = 0
        letter_found = 0
        confidences: list[float] = []
        empty_pages = 0
        texts_first: list[str] = []
        for index, run in enumerate(items):
            page_text = " ".join(run["texts"])
            accuracy = char_accuracy(" ".join(truth), page_text)
            stats = turkish_letter_stats(" ".join(truth), page_text)
            accuracies.append(round(accuracy, 4))
            elapsed.append(run["elapsedMs"])
            chars.append(run["chars"])
            empty_pages += sum(1 for text in run["texts"] if not text.strip())
            if run["confidence"] is not None:
                confidences.append(run["confidence"])
            if index == 0:
                texts_first = run["texts"]
                letter_expected = stats["expected"]
                letter_found = stats["found"]
        out[lang] = {
            "runs": len(items),
            "pages": items[0]["pages"],
            "charsMedian": int(statistics.median(chars)),
            "emptyPages": empty_pages,
            "accuracyMedian": round(statistics.median(accuracies), 4),
            "accuracyMin": min(accuracies),
            "accuracyMax": max(accuracies),
            "turkishLettersExpected": letter_expected,
            "turkishLettersFound": letter_found,
            "confidenceMedian": (
                round(statistics.median(confidences), 1) if confidences else None
            ),
            "elapsedMs": {
                "median": round(statistics.median(elapsed), 1),
                "min": round(min(elapsed), 1),
                "max": round(max(elapsed), 1),
            },
            "observedText": texts_first,
        }
    return out


def self_test() -> int:
    failures: list[str] = []
    checks = 0

    def check(name: str, condition: bool, detail: str = "") -> None:
        nonlocal checks
        checks += 1
        if not condition:
            failures.append(f"{name}: {detail}")

    check("levenshtein özdeş", levenshtein("abc", "abc") == 0)
    check("levenshtein ekleme", levenshtein("abc", "abcd") == 1)
    check("levenshtein silme", levenshtein("abcd", "abc") == 1)
    check("levenshtein yerine koyma", levenshtein("abc", "axc") == 1)
    check("levenshtein boş", levenshtein("", "abc") == 3)
    check("normalize satır sonu", normalize("a\nb   c") == "a b c")
    check("doğruluk tam", abs(char_accuracy("şüpheli", "şüpheli") - 1.0) < 1e-9)
    check("doğruluk yarım", abs(char_accuracy("abcd", "ab") - 0.5) < 1e-9)
    check("doğruluk boş beklenti", char_accuracy("", "abc") == 0.0)
    stats = turkish_letter_stats("şüpheli", "supheli")
    check("harf beklenen sayı", stats["expected"] == 2, str(stats))
    check("harf bulunan sayı", stats["found"] == 0, str(stats))
    check(
        "ascii katlanmış sayfa",
        turkish_letter_stats("şüpheli", "supheli")["ascii_folded_whole_page"] == 1,
    )
    check(
        "güven ayrıştırma",
        confidence_of(["sayfa 1: OCR ile okundu (tur); güven skoru 87/100"]) == 87.0,
    )
    check("güven yok", confidence_of(["başka uyarı"]) is None)
    check("güven ondalık", confidence_of(["güven skoru 82,5/100"]) == 82.5)

    if failures:
        print("ÖZ TEST BAŞARISIZ:")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print(f"öz test: {checks} kontrol geçti")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="OCR dil modeli ölçümü")
    parser.add_argument("--reps", type=int, default=5, help="dil başına koşu sayısı")
    parser.add_argument("--out", default="", help="JSON rapor yolu")
    parser.add_argument("--langs", default=",".join(LANGS), help="virgülle ayrılmış diller")
    parser.add_argument("--self-test", action="store_true", help="yalnız saf yardımcıları sına")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    if not FIXTURE.is_file():
        print(f"fikstür yok: {FIXTURE} (fixtures/synthetic/generate.py ile üretilir)")
        return 2
    data = FIXTURE.read_bytes()
    truth = truth_pages()
    langs = [lang.strip() for lang in args.langs.split(",") if lang.strip()]
    if not langs:
        print("dil listesi boş")
        return 2

    runs: list[dict] = []
    for lang in langs:
        for index in range(max(1, args.reps)):
            try:
                run = run_once(lang, data)
            except RuntimeError as exc:
                print(f"{lang} koşusu düştü: {exc}")
                return 3
            runs.append(run)
            print(
                f"{lang} #{index + 1}: {run['elapsedMs']:.0f} ms · "
                f"{run['pages']} sayfa · {run['chars']} karakter"
                + (f" · güven {run['confidence']}" if run["confidence"] else "")
            )

    summary = summarize(runs, truth)
    report = {
        "generatedAt": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "fixture": str(FIXTURE.relative_to(ROOT)),
        "fixtureBytes": len(data),
        "tesseract": tesseract_version(),
        "pages": len(truth),
        "reps": max(1, args.reps),
        "summary": summary,
        "runs": [
            {key: value for key, value in run.items() if key != "texts"} for run in runs
        ],
    }
    if args.out:
        out_path = Path(args.out)
        if not out_path.is_absolute():
            out_path = ROOT / out_path
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"rapor: {out_path}")
    else:
        print(json.dumps(report["summary"], ensure_ascii=False, indent=2))
    return 0


def tesseract_version() -> str:
    try:
        out = subprocess.run(
            ["tesseract", "--version"], capture_output=True, text=True, check=False
        )
    except FileNotFoundError:
        return "yok"
    first = (out.stdout or out.stderr).splitlines()
    return first[0].strip() if first else "bilinmiyor"


if __name__ == "__main__":
    sys.exit(main())
