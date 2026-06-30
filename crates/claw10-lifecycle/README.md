# claw10-lifecycle

Manajemen siklus hidup (*lifecycle*) agen untuk **Claw10 OS**.

Crate ini mengelola transisi status hidup agen (seperti pembentukan, hibernasi, karantina, pencabutan hak akses, hingga terminasi aman) baik untuk tipe agen sementara (*ephemeral*) maupun persisten (*persistent*).

## Fitur Utama
* **Transisi Status**: Logika transisi status daur hidup agen yang konsisten.
* **Tindakan Ephemeral**: Pembersihan ruang kerja sementara, pembekuan metadata, dan pencabutan kredensial otomatis saat agen ephemeral menyelesaikan tugasnya.
* **Status Karantina**: Memutus hak eksekusi agen yang terdeteksi melakukan pelanggaran kebijakan atau melampaui anggaran.

## Cara Penggunaan
```toml
[dependencies]
claw10-lifecycle = { workspace = true }
```
