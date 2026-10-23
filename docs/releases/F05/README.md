# F05 — İçe aktarma ve belge sürümleri

- **Sürüm etiketi:** `v0.4.0`
- **Durum:** Devam ediyor (aşağıdaki açık işler)
- **Tarih:** 2026-10-23

| İş | Durum | Kanıt |
|---|---|---|
| F05-T01 | ✅ | dosya seçimi (dialog izni), türü içerikten algılama |
| F05-T02 | ✅ | SHA-256, özgün bayt değişmezliği, şifreli blob |
| F05-T03 | ✅ | `(document_id, version_no)` benzersiz, sürüm soy ağacı |
| F05-T04 | ✅ | aynı içerik aynı dosyada → yeni kayıt yok, mevcut gösterilir |
| F05-T05 | ✅ | 250 MiB sınırı, desteklenmeyen tür reddi |

## Doğrulama

`cargo test -p dim-ingest` — 4 test: magic-byte, şifreli blob (düz metin yok),
tekrar tespiti, desteklenmeyen dosya. CI: **bekliyor**.

## Bilinen sınırlar

- Parola korumalı PDF algılanır ama parola girişi F06'da.
- `original_filename_encrypted` şu an boş; görünen ad `documents.display_name`.
