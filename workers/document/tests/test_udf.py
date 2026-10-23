"""UDF (UYAP ZIP/XML) ayrıştırma testleri: güvenlik, offset ve envanter."""

from __future__ import annotations

import io
import zipfile

import pytest

from dim_worker.engines import EngineError, extract, extract_udf
from dim_worker.udf import UdfError, extract_udf_document


def _zip(entries: dict[str, bytes]) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, payload in entries.items():
            archive.writestr(name, payload)
    return buffer.getvalue()


def _udf(content_xml: str, extra: dict[str, bytes] | None = None) -> bytes:
    entries = {"content.xml": content_xml.encode("utf-8")}
    entries.update(extra or {})
    return _zip(entries)


def _paragraphs_xml(paragraphs: list[str], extra_body: str = "") -> str:
    pool = "\n".join(paragraphs)
    offset = 0
    refs = []
    for paragraph in paragraphs:
        refs.append(f'<paragraph><content startOffset="{offset}" length="{len(paragraph)}"/></paragraph>')
        offset += len(paragraph) + 1  # ayırıcı \n bir karakterdir
    return (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<template format_id="1.8">'
        f"<content><![CDATA[{pool}]]></content>"
        "<properties><page mediaSizeName=\"1\"/></properties>"
        f"<elements><body>{''.join(refs)}{extra_body}</body></elements>"
        "<styles></styles>"
        "</template>"
    )


def test_udf_text_uses_scalar_offsets_with_turkish_and_emoji():
    paragraphs = ["İtirazın reddine karar verildi.", "😀 Emoji satırı", "Son paragraf."]
    result = extract_udf(_udf(_paragraphs_xml(paragraphs)))

    assert result.engine == "uyap-udf"
    assert result.pages[0].text == "\n".join(paragraphs)
    assert any("sayfa ayrımı doğrulanmadı" in warning for warning in result.warnings)


def test_udf_unknown_element_is_reported_not_silently_dropped():
    xml = _paragraphs_xml(["Metin"], extra_body="<bilinmeyen foo=\"1\"/>")
    document = extract_udf_document(_udf(xml))
    assert any("bilinmeyen×1" in warning for warning in document.warnings)


def test_udf_signature_is_only_detected_not_verified():
    xml = _paragraphs_xml(["İmzalı dilekçe"])
    document = extract_udf_document(_udf(xml, {"signature.p7s": b"\x30\x80"}))
    assert document.signature_status == "tespit edildi — denetlenmedi"
    assert any("denetlenmedi" in warning for warning in document.warnings)
    assert not any("doğrulandı" in warning for warning in document.warnings)


def test_udf_embedded_objects_are_inventoried():
    xml = _paragraphs_xml(["Ekli belge"])
    document = extract_udf_document(_udf(xml, {"images/1.png": b"\x89PNG\r\n"}))
    assert "images/1.png" in document.embedded
    assert any("gömülü nesne" in warning for warning in document.warnings)


def test_udf_rejects_external_entity_declaration():
    xml = (
        '<?xml version="1.0"?>'
        '<!DOCTYPE template [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>'
        "<template><content><![CDATA[metin]]></content>"
        "<elements><body><paragraph><content startOffset=\"0\" length=\"5\"/></paragraph></body></elements>"
        "</template>"
    )
    with pytest.raises(UdfError) as exc:
        extract_udf_document(_udf(xml))
    assert exc.value.code == "E_UDF_XML_UNSAFE"


def test_udf_rejects_zip_slip_entry():
    xml = _paragraphs_xml(["Metin"])
    with pytest.raises(UdfError) as exc:
        extract_udf_document(_udf(xml, {"../evil.txt": b"x"}))
    assert exc.value.code == "E_UDF_ZIP_UNSAFE"


def test_udf_requires_content_xml():
    with pytest.raises(UdfError) as exc:
        extract_udf_document(_zip({"document.xml": b"<template/>"}))
    assert exc.value.code == "E_UDF_STRUCTURE"


def test_udf_without_paragraphs_reports_fallback():
    xml = (
        '<template format_id="1.8"><content><![CDATA[Bir cümle.]]></content>'
        '<elements><body><content startOffset="0" length="10"/></body></elements></template>'
    )
    document = extract_udf_document(_udf(xml))
    assert document.text == "Bir cümle."
    assert any("paragraf yapısı bulunamadı" in warning for warning in document.warnings)


def test_udf_dispatch_by_mime_and_by_content():
    data = _udf(_paragraphs_xml(["Dağıtım metni"]))
    assert extract(data, "application/udf").engine == "uyap-udf"
    # MIME genel olsa da içerik UDF ise doğru motor seçilir.
    assert extract(data, "application/octet-stream").engine == "uyap-udf"


def test_udf_broken_xml_is_engine_error():
    with pytest.raises(EngineError) as exc:
        extract_udf(_udf("<template><content>eksik"))
    assert exc.value.code == "E_UDF_XML"
