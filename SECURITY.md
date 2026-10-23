# Güvenlik politikası

## Bildirim

Güvenlik açığını **kamuya açık issue açmadan** bildirin:
GitHub → Security → *Report a vulnerability* (özel danışma), veya depo sahibine doğrudan mesaj.

İlk yanıt hedefi: 72 saat. Düzeltme süresi, etkiye göre 7–30 gün; kritik veri
bütünlüğü/şifreleme açıklarında sürüm yayını bloke edilir.

## Kapsam

- Kasa şifreleme, anahtar türetme, kurtarma yolu
- Blob bütünlüğü, kesilme/truncation tespiti
- Dosya kapsamı sızıntısı (bir dosyanın verisinin başka dosyaya görünmesi)
- Aktarım izni olmadan dışarı veri gönderimi
- CLI adaptörlerinde yetki genişlemesi, kimlik kopyalama
- Maskeleme çıktısında içerik sızması

## Kapsam dışı

- Üçüncü taraf CLI'lerin kendi hataları (sağlayıcıya bildirilir; adaptör davranışı bizde)
- Kullanıcının kendi parolasını kaybetmesi (kurtarma anahtarı akışı belgelenmiştir)
- Desteklenmeyen platformlar (Linux v1 kapsamı dışı)

## Bilinen sınırlar

Bu depo pre-alpha'dır; güvenlik iddiaları `docs/releases/` altındaki faz kanıtlarıyla
sınırlıdır. "Şifreli" ifadesi, o fazın testiyle doğrulanmamışsa pazarlama iddiası sayılmaz.
