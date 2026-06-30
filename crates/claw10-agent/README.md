# claw10-agent

*Runtime Engine* dan orkestrasi agen utama untuk **Claw10 OS**.

Crate ini adalah otak pelaksana dari agen Claw10. Ia mengoordinasikan loop eksekusi utama agen (observasi, perencanaan tindakan, pemanggilan LLM, eksekusi perkakas, penulisan memori, evaluasi anggaran, dan pelaporan hasil kerja).

## Fitur Utama
* **Agent Loop (ReAct)**: Menggerakkan siklus pemikiran dan tindakan agen hingga tugas selesai.
* **Integrasi Subsistem**: Menghubungkan modul kebijakan (*policy*), anggaran (*budget*), memori (*memory*), dan perkakas (*tools*) dalam satu lingkungan runtime yang aman.

## Cara Penggunaan
```toml
[dependencies]
claw10-agent = { workspace = true }
```
