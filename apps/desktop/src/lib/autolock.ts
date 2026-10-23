// Otomatik kilit: kasa açıkken hareketsizlik süresi dolunca kapanır. Saf
// fonksiyon; saat/test edilebilirlik için `now` dışarıdan verilir.

export const AUTO_LOCK_MINUTES = 10;
export const AUTO_LOCK_CHECK_MS = 30_000;

export function shouldAutoLock(
  lastActivityMs: number,
  nowMs: number,
  limitMs: number = AUTO_LOCK_MINUTES * 60_000,
): boolean {
  if (!Number.isFinite(lastActivityMs) || !Number.isFinite(nowMs)) return false;
  if (limitMs <= 0) return false;
  // Saat geri alınmışsa (uyku/manuel değişiklik) kilitleme; negatif fark
  // yüzünden kasa açık kalmasın diye sıfır kabul edilir.
  const elapsed = nowMs - lastActivityMs;
  if (elapsed < 0) return true;
  return elapsed >= limitMs;
}
