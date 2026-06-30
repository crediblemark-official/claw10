# claw10-toon

Utilitas enkoding **Token-Oriented Object Notation (TOON)** untuk **Claw10 OS**.

Crate ini menyediakan implementasi encoder untuk mengonversi data internal Claw10 OS menjadi format TOON. TOON adalah alternatif JSON yang dirancang khusus untuk meminimalkan penggunaan token pada prompt LLM (*Large Language Model*) dengan tetap menjaga keterbacaan manusia secara maksimal.

Situs web resmi: [https://toonformat.dev/](https://toonformat.dev/)

## Keunggulan TOON
* **Efisiensi Token**: Mengurangi konsumsi token hingga ~40% dibandingkan format JSON standar dalam skenario Mixed-Structure Benchmarks.
* **Akurasi Tinggi**: Mempertahankan akurasi pemahaman model (bahkan mencapai 76.4% dibanding JSON yang berkisar 75.0% pada evaluasi benchmark).
* **LLM-Friendly Guardrails**: Memberikan batasan panjang array `[N]` dan header kolom `{fields}` yang eksplisit agar model AI dapat mem-parse data dengan andal.
* **Minimal Syntax**: Menggantikan tanda kurung kurawal `{}` dengan indentasi dan meminimalkan tanda kutip dua (`"`) sehingga lebih ringkas layaknya perpaduan YAML dan CSV.
* **Tabular Arrays**: Mampu meringkas kumpulan objek sejenis menjadi bentuk tabel satu baris untuk efisiensi token maksimal.

## Fitur di Claw10 OS
* **Context Encoding**: Mengonversi `Task`, `Mission`, `Memory`, `Policy`, `Lineage`, `Evidence`, `Skill`, dan `Worker` ke format TOON.
* **Tabular Roster**: Mengubah daftar representasi agen dan status worker menjadi bentuk baris tabel ringkas.

## Cara Penggunaan
Tambahkan dependensi ke `Cargo.toml`:
```toml
[dependencies]
claw10-toon = { workspace = true }
```
