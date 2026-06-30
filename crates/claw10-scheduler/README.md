# claw10-scheduler

Layanan penjadwalan berkala (*background scheduler*) untuk **Claw10 OS**.

Crate ini mengelola penjadwalan tugas agen persisten secara asinkron berbasis ekspresi cron atau interval waktu tertentu (heartbeat).

## Fitur Utama
* **Cron Scheduling**: Mengevaluasi ekspresi cron standar untuk menentukan waktu eksekusi tugas berikutnya.
* **Background Worker**: Berjalan di latar belakang untuk membangunkan agen atau mengirimkan event tugas pada waktu yang tepat.

## Cara Penggunaan
```toml
[dependencies]
claw10-scheduler = { workspace = true }
```
