# claw10-prompt

Kompiler perakitan berkas template prompt dinamis untuk agen **Claw10 OS**.

Crate ini menangani penyusunan instruksi dasar (*system prompts*), aturan peran agen, siklus hidup agen, dan integrasi batasan keamanan (injection prevention) menjadi prompt final yang dikirimkan ke LLM.

## Fitur Utama
* **Assembler Prompt**: Menggabungkan berbagai berkas template prompt `.icvs` atau berkas mentah menjadi satu kesatuan prompt.
* **Manajemen Peran (Roles)**: Menyuntikkan deskripsi peran agen secara dinamis berdasarkan definisi tugas.

## Cara Penggunaan
```toml
[dependencies]
claw10-prompt = { workspace = true }
```
