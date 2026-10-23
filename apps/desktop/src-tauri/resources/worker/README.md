# Paketlenmiş worker dizini

Bu dizin boşken de depoda **izlenir**: `tauri.conf.json` içindeki
`bundle.resources` glob'u (`resources/worker/**/*`) hedef bulamazsa Tauri
build betiği hata verir (temiz checkout'ta `cargo check` kırılır).

Gerçek worker çıktısı `scripts/build-worker.sh` ile buraya kopyalanır ve
git'e girmez (bkz. `.gitignore`). Paketleme yapılmamış checkout'ta yalnız
bu dosya ve `.gitkeep` bulunur; uygulama worker'ı geliştirme yolundan
(`dev_worker_dir`) arar.

Elle paketleme:

```sh
scripts/build-worker.sh
```
