import { describe, expect, it } from "vitest";
import type { WorkerPackageView } from "./api";
import {
  packageLimits,
  packageStatusText,
  packageSummary,
  packageTone,
  packageWarnings,
  problemText,
  roleLabel,
  roleRows,
  verificationCounts,
} from "./package";

function view(patch: Partial<WorkerPackageView> = {}): WorkerPackageView {
  return {
    found: true,
    packageDir: "/Applications/Dim.app/Contents/Resources/worker",
    note: null,
    manifest: {
      workerVersion: "0.1.0",
      mode: "onedir",
      buildPlatform: "macos-arm64",
      pythonVersion: "3.12.12",
      pyinstallerVersion: "6.11.1",
      entryPoint: "dim-worker",
      totals: { files: 118, symlinks: 18, bytes: 79294775, truncated: false, unsafeLinks: 0 },
      roles: { bootloader: 1, native_library: 74, pdf_engine: 4, other: 10 },
      tesseract: { bundled: false, note: "sistemde kurulu olmalı" },
    },
    verification: {
      verified: 136,
      problemCount: 0,
      problems: [],
      extraCount: 0,
      extra: [],
      totalBytes: 79294775,
      entryPointPresent: true,
      truncated: false,
      ok: true,
    },
    tesseractOnPath: true,
    ...patch,
  };
}

describe("packageStatusText / packageTone", () => {
  it("doğrulanmış paketi yeşil gösterir", () => {
    const sample = view();
    expect(packageStatusText(sample)).toBe("Bütünlük doğrulandı");
    expect(packageTone(sample)).toBe("ok");
  });

  it("kusurlu doğrulamayı kırmızı gösterir", () => {
    const sample = view({
      verification: { ...view().verification!, ok: false, problemCount: 2 },
    });
    expect(packageStatusText(sample)).toBe("Kusurlu");
    expect(packageTone(sample)).toBe("danger");
  });

  it("paket yokluğunu hata değil nötr gösterir", () => {
    const sample = view({ found: false, manifest: null, verification: null, note: "geliştirme kipi" });
    expect(packageStatusText(sample)).toBe("Paket yok");
    expect(packageTone(sample)).toBe("plain");
    expect(packageStatusText(null)).toBe("Doğrulanmadı");
  });

  it("kesilmiş taramayı uyarı tonunda gösterir", () => {
    const sample = view({
      verification: { ...view().verification!, truncated: true, ok: false },
    });
    expect(packageStatusText(sample)).toBe("Sınırda kesildi");
    expect(packageTone(sample)).toBe("warn");
  });
});

describe("packageSummary", () => {
  it("dosya, bağ ve boyutu tek satırda verir", () => {
    expect(packageSummary(view().manifest)).toBe("118 dosya + 18 bağ · 75,6 MiB");
  });

  it("bağ yoksa yalnız dosya yazar", () => {
    const manifest = { ...view().manifest!, totals: { ...view().manifest!.totals, symlinks: 0 } };
    expect(packageSummary(manifest)).toBe("118 dosya · 75,6 MiB");
  });

  it("manifest yoksa tire döner", () => {
    expect(packageSummary(null)).toBe("—");
  });
});

describe("roleRows / roleLabel", () => {
  it("rolleri adetçe sıralar ve Türkçe etiket verir", () => {
    const rows = roleRows(view().manifest);
    expect(rows.map((row) => row.role)).toEqual(["native_library", "other", "pdf_engine", "bootloader"]);
    expect(rows[0]?.label).toBe("Yerel kitaplık");
  });

  it("bilinmeyen rolü olduğu gibi bırakır", () => {
    expect(roleLabel("yeni_rol")).toBe("yeni_rol");
  });

  it("sıfır/negatif sayıları listelemez", () => {
    const manifest = { ...view().manifest!, roles: { bootloader: 1, hayalet: 0 } };
    expect(roleRows(manifest).map((row) => row.role)).toEqual(["bootloader"]);
    expect(roleRows(null)).toEqual([]);
  });
});

describe("problemText / verificationCounts", () => {
  it("sorun kodunu Türkçe etikete çevirir", () => {
    expect(problemText({ path: "dim-worker", problem: "hashChanged" })).toBe("içerik değişti");
    expect(problemText({ path: "x", problem: "sizeChanged", detail: "beklenen 10, diskte 12" })).toBe(
      "boyut değişti (beklenen 10, diskte 12)",
    );
  });

  it("bilinmeyen sorun kodunu gizlemez", () => {
    expect(problemText({ path: "x", problem: "yeniKod" as never })).toBe("yeniKod");
  });

  it("doğrulama yoksa sıfır döner", () => {
    expect(verificationCounts(null)).toEqual({ verified: 0, problems: 0, extra: 0 });
    expect(verificationCounts(view().verification)).toEqual({ verified: 136, problems: 0, extra: 0 });
  });
});

describe("packageWarnings", () => {
  it("temiz pakette uyarı üretmez", () => {
    expect(packageWarnings(view())).toEqual([]);
  });

  it("paket yokluğunda gerekçeyi taşır", () => {
    const sample = view({ found: false, note: "Paket yok: geliştirme kipi", verification: null });
    expect(packageWarnings(sample)).toEqual(["Paket yok: geliştirme kipi"]);
  });

  it("kusur, fazla dosya, eksik giriş noktası ve kesilmeyi ayrı yazar", () => {
    const sample = view({
      verification: {
        ...view().verification!,
        ok: false,
        problemCount: 3,
        problems: [{ path: "a", problem: "hashChanged" }],
        extraCount: 2,
        extra: ["x", "y"],
        entryPointPresent: false,
        truncated: true,
      },
    });
    const warnings = packageWarnings(sample);
    expect(warnings).toContain("3 dosya manifestle uyuşmuyor.");
    expect(warnings).toContain("Manifestte olmayan 2 dosya var (paket beklenenden fazla).");
    expect(warnings).toContain("Giriş noktası ikilisi diskte yok.");
    expect(warnings).toContain("Tarama girdi sınırına takıldı: sonuç eksik olabilir.");
    expect(warnings).toContain("Sorunların yalnız ilk 1 tanesi listeleniyor (toplam 3).");
  });

  it("Tesseract yoksa OCR'ın çalışmayacağını yazar", () => {
    expect(packageWarnings(view({ tesseractOnPath: false }))).toEqual([
      "Tesseract sistemde bulunamadı: OCR çalışmaz (paket Tesseract içermez).",
    ]);
  });

  it("sınır cümlesi imza iddiası taşımaz", () => {
    expect(packageLimits()).toContain("imza değildir");
    expect(packageLimits()).toContain("Tesseract");
  });
});
