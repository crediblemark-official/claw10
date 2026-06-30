# claw10-task

Manajemen tugas (*task lifecycle*) untuk **Claw10 OS**.

Crate ini mengelola pembuatan, transisi status status transisi, pembagian tugas kepada agen pelaksana (*claimed/assigned*), dan verifikasi penyelesaian tugas berdasarkan kriteria deterministik.

## Fitur Utama
* **Task State Transitions**: Mengontrol siklus hidup tugas (Created, Ready, Claimed, Running, Verifying, Accepted, Closed, Failed).
* **Penetapan Tugas**: Mengaitkan tugas dengan agen pelaksana spesifik yang memegang izin yang sesuai.

## Cara Penggunaan
```toml
[dependencies]
claw10-task = { workspace = true }
```
