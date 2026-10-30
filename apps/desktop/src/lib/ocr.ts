/**
 * OCR dili — ürün varsayılanı ve etiketleri **tek yerde**.
 *
 * Tek kaynak Rust tarafıdır: `dim_storage::queue::DEFAULT_OCR_LANG`. Buradaki
 * değer onun aynasıdır; `ocr.test.ts` iki değeri karşılaştırır (biri değişip
 * diğeri kalırsa test kırmızı olur). Aynı ayna worker'da da var
 * (`dim_worker.engines.DEFAULT_OCR_LANG`, `tests/test_ocr_lang.py` denetler).
 *
 * Neden `tur+eng`: ölçüm (F15, `scripts/ocr_lang_bench.py`) `eng` modelinin
 * Türkçe diyakritiklerin **0/79**'ını koruduğunu gösterdi (karakter doğruluğu
 * %82,8); `tur` ve `tur+eng` **79/79** (%100). `tur+eng` karışık belgeler için
 * seçildi; bedeli ölçüldü: 2 sayfada +183 ms (~%8).
 */
import { t } from "./i18n";

export const DEFAULT_OCR_LANG = "tur+eng";

/** Tesseract dil kodunun insan okunur adı; tanınmayan kod **olduğu gibi** yazılır. */
export function ocrLanguageLabel(lang: string): string {
  const codes = lang
    .split("+")
    .map((part) => part.trim().toLowerCase())
    .filter((part) => part !== "");
  if (codes.length === 0) return lang;
  const names = codes.map((code) => {
    switch (code) {
      case "tur":
        return t("ocrLang.tur");
      case "eng":
        return t("ocrLang.eng");
      case "deu":
        return t("ocrLang.deu");
      case "fra":
        return t("ocrLang.fra");
      default:
        return code;
    }
  });
  return names.join(" + ");
}

/** OCR düğmesi/ipucu için kısa etiket: "Türkçe + İngilizce". */
export function defaultOcrLanguageLabel(): string {
  return ocrLanguageLabel(DEFAULT_OCR_LANG);
}
