"""Rapor üretimi: DOCX ve PDF (gömülü Unicode font).

`engines` modülünden ayrıldı; PyMuPDF tembel yüklenir (döngüsel import yok).
"""

from __future__ import annotations

import html
import io
from typing import Any


def _pymupdf():
    from .engines import _pymupdf as loader

    return loader()


def render_report(payload: dict[str, Any]) -> bytes:
    """Duruşma/devir raporunu DOCX olarak üretir. Girdi çağıran çekirdekten gelir;
    worker veritabanına erişmez. İçerik düz metindir, HTML çalıştırılmaz."""
    try:
        import docx  # type: ignore
        from docx.shared import Pt  # type: ignore
    except ImportError as exc:
        raise EngineError("E_DOCX_MISSING", "python-docx bu worker paketinde yok") from exc

    document = docx.Document()
    document.add_heading(str(payload.get("title") or "Dosya raporu"), 0)
    meta = payload.get("meta") or {}
    if isinstance(meta, dict):
        for key, value in meta.items():
            if value not in (None, ""):
                p = document.add_paragraph()
                p.add_run(f"{key}: ").bold = True
                p.add_run(str(value))

    for section in payload.get("sections") or []:
        if not isinstance(section, dict):
            continue
        document.add_heading(str(section.get("heading") or ""), level=1)
        rows = section.get("rows") or []
        if rows and isinstance(rows[0], list):
            table = document.add_table(rows=1, cols=len(rows[0]))
            table.style = "Light Grid Accent 1"
            header = table.rows[0].cells
            for index, value in enumerate(rows[0]):
                header[index].text = str(value)
            for row in rows[1:]:
                cells = table.add_row().cells
                for index, value in enumerate(row[: len(cells)]):
                    cells[index].text = str(value)
        for line in section.get("lines") or []:
            document.add_paragraph(str(line))

    footer = document.add_paragraph()
    run = footer.add_run("Bu rapor Dosya İnceleme Masası tarafından üretildi; hukuki sonuç değildir.")
    run.font.size = Pt(8)
    buffer = io.BytesIO()
    document.save(buffer)
    return buffer.getvalue()


def _report_html(payload: dict[str, Any]) -> str:
    import html as _html

    esc = _html.escape
    parts = [f"<h1>{esc(str(payload.get('title') or 'Dosya raporu'))}</h1>"]
    meta = payload.get("meta") or {}
    if isinstance(meta, dict):
        parts.append("<p>")
        for key, value in meta.items():
            if value not in (None, ""):
                parts.append(f"<b>{esc(str(key))}:</b> {esc(str(value))}<br/>")
        parts.append("</p>")

    for section in payload.get("sections") or []:
        if not isinstance(section, dict):
            continue
        parts.append(f"<h2>{esc(str(section.get('heading') or ''))}</h2>")
        rows = section.get("rows") or []
        if rows and isinstance(rows[0], list):
            parts.append('<table border="1" cellspacing="0" cellpadding="4">')
            header = rows[0]
            parts.append("<tr>" + "".join(f"<th>{esc(str(c))}</th>" for c in header) + "</tr>")
            for row in rows[1:]:
                parts.append("<tr>" + "".join(f"<td>{esc(str(c))}</td>" for c in row) + "</tr>")
            parts.append("</table>")
        for line in section.get("lines") or []:
            parts.append(f"<p>{esc(str(line))}</p>")
    parts.append(
        "<p><i>Bu rapor Dosya İnceleme Masası tarafından üretildi; hukuki sonuç değildir.</i></p>"
    )
    return "".join(parts)


def render_report_pdf(payload: dict[str, Any]) -> bytes:
    """Raporu PDF olarak üretir. HTML kaçışlanır; kullanıcı verisi HTML olarak
    çalıştırılmaz. Uzun içerik `Story` ile sayfalara akıtılır."""
    pymupdf = _pymupdf()
    html = _report_html(payload)
    story = pymupdf.Story(html=html)
    buffer = io.BytesIO()
    writer = pymupdf.DocumentWriter(buffer)
    rect = pymupdf.paper_rect("a4")
    where = rect + (36, 36, -36, -36)
    more = True
    pages = 0
    while more:
        device = writer.begin_page(rect)
        more, _ = story.place(where)
        story.draw(device)
        writer.end_page()
        pages += 1
        if pages > 200:
            break
    writer.close()
    return buffer.getvalue()
