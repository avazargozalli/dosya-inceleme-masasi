# Katkı rehberi

## Temel kurallar

1. **Gerçek veri yok.** Müvekkil belgesi, kimlik numarası, gerçek esas numarası, gerçek
   yazışma içeriği eklenemez. Testler `fixtures/synthetic/` altındaki kurgu dosyalarla yapılır.
2. **Güvenlik kapısı gevşetilmez.** Test geçsin diye kaynak doğrulaması, kapsam denetimi,
   şifreleme veya yetki daraltması kaldırılamaz.
3. **Tek faz kuralı.** Bir PR tek bir Fxx işini (ve zorunlu önkoşulunu) hedefler. İlgisiz
   refactor aynı PR'a karıştırılmaz.
4. **Kanıt zorunlu.** Her PR "Nasıl doğrulandı?" bölümünde çalıştırılan komutu ve sonucu yazar.
   "Yapıldı" demek kanıt değildir.

## Geliştirme akışı

```bash
pnpm install --frozen-lockfile
pnpm lint && pnpm typecheck && pnpm test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m pytest workers/document/tests -q
```

Commit mesajı: `F03-T04: SQLCipher şeması ve migrasyon checksum'ları`
(bkz. `docs/roadmap.md` iş kimlikleri).

## Dal ve PR

- Dal: `f03-t04-sqlcipher-schema`
- PR açıklaması: amaç, kapsam, kabul kanıtı, açık sınırlamalar, veri/güvenlik etkisi.
- En az bir inceleyici; güvenlikle ilgili dosyalarda (`crates/vault`, `crates/policy`,
  `workers/document/src/redact*`) iki inceleyici.

## Kodlama ajanları

Ajanlar `AGENTS.md` ve `docs/spec/` içindeki şartnameyi bağlayıcı kabul eder. Bir fazın
kabul kanıtını üretmeden sonraki fazı "tamamlandı" işaretlemek yasaktır.
