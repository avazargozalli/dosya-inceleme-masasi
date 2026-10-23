#!/usr/bin/env python3
"""F02-T12 duman testi: sentetik PDF → gerçek worker → metin.

Rust çekirdeği olmadan worker sözleşmesini uçtan uca doğrular.
CI'da `pip install -e workers/document[pdf]` sonrası çalışır.
"""

from __future__ import annotations

import base64
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "fixtures" / "synthetic"))

from generate import generate_pdf  # noqa: E402


def send(proc: subprocess.Popen, payload: dict) -> dict:
    assert proc.stdin is not None and proc.stdout is not None
    proc.stdin.write(json.dumps(payload, ensure_ascii=False).encode() + b"\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise SystemExit("worker yanıt vermeden kapandı")
    return json.loads(line)


def main() -> None:
    pdf = generate_pdf(ROOT / "fixtures" / "generated" / "duman.pdf")
    data = pdf.read_bytes()

    proc = subprocess.Popen(
        [sys.executable, "-m", "dim_worker.worker"],
        cwd=ROOT / "workers" / "document",
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        pong = send(proc, {"v": 1, "id": "1", "method": "ping", "params": {}})
        assert pong["ok"] is True and pong["result"]["pong"] is True, pong

        caps = send(proc, {"v": 1, "id": "2", "method": "capabilities", "params": {}})
        assert caps["ok"] is True, caps
        assert caps["result"]["protocolVersion"] == 1, caps

        result = send(
            proc,
            {
                "v": 1,
                "id": "3",
                "method": "extract_text",
                "params": {"contentBase64": base64.b64encode(data).decode(), "mime": "application/pdf"},
            },
        )
        assert result["ok"] is True, result
        text = result["result"]["pages"][0]["text"]
        assert "Kira sozlesmesi 2026" in text, text
        assert result["result"]["engine"] == "pymupdf", result["result"]

        bye = send(proc, {"v": 1, "id": "4", "method": "shutdown", "params": {}})
        assert bye["ok"] is True, bye
    finally:
        proc.wait(timeout=10)

    print("duman testi geçti: ping + capabilities + PDF metni + shutdown")
    print(f"metin: {text.strip()[:60]!r}")


if __name__ == "__main__":
    main()
