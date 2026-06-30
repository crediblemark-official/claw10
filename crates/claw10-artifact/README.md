# claw10-artifact

Layanan manajemen penyimpanan hasil kerja (*artifacts*) untuk **Claw10 OS**.

Crate ini bertanggung jawab untuk menyimpan, mengindeks, memverifikasi integritas (*checksum*), dan mengunduh berkas hasil kerja (seperti laporan markdown, kode script, data gambar, dll) yang diproduksi oleh agen selama menyelesaikan tugasnya.

## Fitur Utama
* **Penyimpanan Berkas**: Mengaitkan berkas dengan `TaskId` dan `AgentId` pembuatnya.
* **Verifikasi Integritas**: Mendeteksi modifikasi atau kerusakan berkas secara tidak sah menggunakan kalkulasi hash.
* **Metrik & Penghitung**: Pelacakan statistik ukuran berkas dan jumlah total hasil kerja yang tersimpan.

## Cara Penggunaan
```toml
[dependencies]
claw10-artifact = { workspace = true }
```
