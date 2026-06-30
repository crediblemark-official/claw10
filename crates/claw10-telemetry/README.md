# claw10-telemetry

Layanan telemetri, logging, dan tracing untuk **Claw10 OS**.

Crate ini bertanggung jawab untuk inisialisasi pipeline monitoring, perekaman log, monitoring aktivitas agen, dan pengiriman trace untuk debugging swarm agen.

## Fitur Utama
* **Tracing & Logging**: Menggunakan ekosistem `tracing` Rust untuk melacak alur eksekusi internal.
* **Appender Log Berkas**: Perekaman log otomatis ke dalam direktori `.log` atau sub-direktori proyek.

## Cara Penggunaan
```toml
[dependencies]
claw10-telemetry = { workspace = true }
```
