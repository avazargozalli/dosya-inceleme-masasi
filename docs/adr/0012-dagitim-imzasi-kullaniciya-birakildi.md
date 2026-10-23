# ADR-0012 — Dağıtım imzası kullanıcıya bırakıldı

- **Durum:** Kabul
- **Tarih:** 2026-10-23
- **Bağlam:** ADR-0008 UYAP/e-imza otomasyonunu kapsam dışı bırakıyor. F16 ise
  "imzalı genel sürüm"ü faz olarak tanımlıyordu; bu, Apple Developer ID ve
  Windows kod imza sertifikası gerektiriyordu.

## Karar

Ürün **imzasız** dağıtılır. Notarization ve kod imzası ürün kapsamı dışıdır.

- Depo AGPL-3.0 ve kaynak açıktır; isteyen kendi sertifikasıyla imzalar.
- `desktop-build` imzasız `.app` ve Windows ikilisi üretir; sürüm notunda
  "imzasız" açıkça yazar.
- Güncelleme (updater) **varsayılan kapalıdır**; `pubkey` boştur. Kendi
  dağıtımını imzalamak isteyen, `tauri.conf.json` içine kendi açık anahtarını
  ve uç noktasını koyar. Anahtar yokken imza doğrulaması devreye girmez ve
  güncelleme akışı çalışmaz.
- Kullanıcıya Gatekeeper/Defender uyarısı çıkacağı **dürüstçe** bildirilir;
  "sessiz kurulum" vaadi verilmez.

## Sonuçlar

- F16'nın sertifika gerektiren maddeleri (T04, T05, T06) **kapsam dışı**.
- F16'da kalan kod işi: SBOM (yapıldı), atomik geri dönüş tatbikatı, updater
  imza doğrulama mekanizmasının test edilebilir iskeleti.
- Hiçbir yerde "imzalı sürüm" denmez; paket adları ve sürüm notları imzasız
  olduğunu söyler.
