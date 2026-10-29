#!/usr/bin/env python3
"""F12 — gerçek PDF düzeni fikstürü ve **kayıtlı çıkarım** anlık görüntüsü.

Neden ayrı üretici: `generate.py` (paylaşılan) başka bir ajanın elinde; bu
fikstür yalnız F12 metin içi alıntı doğrulamasının **gerçek metin katmanıyla**
sınanması için var. Depoya iki dosya yazılır:

- `fixtures/synthetic/intext-layout.pdf` — 3 sayfa, gömülü Unicode font
  (MuPDF `Story` hattı; Türkçe diyakritik + tipografik tırnak korunur).
  Vakalar: sayfa içi satır sonu tiresi, sayfa sınırını aşan tire, tiresiz
  sayfa sınırı, `fl` ligatürü (`taraﬂar` → U+FB02).
- `fixtures/synthetic/intext-layout.extracted.json` — aynı PDF'in **ürün
  worker'ıyla** (`dim_worker.engines.extract_pdf`) çıkarılmış hâli. Rust
  testi CI'da bu kaydı yükler; böylece Python/PyMuPDF kurulu olmayan CI'da da
  gerçek çıkarım metni üzerinden alıntı katmanları sınanır.

Yeniden üretmek için (worker venv'i gerekir):

    workers/document/.venv/bin/python fixtures/synthetic/generate_intext.py

Çıkarım metni değişirse (PyMuPDF sürümü) anlık görüntü de değişir; canlı
doğrulama testi (`crates/ingest/tests/intext_layout_live.rs`, `#[ignore]`)
kayıtla canlı çıkarımı karşılaştırır ve farkı **gürültüye yazmaz**.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "workers" / "document"))

import fitz  # noqa: E402

from dim_worker import engines  # noqa: E402

PDF_PATH = Path(__file__).with_name("intext-layout.pdf")
SNAPSHOT_PATH = Path(__file__).with_name("intext-layout.extracted.json")

# Her sayfa ayrı `Story`: sayfa sonları **birebir** denetlenir (Story kendi
# kendine sayfa bölerse vakalar kayar). `line-height` ve punto sabit.
PAGES: list[str] = [
    # 0 — sayfa içi tire bölünmesi (bedel-\nlini) + sayfa sonunda tire
    """<div style="font-size:11pt;line-height:1.45">
<p>KARAR GEREKÇESİ</p>
<p>Davacı vekili 12.03.2026 tarihli dilekçesinde “bedel-<br/>lini ödemediğini” ileri sürmüştür.</p>
<p>Mahkeme dosyadaki delilleri değerlendirmiş, itirazın<br/>reddine karar ve-</p>
</div>""",
    # 1 — sayfa sınırından gelen tire devamı + satır sonu (tiresiz) + ligatür
    """<div style="font-size:11pt;line-height:1.45">
<p>rilmiştir. Hüküm davalı tarafa 20.04.2026 tarihinde<br/>tebliğ edilmiştir.</p>
<p>Gerekçenin tamamı aşağıdadır: ödeme yapılmadığı<br/>sabit olup faiz işletilmesine yer olmadığına karar<br/>verilmiştir.</p>
<p>Kararın gerekçesi</p>
</div>""",
    # 2 — tiresiz sayfa sınırı devamı + ligatür (`taraﬂar`)
    """<div style="font-size:11pt;line-height:1.45">
<p>şöyledir: taraflar arasındaki sözleşme 01.02.2026<br/>tarihinde kurulmuştur.</p>
</div>""",
]


def build_pdf(path: Path) -> None:
    writer = fitz.DocumentWriter(str(path))
    rect = fitz.paper_rect("a4")
    for html in PAGES:
        story = fitz.Story(html=html)
        device = writer.begin_page(rect)
        story.place(rect + (50, 50, -50, -50))
        story.draw(device)
        writer.end_page()
    writer.close()


def main() -> int:
    build_pdf(PDF_PATH)
    data = PDF_PATH.read_bytes()
    extraction = engines.extract_pdf(data)
    pages = [
        {
            "index": page.index,
            "text": page.text,
            "width": page.width,
            "height": page.height,
            "rotation": page.rotation,
            "coordinateSpace": page.coordinate_space,
            "label": page.label,
        }
        for page in extraction.pages
    ]
    snapshot = {
        "fixture": PDF_PATH.name,
        "fixtureBytes": len(data),
        "fixtureSha256": hashlib.sha256(data).hexdigest(),
        "producer": "fixtures/synthetic/generate_intext.py",
        "engine": extraction.engine,
        "engineVersion": extraction.engine_version,
        "pages": pages,
    }
    SNAPSHOT_PATH.write_text(
        json.dumps(snapshot, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"yazıldı: {PDF_PATH.name} ({len(data)} bayt, {len(pages)} sayfa)")
    print(f"yazıldı: {SNAPSHOT_PATH.name} ({SNAPSHOT_PATH.stat().st_size} bayt)")
    for page in pages:
        print(f"  sayfa {page['index']}: {page['text']!r}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
