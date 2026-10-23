// Kasa girişi doğrulama kuralları. Arayüzde ve testte aynı fonksiyon kullanılır;
// böylece "buton açık ama istek reddedildi" durumu oluşmaz.

export const MIN_PASSWORD_LENGTH = 12;

const COMMON_PASSWORDS = new Set([
  "password1234",
  "123456789012",
  "qwertyuiop12",
  "avukat123456",
  "parola123456",
]);

export type PasswordCheck = { ok: true } | { ok: false; reason: string };

export function checkPassword(password: string): PasswordCheck {
  if (password.length < MIN_PASSWORD_LENGTH) {
    return { ok: false, reason: `Parola en az ${MIN_PASSWORD_LENGTH} karakter olmalı.` };
  }
  if (COMMON_PASSWORDS.has(password.toLowerCase())) {
    return { ok: false, reason: "Bu parola çok yaygın; farklı bir parola seçin." };
  }
  if (/^(.)\1+$/.test(password)) {
    return { ok: false, reason: "Tek karakterin tekrarı parola olarak kullanılamaz." };
  }
  if (/^\d+$/.test(password)) {
    return { ok: false, reason: "Yalnız rakamlardan oluşan parola kullanılamaz." };
  }
  return { ok: true };
}

export function checkPasswordConfirmation(password: string, confirmation: string): PasswordCheck {
  if (password !== confirmation) return { ok: false, reason: "Parolalar eşleşmiyor." };
  return checkPassword(password);
}

/** Kurtarma anahtarı gösterimi: 8 grup × 4 karakter, tireli. */
export function normalizeRecoveryInput(input: string): string {
  return input.replace(/[\s-]/g, "").toUpperCase();
}
