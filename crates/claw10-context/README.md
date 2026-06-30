# claw10-context

Pipeline pengelola konteks obrolan dan riwayat pesan agen untuk **Claw10 OS**.

Crate ini menangani penyusunan, pembatasan ukuran token (*sliding window*), pengayaan konteks menggunakan memori semantik, dan penyediaan konteks yang siap dikirimkan ke model LLM.

## Fitur Utama
* **Context Assembly**: Menyusun konteks lengkap yang menggabungkan memori, detail tugas, dan metadata lingkungan.
* **Token Pruning**: Mengurangi dan memotong riwayat interaksi secara cerdas agar pas dengan batas token konteks LLM.

## Cara Penggunaan
```toml
[dependencies]
claw10-context = { workspace = true }
```
