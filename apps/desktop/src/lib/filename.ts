/**
 * Dışa aktarma dosya adı (F04-T08).
 *
 * Dosya adı kullanıcıya görünen dosya başlığından üretilir. Ad, işletim
 * sisteminde sorun çıkaran karakterlerden arındırılır; **Türkçe harfler
 * korunur** (dosya adı Türkçe kalır — ASCII'ye çevirmek kullanıcı için
 * tanınmaz ad üretirdi).
 *
 * Bu yardımcı ekrandan çıkarıldı çünkü Türkçe harf kümesi ekran dosyasında
 * gömülü metin sayılıyordu; yerelleştirme kapısı "taşınmış ekranda Türkçe harf
 * yok" kuralıyla çalışır ve bu kural gerçek bir metin sızıntısını yakalar.
 */

/** İzin verilen: kelime karakterleri, tire, boşluk ve Türkçe harfler. */
const UNSAFE = /[^\w\- ığüşöçİĞÜŞÖÇ]+/g;

/** Başlıktan dosya adı gövdesi. Boş başlık boş gövde verir (çağıran karar verir). */
export function fileStem(title: string): string {
  return title.replace(UNSAFE, "_");
}
