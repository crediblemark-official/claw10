# claw10-memory

Layanan memori jangka panjang (*long-term memory*) agen untuk **Claw10 OS**.

Crate ini menyediakan kemampuan bagi agen untuk menyimpan, mengindeks, dan memanggil ingatan/fakta (memori episodik dan memori semantik) yang didapatkan selama pengerjaan misi.

## Fitur Utama
* **Memori Semantik**: Penyimpanan informasi berbasis teks atau tag pencarian yang relevan.
* **Konsolidasi Memori**: Menggabungkan dan merangkum informasi lama untuk mencegah kapasitas prompt berlebih.
* **Integrasi Store Persisten**: Menyimpan memori secara aman di database lokal melalui abstraksi store.

## Cara Penggunaan
```toml
[dependencies]
claw10-memory = { workspace = true }
```
