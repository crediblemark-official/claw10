# claw10-toon

Utilitas enkoding **Token-Oriented Object Notation (TOON)** untuk **Claw10 OS**.

Crate ini menyediakan fungsi pembantu (helper) untuk mengompresi struktur data kompleks menjadi format ringkas guna meminimalkan penggunaan token saat dikirim ke LLM (*Large Language Model*).

## Fitur Utama
* **Enkoding Ringkas**: Mengonversi memori, riwayat chat, deskripsi tugas, dan informasi perkakas (tools) menjadi representasi string yang optimal untuk model AI.
* **Penghematan Token**: Mengurangi overhead overhead token prompt secara signifikan.

## Cara Penggunaan
```toml
[dependencies]
claw10-toon = { workspace = true }
```
