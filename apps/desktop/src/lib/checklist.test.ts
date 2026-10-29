import { describe, expect, it } from "vitest";

import {
  CHECKLIST_STATE_HELP,
  CHECKLIST_STATE_LABEL,
  checklistLicenseLabel,
  copiedLabel,
  MISSING_SCOPE_NOTE,
  shortContentHash,
  summarizeChecklist,
  validateChecklistEntry,
} from "./checklist";
import { t } from "./i18n";

describe("validateChecklistEntry", () => {
  it("bulundu durumunda belge bağı ister", () => {
    expect(validateChecklistEntry({ state: "present", linkedDocumentIds: [], note: "" })).toContain(
      "belge",
    );
    expect(
      validateChecklistEntry({ state: "present", linkedDocumentIds: ["doc-1"], note: "" }),
    ).toBeNull();
  });

  it("bulunamadı ve uygulanamaz için gerekçe ister", () => {
    expect(validateChecklistEntry({ state: "missing", linkedDocumentIds: [], note: "  " })).toContain(
      "kapsamı",
    );
    expect(
      validateChecklistEntry({
        state: "missing",
        linkedDocumentIds: [],
        note: "2020-2024 sözleşmeler incelendi",
      }),
    ).toBeNull();
    expect(
      validateChecklistEntry({ state: "not_applicable", linkedDocumentIds: [], note: "" }),
    ).toContain("gerekçe");
  });

  it("bulunamadı durumunda belge bağı kabul etmez", () => {
    expect(
      validateChecklistEntry({
        state: "missing",
        linkedDocumentIds: ["doc-1"],
        note: "İncelendi",
      }),
    ).toContain("yalnız “Bulundu”");
  });

  it("açık durumda belge bağı bırakmaz", () => {
    expect(validateChecklistEntry({ state: "open", linkedDocumentIds: ["doc-1"], note: "" })).toContain(
      "kaldırın",
    );
    expect(validateChecklistEntry({ state: "open", linkedDocumentIds: [], note: "" })).toBeNull();
  });
});

describe("summarizeChecklist", () => {
  it("bulunamadıyı ayrı sayar", () => {
    const summary = summarizeChecklist([
      { state: "present" },
      { state: "missing" },
      { state: "missing" },
      { state: "not_applicable" },
      { state: "open" },
    ]);
    expect(summary).toEqual({ total: 5, open: 1, present: 1, missing: 2, notApplicable: 1 });
  });
});

describe("künye", () => {
  it("sürüm ve lisansı birlikte gösterir", () => {
    const label = checklistLicenseLabel({
      licenseId: "CC-BY-4.0",
      version: 2,
      author: "İçerik ekibi",
      reviewDate: "2026-10-01",
    });
    expect(label).toContain("sürüm 2");
    expect(label).toContain("CC-BY-4.0");
    expect(label).toContain("2026-10-01");
  });

  it("içerik hash'ini kısaltır", () => {
    expect(shortContentHash("a".repeat(64))).toBe(`${"a".repeat(12)}…`);
    expect(shortContentHash("kısa")).toBe("kısa");
  });

  it("kopya rozetini yalnız kopya varsa verir", () => {
    expect(
      copiedLabel({
        id: "tpl:x:v1",
        title: "X",
        practiceArea: null,
        version: 1,
        author: "a",
        reviewDate: null,
        licenseId: null,
        licenseUrl: null,
        sourceUrl: null,
        contentHash: "h",
        isCommunity: false,
        itemCount: 3,
        copiedItemCount: 3,
      }),
    ).toBe("3 öğe dosyada");
  });

  it("bulunamadı ifadesi dosyada yok demez", () => {
    expect(CHECKLIST_STATE_LABEL.missing).toBe("İncelenen kapsamda bulunamadı");
    expect(CHECKLIST_STATE_LABEL.missing.toLowerCase()).not.toContain("dosyada yok");
    expect(MISSING_SCOPE_NOTE).toContain("yalnız bakılan aralığı");
  });
});

describe("sözlük bağı (F04-T08)", () => {
  // Etiketler `lib/i18n.ts`ten gelir. Bu test, kopyanın sözlükten koptuğu
  // durumu (lib'de yeniden gömülü metin) yakalar: değerler anahtardan gelmeli.
  it("durum etiketleri ve yardım metinleri sözlükten gelir", () => {
    expect(CHECKLIST_STATE_LABEL).toEqual({
      open: t("checklists.stateLabel.open"),
      present: t("checklists.stateLabel.present"),
      missing: t("checklists.stateLabel.missing"),
      not_applicable: t("checklists.stateLabel.not_applicable"),
    });
    expect(CHECKLIST_STATE_HELP.present).toBe(t("checklists.stateHelp.present"));
    expect(MISSING_SCOPE_NOTE).toBe(t("checklists.missingScopeNote"));
  });

  it("kural iletileri sözlükten gelir ve parametresiz anahtar hata atmaz", () => {
    expect(validateChecklistEntry({ state: "present", linkedDocumentIds: [], note: "" })).toBe(
      t("checklists.rule.presentNeedsDocument"),
    );
    expect(
      validateChecklistEntry({ state: "open", linkedDocumentIds: ["doc-1"], note: "" }),
    ).toBe(t("checklists.rule.openClearsLinks"));
  });

  it("parametreli metin sayaçları gerçekten doldurur", () => {
    const summary = t("checklists.summary", {
      total: 7,
      open: 1,
      present: 2,
      missing: 3,
      notApplicable: 1,
    });
    expect(summary).toContain("7 öğe");
    expect(summary).toContain("1 açık");
    expect(summary).toContain("2 bulundu");
    expect(summary).toContain("3 incelenen kapsamda bulunamadı");
    expect(summary).toContain("1 uygulanamaz");
    expect(summary).not.toContain("{");
  });
});
