# claw10-store

Abstraksi dan implementasi penyimpanan (*storage*) untuk **Claw10 OS**.

Crate ini menyediakan antarmuka penyimpanan data persisten yang aman dan cepat untuk seluruh komponen sistem, dengan dukungan penyimpanan dalam memori (*InMemory*) maupun database lokal tertanam (*Embedded Sled Database*).

## Fitur Utama
* **Abstraksi Store**: Trait `Store` asinkron untuk operasi penyimpanan key-value.
* **Penyimpanan Sled**: Persistensi database lokal yang cepat menggunakan pustaka `sled`.
* **Penyimpanan InMemory**: Mock store yang sangat berguna untuk unit testing asinkron.
* **Namespaced Store**: Mengisolasi ruang kunci (*keyspace*) per agen, per tugas, atau per misi.

## Cara Penggunaan
```toml
[dependencies]
claw10-store = { workspace = true }
```
