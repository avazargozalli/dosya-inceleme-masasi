/**
 * Not düzenleme yardımcıları (F07-T10). Saf fonksiyonlar: arayüz bunları
 * kullanır, böylece taslak durumu, çakışma farkı ve sürüm etiketleri
 * test edilebilir kalır. Metin karşılaştırması sunucudan gelen gerçek
 * içerikle yapılır; çakışma ekranı uydurma metin göstermez.
 */

/** Metinler sözlükten (`lib/i18n.ts`) — F04-T08. */
import { t } from "./i18n";

export type DraftStatus = "idle" | "saving" | "saved" | "failed";

export const DRAFT_STATUS_LABEL: Record<DraftStatus, string> = {
  idle: t("notes.draftIdle"),
  saving: t("notes.draftSaving"),
  saved: t("notes.draftSaved"),
  failed: t("notes.draftFailed"),
};

export function draftStatusLabel(status: DraftStatus): string {
  return DRAFT_STATUS_LABEL[status] ?? DRAFT_STATUS_LABEL.idle;
}

export const REVISION_ORIGIN_LABEL: Record<string, string> = {
  create: t("notes.originCreate"),
  edit: t("notes.originEdit"),
  undo: t("notes.originUndo"),
  redo: t("notes.originRedo"),
};

export function revisionOriginLabel(origin: string): string {
  return REVISION_ORIGIN_LABEL[origin] ?? t("notes.originUnknown");
}

export type NoteDiffLine = { kind: "same" | "mine" | "theirs"; text: string };

/** Farkı hesaplarken kabul edilen en çok satır; üstünde kaba karşılaştırma. */
export const MAX_DIFF_LINES = 400;

function splitLines(text: string): string[] {
  // Sondaki boş satır kaybolmasın: boş metin tek satır sayılır.
  if (text.length === 0) return [""];
  return text.split("\n");
}

/**
 * Satır bazlı LCS farkı: `mine` (benim kaydetmek istediğim metin) ile
 * `theirs` (kasadaki güncel metin) karşılaştırılır. Aynı satır `same`,
 * yalnız bende olan `mine`, yalnız güncelde olan `theirs` olarak döner.
 * Sıra korunur; `mine` satırları kendi aralarında, `theirs` satırları kendi
 * aralarında orijinal sırasını izler.
 */
export function diffNoteText(mine: string, theirs: string): NoteDiffLine[] {
  const a = splitLines(mine);
  const b = splitLines(theirs);
  if (a.length > MAX_DIFF_LINES || b.length > MAX_DIFF_LINES) {
    // Çok büyük metinde satır hizalaması yerine kaba ama dürüst karşılaştırma.
    const out: NoteDiffLine[] = [];
    if (mine !== theirs) {
      out.push({ kind: "mine", text: mine });
      out.push({ kind: "theirs", text: theirs });
    } else {
      out.push({ kind: "same", text: mine });
    }
    return out;
  }
  const rows = a.length;
  const cols = b.length;
  const table: number[][] = [];
  for (let i = 0; i <= rows; i += 1) table.push(new Array<number>(cols + 1).fill(0));
  for (let i = rows - 1; i >= 0; i -= 1) {
    const row = table[i];
    const next = table[i + 1];
    if (!row || !next) continue;
    for (let j = cols - 1; j >= 0; j -= 1) {
      const diagonal = next[j + 1] ?? 0;
      const down = next[j] ?? 0;
      const right = row[j + 1] ?? 0;
      row[j] = a[i] === b[j] ? diagonal + 1 : Math.max(down, right);
    }
  }
  const out: NoteDiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < rows && j < cols) {
    const mineLine = a[i] ?? "";
    const theirLine = b[j] ?? "";
    if (mineLine === theirLine) {
      out.push({ kind: "same", text: mineLine });
      i += 1;
      j += 1;
      continue;
    }
    const down = table[i + 1]?.[j] ?? 0;
    const right = table[i]?.[j + 1] ?? 0;
    if (down >= right) {
      out.push({ kind: "mine", text: mineLine });
      i += 1;
    } else {
      out.push({ kind: "theirs", text: theirLine });
      j += 1;
    }
  }
  while (i < rows) {
    out.push({ kind: "mine", text: a[i] ?? "" });
    i += 1;
  }
  while (j < cols) {
    out.push({ kind: "theirs", text: b[j] ?? "" });
    j += 1;
  }
  return out;
}

export type DiffSummary = { mineOnly: number; theirsOnly: number; same: number; hasChanges: boolean };

export function diffSummary(diff: NoteDiffLine[]): DiffSummary {
  let mineOnly = 0;
  let theirsOnly = 0;
  let same = 0;
  for (const line of diff) {
    if (line.kind === "mine") mineOnly += 1;
    else if (line.kind === "theirs") theirsOnly += 1;
    else same += 1;
  }
  return { mineOnly, theirsOnly, same, hasChanges: mineOnly > 0 || theirsOnly > 0 };
}

/**
 * Kasada saklanan taslak, notun o sırada okunan sürümüne mi ait? Sürüm
 * ilerlediyse taslak eski görünüme dayanır; kaydetme çakışma ekranına düşer.
 */
export function draftIsStale(draftBaseRowVersion: number | null, currentRowVersion: number): boolean {
  if (draftBaseRowVersion === null) return false;
  return draftBaseRowVersion !== currentRowVersion;
}

export const CONFLICT_HINT = t("notes.conflictHint");

export function noteEditDirty(
  current: { title: string; body: string; sourceAnchorIds: string[] },
  saved: { title: string; body: string; sourceAnchorIds: string[] },
): boolean {
  if (current.title.trim() !== saved.title.trim()) return true;
  if (current.body !== saved.body) return true;
  if (current.sourceAnchorIds.length !== saved.sourceAnchorIds.length) return true;
  return current.sourceAnchorIds.some((id, index) => id !== saved.sourceAnchorIds[index]);
}
