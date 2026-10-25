#!/usr/bin/env python3
"""Paketlenmiş belge worker'ını **çevrimdışı** koşturur (F01-T02).

Ne kanıtlıyor:
  1. Paket kendi kendine yeter — depo venv'i, `PYTHONPATH`, Python kurulumu
     olmadan çalışır (ortam `env` ile asgariye indirilir, `PYTHONPATH` kasıtlı
     olarak bozuk bir yola ayarlanır).
  2. Ağ **kapalıyken** çalışır (macOS `sandbox-exec`; profil `deny network*`).
  3. Gerçek iş yapar: sentetik PDF'ten metin çıkarır, sentetik DOCX'in
     envanterini çıkarır — yalnız `ping` ile "çalışıyor" denmez.
  4. Sandbox'ın gerçekten ağı kapattığı **ayrıca** ölçülür (dedektör kanıtı):
     aynı profilde bir TCP bağlantısı denenir ve başarısız olması beklenir.
     Aksi hâlde "çevrimdışı" iddiası kanıtsız kalırdı.

Kullanım:
    python3 scripts/verify-worker-offline.py
    python3 scripts/verify-worker-offline.py --json
    python3 scripts/verify-worker-offline.py --no-offline   # yalnız izolasyon

Çıkış kodu: 0 = tüm kontroller geçti, 1 = kusur, 2 = koşulamadı (paket yok).
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_PACKAGE = ROOT / "apps" / "desktop" / "src-tauri" / "resources" / "worker"
PDF_FIXTURE = ROOT / "fixtures" / "synthetic" / "redaction-hidden.pdf"
DOCX_FIXTURE = ROOT / "fixtures" / "synthetic" / "docx-inventory.docx"
SANDBOX_PROFILE = "(version 1)(allow default)(deny network*)"


def _frames() -> list[dict]:
    pdf = base64.b64encode(PDF_FIXTURE.read_bytes()).decode("ascii")
    docx = base64.b64encode(DOCX_FIXTURE.read_bytes()).decode("ascii")
    return [
        {"v": 1, "id": "1", "method": "ping", "params": {}},
        {"v": 1, "id": "2", "method": "capabilities", "params": {}},
        {
            "v": 1,
            "id": "3",
            "method": "extract_text",
            "params": {"mime": "application/pdf", "contentBase64": pdf},
        },
        {
            "v": 1,
            "id": "4",
            "method": "inventory",
            "params": {
                "mime": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "contentBase64": docx,
            },
        },
        {"v": 1, "id": "5", "method": "shutdown", "params": {}},
    ]


def _command(binary: Path, offline: bool) -> list[str]:
    if offline:
        return ["sandbox-exec", "-p", SANDBOX_PROFILE, str(binary)]
    return [str(binary)]


def _decode(stdout: str) -> dict[str, dict]:
    responses: dict[str, dict] = {}
    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            frame = json.loads(line)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"HATA: worker yanıtı JSON değil ({exc}): {line[:120]}")
        if isinstance(frame, dict) and isinstance(frame.get("id"), str):
            responses[frame["id"]] = frame
    return responses


def _network_denied(offline: bool, workdir: Path) -> tuple[bool | None, str]:
    """Sandbox gerçekten ağı kapatıyor mu? Aynı profilde TCP denenir.

    Prob **temiz** ortamda koşar: worker'a verdiğimiz kasıtlı bozuk
    `PYTHONHOME`/`PYTHONPATH` probu çökertirse "ağ açık" gibi okunurdu
    (ilk koşuda tam olarak bu yanlış sonucu verdi). Bu yüzden prob kendi
    işaretini yazar; başka her sonuç "ölçülemedi" sayılır.
    """
    if not offline:
        return None, "sandbox kullanılmadı"
    probe = (
        "import socket, sys\n"
        "try:\n"
        "    socket.setdefaulttimeout(3)\n"
        "    socket.create_connection(('1.1.1.1', 443), 3)\n"
        "except OSError:\n"
        "    print('DENIED')\n"
        "    sys.exit(0)\n"
        "print('OPEN')\n"
        "sys.exit(1)\n"
    )
    result = subprocess.run(
        ["sandbox-exec", "-p", SANDBOX_PROFILE, sys.executable, "-c", probe],
        cwd=workdir,
        env={"PATH": "/usr/bin:/bin", "HOME": str(workdir)},
        capture_output=True,
        text=True,
        check=False,
    )
    out = result.stdout.strip()
    if "DENIED" in out:
        return True, "aynı profilde TCP bağlantısı reddedildi"
    if "OPEN" in out:
        return False, "ağ AÇIKTI"
    return None, f"prob koşmadı (exit={result.returncode})"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Paketlenmiş worker çevrimdışı koşusu")
    parser.add_argument("--package-dir", type=Path, default=DEFAULT_PACKAGE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--no-offline",
        action="store_true",
        help="ağ kapalı sandbox kullanma (yalnız ortam izolasyonu ölçülür)",
    )
    args = parser.parse_args(argv)

    package = args.package_dir.resolve()
    binary = package / "dim-worker"
    if not binary.is_file():
        print(f"HATA: paketlenmiş worker yok: {binary}", file=sys.stderr)
        print("Önce: scripts/build-worker.sh", file=sys.stderr)
        return 2
    for fixture in (PDF_FIXTURE, DOCX_FIXTURE):
        if not fixture.is_file():
            print(f"HATA: fikstür yok: {fixture}", file=sys.stderr)
            return 2

    offline = platform.system() == "Darwin" and not args.no_offline
    if not offline and not args.no_offline:
        print("NOT: bu platformda sandbox yok → ağ kapatma ölçülmedi", file=sys.stderr)

    workdir = Path(tempfile.mkdtemp(prefix="dim-offline-probe-"))
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(workdir),
        "TMPDIR": str(workdir),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        # Kasıtlı: depo kaynakları görünmesin. Paket bunu yok saymalı.
        "PYTHONPATH": str(workdir / "olmayan-python-yolu"),
        "PYTHONHOME": str(workdir / "olmayan-python-home"),
    }
    payload = "\n".join(json.dumps(frame, ensure_ascii=False) for frame in _frames()) + "\n"

    started = time.monotonic()
    try:
        result = subprocess.run(
            _command(binary, offline),
            input=payload,
            cwd=workdir,
            env=env,
            capture_output=True,
            text=True,
            timeout=180,
            check=False,
        )
    finally:
        pass
    elapsed = time.monotonic() - started

    checks: list[dict] = []
    def check(name: str, ok: bool, detail: str = "") -> None:
        checks.append({"name": name, "ok": ok, "detail": detail})

    check("worker çıkış kodu", result.returncode == 0, f"exit={result.returncode}")
    responses = _decode(result.stdout) if result.stdout.strip() else {}

    pong = responses.get("1", {})
    check("ping", pong.get("ok") is True and pong.get("result", {}).get("pong") is True,
          f"workerVersion={pong.get('result', {}).get('workerVersion')}")

    caps = responses.get("2", {}).get("result", {})
    pdf_engine = caps.get("engines", {}).get("pdf", {})
    check("pdf motoru pakette", pdf_engine.get("available") is True,
          f"sürüm={pdf_engine.get('version')}")

    extraction = responses.get("3", {}).get("result", {})
    pages = extraction.get("pages") or []
    text_chars = sum(len(page.get("text") or "") for page in pages)
    check("pdf metni çıkarıldı", bool(pages) and text_chars > 0,
          f"{len(pages)} sayfa · {text_chars} karakter · motor={extraction.get('engine')}")

    inventory = responses.get("4", {}).get("result", {})
    structure = inventory.get("structure") or {}
    check("docx envanteri çıkarıldı",
          inventory.get("kind") == "docx" and int(structure.get("paragraphs") or 0) > 0,
          f"kind={inventory.get('kind')} · paragraf={structure.get('paragraphs')} · "
          f"ek={len(inventory.get('attachments') or [])}")

    check("shutdown", responses.get("5", {}).get("ok") is True)

    denied, denied_detail = _network_denied(offline, workdir)
    if denied is not None or offline:
        check("sandbox ağı gerçekten kapattı", denied is True, denied_detail)

    shutil.rmtree(workdir, ignore_errors=True)

    failed = [item for item in checks if not item["ok"]]
    report = {
        "ok": not failed,
        "offline": offline,
        "platform": f"{platform.system().lower()}-{platform.machine().lower()}",
        "package": str(package),
        "binaryBytes": binary.stat().st_size,
        "durationMs": round(elapsed * 1000),
        "checks": checks,
        "warnings": [line for line in result.stderr.splitlines() if line.strip()][:10],
    }

    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print(f"paket: {package}")
        print(f"kip: {'çevrimdışı (ağ kapalı sandbox)' if offline else 'yalnız ortam izolasyonu'} · "
              f"{round(elapsed * 1000)} ms · ikili {report['binaryBytes']} bayt")
        for item in checks:
            mark = "✓" if item["ok"] else "✗"
            suffix = f" — {item['detail']}" if item["detail"] else ""
            print(f"  {mark} {item['name']}{suffix}")
        if report["warnings"]:
            print("  uyarılar:", "; ".join(report["warnings"]))
        print("SONUÇ:", "TAMAM" if report["ok"] else "KUSURLU")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
