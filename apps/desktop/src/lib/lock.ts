// Kilit kuralı: kasa kilitliyken yalnız `vault` ekranı gösterilebilir.
// Saf fonksiyon — arayüz render edilmeden test edilir.

export function canShowScreen(screen: string, locked: boolean): boolean {
  return !locked || screen === "vault";
}
