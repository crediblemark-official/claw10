# claw10-icvs

Wrapper kompiler ICVS (Internal Agent Policy Schema) untuk **Claw10 OS**.

Crate ini menyediakan antarmuka pengompilasian skema berkas aturan kebijakan internal agen agar kompatibel dengan modul evaluasi kebijakan di runtime.

## Fitur Utama
* **Parsing ICVS**: Membaca berkas skema kebijakan ICVS.
* **Kompilasi Skema**: Mengubah sintaks kebijakan menjadi struktur data evaluasi kebijakan runtime yang dapat diproses dengan cepat.

## Cara Penggunaan
```toml
[dependencies]
claw10-icvs = { workspace = true }
```
