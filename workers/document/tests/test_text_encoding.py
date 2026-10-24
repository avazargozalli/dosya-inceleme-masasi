"""F06-T07: düz metin kodlama düzeltmesi (otomatik algılama + açık seçim).

Türkçe hukuk metinleri sık sık Windows-1254 ile üretilir; katı UTF-8 çözümü
bunlarda hata veriyordu. Testler hem doğru çözümü hem de "sessiz mojibake yok"
kuralını doğrular.
"""

from __future__ import annotations

import base64
import codecs
from typing import Any

import pytest

from dim_worker.engines import EngineError, extract, extract_plain
from dim_worker.worker import handle

TURKISH = "Mahkeme, itirazın reddine karar verdi. ĞŞİÇÖÜğşıçöü"


def _frame(**params: Any) -> dict[str, Any]:
    return {"v": 1, "id": "req-1", "method": "extract_text", "params": params}


def test_cp1254_turkish_is_decoded_without_mojibake():
    data = TURKISH.encode("cp1254")
    result = extract_plain(data)
    assert result.pages[0].text == TURKISH
    assert result.engine_version == "2"
    assert any("Windows-1254" in warning for warning in result.warnings)


def test_utf8_bom_is_stripped_and_reported():
    data = codecs.BOM_UTF8 + "Dilekçe ığüş".encode()
    result = extract_plain(data)
    assert result.pages[0].text == "Dilekçe ığüş"
    assert any("utf-8" in warning for warning in result.warnings)


def test_utf16_bom_is_decoded():
    data = "Dilekçe ığüş".encode("utf-16")
    result = extract_plain(data)
    assert result.pages[0].text == "Dilekçe ığüş"
    assert any("utf-16" in warning for warning in result.warnings)


def test_plain_ascii_stays_utf8_without_noise():
    result = extract_plain(b"duz metin")
    assert result.pages[0].text == "duz metin"
    assert result.warnings == []


def test_explicit_utf8_rejects_cp1254_and_asks_for_manual_choice():
    with pytest.raises(EngineError) as exc:
        extract_plain(TURKISH.encode("cp1254"), encoding="utf-8")
    assert exc.value.code == "E_TEXT_ENCODING"
    assert "elle" in exc.value.message


def test_explicit_windows_1254_is_honored():
    result = extract_plain(TURKISH.encode("cp1254"), encoding="windows-1254")
    assert result.pages[0].text == TURKISH
    # Açık seçimde "otomatik belirlendi" uyarısı üretilmez.
    assert not any("otomatik" in warning for warning in result.warnings)


def test_unknown_encoding_name_is_rejected():
    with pytest.raises(EngineError) as exc:
        extract_plain(b"metin", encoding="klingon")
    assert exc.value.code == "E_TEXT_ENCODING"
    assert "bilinmeyen kodlama" in exc.value.message


def test_binary_content_is_not_decoded_as_text():
    with pytest.raises(EngineError) as exc:
        extract_plain(bytes(range(0, 32)) + b"\x00\x01\x02")
    assert exc.value.code == "E_TEXT_ENCODING"
    assert "ikili" in exc.value.message


def test_dispatch_defaults_to_auto_detection():
    data = TURKISH.encode("cp1254")
    result = extract(data, "text/plain")
    assert result.pages[0].text == TURKISH


def test_protocol_omitting_encoding_uses_auto():
    data = TURKISH.encode("cp1254")
    frames, close = handle(
        _frame(contentBase64=base64.b64encode(data).decode(), mime="text/plain")
    )
    assert close is False
    assert isinstance(frames, list)
    first = frames[0]
    assert first["ok"] is True
    assert first["result"]["pages"][0]["text"] == TURKISH


def test_protocol_explicit_encoding_reaches_engine():
    data = TURKISH.encode("cp1254")
    with pytest.raises(EngineError) as exc:
        handle(
            _frame(
                contentBase64=base64.b64encode(data).decode(),
                mime="text/plain",
                encoding="utf-8",
            )
        )
    assert exc.value.code == "E_TEXT_ENCODING"
