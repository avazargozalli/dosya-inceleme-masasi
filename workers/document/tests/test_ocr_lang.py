"""OCR dili: varsayılanın tek kaynağı ve **eksik dil paketi** kapısı.

Ölçülen kusur (F15, `scripts/ocr_lang_bench.py`): `eng` modeli Türkçe
diyakritiklerin **0/79**'ını koruyor (karakter doğruluğu %82,8), `tur` ve
`tur+eng` **79/79** (%100). Ürün varsayılanı `eng` olduğu için aynı belge
yollardan birinde sessizce bozuluyordu. İkinci kusur: eksik dil paketi
`E_OCR_FAILED` + "dil paketi eksik olabilir" **tahminiyle** kapanıyordu —
hangi paketin eksik olduğu söylenmiyordu.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

from dim_worker import engines
from dim_worker.engines import DEFAULT_OCR_LANG, EngineError

REPO = Path(__file__).resolve().parents[3]


def _rust_default() -> str:
    source = (REPO / "crates/storage/src/queue.rs").read_text(encoding="utf-8")
    match = re.search(r'pub const DEFAULT_OCR_LANG: &str = "([^"]+)";', source)
    assert match, "queue.rs içinde DEFAULT_OCR_LANG bulunamadı"
    return match.group(1)


def test_default_language_matches_the_rust_single_source():
    """Python aynası Rust sabitiyle birebir aynı olmalı (biri kayarsa kırmızı)."""
    assert DEFAULT_OCR_LANG == _rust_default()
    assert DEFAULT_OCR_LANG == "tur+eng"


def test_default_language_includes_turkish():
    codes = [code.strip() for code in DEFAULT_OCR_LANG.split("+")]
    assert "tur" in codes, "varsayılan Türkçe modelini içermeli"


@pytest.fixture()
def installed(monkeypatch):
    """Kurulu dil listesini sabitler (gerçek tesseract'a bağımlı olmadan)."""

    def _set(langs: list[str]) -> None:
        monkeypatch.setattr(engines, "_AVAILABLE_LANGS", langs, raising=True)

    return _set


def test_missing_pack_is_named_not_guessed(installed):
    installed(["eng", "osd"])
    with pytest.raises(EngineError) as caught:
        engines._ensure_ocr_languages("tur+eng")
    error = caught.value
    assert error.code == "E_OCR_LANG_MISSING"
    assert "tur" in error.message, "eksik paketin adı yazılmalı"
    assert "eng, osd" in error.message, "kurulu diller yazılmalı"


def test_combined_language_reports_only_the_missing_part(installed):
    installed(["eng", "osd", "tur"])
    engines._ensure_ocr_languages("tur+eng")  # tamamı kurulu: hata yok

    installed(["tur"])
    with pytest.raises(EngineError) as caught:
        engines._ensure_ocr_languages("tur+eng")
    assert "eng" in caught.value.message
    assert "tur," not in caught.value.message.split("eksik:")[1].split("·")[0]


def test_empty_language_is_rejected(installed):
    installed(["eng"])
    with pytest.raises(EngineError) as caught:
        engines._ensure_ocr_languages("  ")
    assert caught.value.code == "E_OCR_LANG"


def test_unreadable_language_list_makes_no_accusation(installed):
    """Liste alınamıyorsa (tesseract yok) dil suçlaması yapılmaz."""
    installed([])
    engines._ensure_ocr_languages("tur+eng")  # hata beklenmiyor


def test_gate_runs_before_the_engine(installed, monkeypatch):
    """Eksik dilde OCR motoru **hiç çağrılmaz** (boşa iş + yanıltıcı hata yok)."""
    installed(["eng"])

    class FakePyTesseract:
        Output = type("Output", (), {"DICT": "dict"})

        @staticmethod
        def image_to_data(*args, **kwargs):  # pragma: no cover - çağrılmamalı
            raise AssertionError("dil kapısı geçilmeden motor çağrıldı")

    monkeypatch.setitem(__import__("sys").modules, "pytesseract", FakePyTesseract)
    monkeypatch.setattr(engines.shutil, "which", lambda _name: "/usr/bin/tesseract")

    with pytest.raises(EngineError) as caught:
        engines._ocr_text_confidence_and_blocks(object(), "tur")
    assert caught.value.code == "E_OCR_LANG_MISSING"


def test_available_languages_are_cached(monkeypatch):
    """Liste süreç başına bir kez okunur; her sayfada `tesseract` başlatılmaz."""
    calls: list[int] = []

    class FakePyTesseract:
        @staticmethod
        def get_languages(config=""):  # noqa: ARG004 - imza uyumu
            calls.append(1)
            return ["eng", "tur"]

    monkeypatch.setitem(__import__("sys").modules, "pytesseract", FakePyTesseract)
    monkeypatch.setattr(engines, "_AVAILABLE_LANGS", None, raising=True)

    assert engines.available_ocr_languages() == ["eng", "tur"]
    assert engines.available_ocr_languages() == ["eng", "tur"]
    assert len(calls) == 1, "ikinci çağrı önbellekten gelmeli"


def test_default_language_keeps_turkish_diacritics_end_to_end():
    """Ürün varsayılanı **gerçek** OCR'da Türkçe harfleri korur (uçtan uca).

    Ölçüm (F15): `eng` 0/79, `tur+eng` 79/79. Bu test varsayılanı hiç
    değiştirmeden çağırır — ürün yolunun kendisi sınanır.
    """
    import shutil

    pytest.importorskip("pymupdf")
    pytest.importorskip("pytesseract")
    if shutil.which("tesseract") is None:
        pytest.skip("tesseract kurulu değil")
    installed_langs = engines.available_ocr_languages()
    if installed_langs and "tur" not in installed_langs:
        pytest.skip("tur dil paketi kurulu değil")

    from dim_worker.engines import extract_pdf

    fixture = REPO / "fixtures/synthetic/turkish-scan.pdf"
    assert fixture.is_file(), "Türkçe tarama fikstürü depoda olmalı"

    result = extract_pdf(fixture.read_bytes(), ocr=True)
    text = " ".join(page.text for page in result.pages)
    letters = "şğıİçöüŞĞÇÖÜ"
    kept = sum(1 for ch in text if ch in letters)
    assert "Şüpheli" in text, f"beklenen sözcük okunamadı: {text[:120]!r}"
    assert kept >= 70, f"Türkçe harf korunmadı: {kept}/79 (varsayılan dil eski mi?)"
