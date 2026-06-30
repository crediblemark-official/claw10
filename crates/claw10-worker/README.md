# claw10-worker

Registri dan manajemen worker lokal untuk **Claw10 OS**.

Crate ini melacak ketersediaan, kesehatan (heartbeat), dan status operasional dari node worker lokal yang bertugas mengeksekusi kode terisolasi atas instruksi agen.

## Fitur Utama
* **Registrasi Worker**: Mendaftarkan node worker baru dengan daftar kemampuan perkakas (*tool capabilities*) yang dimiliki.
* **Heartbeat & Liveness**: Deteksi kesehatan worker secara dinamis untuk mengarantina worker yang tidak responsif (*stale workers*).

## Cara Penggunaan
```toml
[dependencies]
claw10-worker = { workspace = true }
```
