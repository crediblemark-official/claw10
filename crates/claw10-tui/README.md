# claw10-tui

Antarmuka Terminal Pengguna (*Terminal User Interface*) untuk **Claw10 OS**.

Crate ini menyediakan UI berbasis terminal interaktif yang dibangun di atas pustaka `ratatui` dan `crossterm`. Ini memungkinkan pengguna mengobrol dengan agen utama, melihat log aktivitas secara real-time, menyetujui eksekusi perkakas sensitif, dan memantau status swarm.

## Fitur Utama
* **Tampilan Chat Interaktif**: Kolom obrolan dinamis dengan agen utama.
* **Command-based Tool Approval**: Masukan berbasis perintah (seperti `:approve` atau `:reject`) untuk konfirmasi eksekusi perintah sensitif.
* **Sidebar Swarm Monitoring**: Memantau daftar agen anak dan tugas yang sedang berjalan secara berdampingan.

## Cara Penggunaan
```toml
[dependencies]
claw10-tui = { workspace = true }
```
