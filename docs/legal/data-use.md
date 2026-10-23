# Kurgu veri ve fixture rehberi

## Kural

Depoya giren her test belgesi **sentetiktir**. Sentetik veri, gerçek bir kişi/kuruma
işaret etmeyen, üretildiği anda kurgu olduğu bilinen veridir.

## Üretim

`fixtures/synthetic/generate.py` betiği PDF/DOCX/TXT/PNG üretir:

- İsimler: `Ali Yılmaz`, `Ayşe Demir` gibi yaygın kurgu adlar; TCKN üretilmez, yerine
  `00000000000` benzeri açıkça geçersiz diziler kullanılır.
- Esas numarası: `2026/0000` gibi sıra dışı, gerçek olmayan biçim.
- Adres: `Örnek Mah. Test Cad. No:0`.
- Tarihler: sabit tohum (seed) ile üretilir; testler deterministiktir.

## Yasak

- Gerçek müvekkil dosyası, gerçek yazışma, gerçek mahkeme kararı metni (telif + kişisel veri).
- Ekran görüntüsünde gerçek isim/numara.
- `fixtures/real/` klasörü `.gitignore`'dadır; oraya dosya konmaz.

## İzin beyanı

Pilot katılımcısı, anonimleştirdiği belgeyi yalnız kendi cihazında işler; depoya veya
merkezi bir ölçüm sistemine yüklemez. Bu beyan pilot formunda imzalanır.
