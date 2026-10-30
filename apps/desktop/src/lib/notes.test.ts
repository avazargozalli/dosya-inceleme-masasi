import { describe, expect, it } from "vitest";
import { t } from "./i18n";
import {
  CONFLICT_HINT,
  MAX_DIFF_LINES,
  diffNoteText,
  diffSummary,
  draftIsStale,
  draftStatusLabel,
  noteEditDirty,
  revisionOriginLabel,
} from "./notes";

describe("taslak durumu", () => {
  it("durum etiketlerini sözlükten verir", () => {
    expect(draftStatusLabel("idle")).toBe(t("notes.draftIdle"));
    expect(draftStatusLabel("saving")).toBe(t("notes.draftSaving"));
    expect(draftStatusLabel("saved")).toBe(t("notes.draftSaved"));
    expect(draftStatusLabel("failed")).toBe(t("notes.draftFailed"));
    // Metin sözlükten gelse de anlamı korunmalı: başarısız durum "kaydedildi" demez.
    expect(t("notes.draftFailed")).not.toMatch(/kaydedildi$/);
  });

  it("taslağın dayandığı sürüm ilerlediyse bayat sayar", () => {
    expect(draftIsStale(3, 3)).toBe(false);
    expect(draftIsStale(3, 4)).toBe(true);
    expect(draftIsStale(null, 9)).toBe(false);
  });
});

describe("sürüm kaynağı etiketi", () => {
  it("bilinen kaynakları sözlükten çevirir", () => {
    expect(revisionOriginLabel("create")).toBe(t("notes.originCreate"));
    expect(revisionOriginLabel("edit")).toBe(t("notes.originEdit"));
    expect(revisionOriginLabel("undo")).toBe(t("notes.originUndo"));
    expect(revisionOriginLabel("redo")).toBe(t("notes.originRedo"));
  });

  it("bilinmeyen kaynağı uydurmaz", () => {
    expect(revisionOriginLabel("merge")).toBe(t("notes.originUnknown"));
  });
});

describe("not farkı", () => {
  it("aynı metinde tüm satırlar aynıdır", () => {
    const diff = diffNoteText("bir\niki", "bir\niki");
    expect(diff.map((line) => line.kind)).toEqual(["same", "same"]);
    expect(diffSummary(diff)).toEqual({ mineOnly: 0, theirsOnly: 0, same: 2, hasChanges: false });
  });

  it("bende olan satırı 'mine', güncelde olanı 'theirs' işaretler", () => {
    const diff = diffNoteText("bir\niki\nüç", "bir\niki\nüç nokta");
    expect(diff).toEqual([
      { kind: "same", text: "bir" },
      { kind: "same", text: "iki" },
      { kind: "mine", text: "üç" },
      { kind: "theirs", text: "üç nokta" },
    ]);
    const summary = diffSummary(diff);
    expect(summary.mineOnly).toBe(1);
    expect(summary.theirsOnly).toBe(1);
    expect(summary.hasChanges).toBe(true);
  });

  it("yalnız bende olan satırı silinmiş olarak gösterir", () => {
    const diff = diffNoteText("bir\niki\nüç", "bir\nüç");
    expect(diff.filter((line) => line.kind === "mine").map((line) => line.text)).toEqual(["iki"]);
    expect(diff.filter((line) => line.kind === "theirs")).toHaveLength(0);
  });

  it("yalnız güncelde olan satırı eklenmiş olarak gösterir", () => {
    const diff = diffNoteText("bir", "bir\niki");
    expect(diff).toEqual([
      { kind: "same", text: "bir" },
      { kind: "theirs", text: "iki" },
    ]);
  });

  it("Türkçe karakterleri bozmadan karşılaştırır", () => {
    const diff = diffNoteText("İtirazın reddine", "Itirazin reddine");
    expect(diffSummary(diff).mineOnly).toBe(1);
    expect(diffSummary(diff).theirsOnly).toBe(1);
  });

  it("boş metni tek satır sayar ve farkı gizlemez", () => {
    const diff = diffNoteText("", "yeni metin");
    expect(diff).toEqual([
      { kind: "mine", text: "" },
      { kind: "theirs", text: "yeni metin" },
    ]);
  });

  it("çok uzun metinde kaba ama dürüst karşılaştırma yapar", () => {
    const mine = Array.from({ length: MAX_DIFF_LINES + 1 }, (_, index) => `ben-${index}`).join("\n");
    const theirs = Array.from({ length: MAX_DIFF_LINES + 1 }, (_, index) => `kasa-${index}`).join("\n");
    const diff = diffNoteText(mine, theirs);
    expect(diff).toEqual([
      { kind: "mine", text: mine },
      { kind: "theirs", text: theirs },
    ]);
  });

  it("fark yoksa çakışma uyarısı metni yanıltmaz", () => {
    expect(CONFLICT_HINT).toBe(t("notes.conflictHint"));
    expect(CONFLICT_HINT).toMatch(/sessizce yazılmaz/);
    expect(diffSummary(diffNoteText("aynı", "aynı")).hasChanges).toBe(false);
  });
});

describe("kaydedilmemiş değişiklik", () => {
  const saved = { title: "Başlık", body: "Metin", sourceAnchorIds: ["a"] };

  it("başlık boşlukları kırpılarak karşılaştırılır", () => {
    expect(noteEditDirty({ ...saved, title: "  Başlık  " }, saved)).toBe(false);
  });

  it("gövde farkını yakalar", () => {
    expect(noteEditDirty({ ...saved, body: "Metin " }, saved)).toBe(true);
  });

  it("kaynak sırası ve sayısı değişince kirlidir", () => {
    expect(noteEditDirty({ ...saved, sourceAnchorIds: ["a", "b"] }, saved)).toBe(true);
    expect(noteEditDirty({ ...saved, sourceAnchorIds: ["b"] }, saved)).toBe(true);
    expect(noteEditDirty({ ...saved, sourceAnchorIds: ["a"] }, saved)).toBe(false);
  });
});
