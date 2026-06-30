# claw10-policy

Mesin evaluasi kebijakan keamanan dan pembatasan tindakan agen untuk **Claw10 OS**.

Crate ini bertanggung jawab untuk memvalidasi apakah tindakan (action) yang diajukan oleh agen (seperti pemanggilan shell command atau HTTP request ke luar) diperbolehkan berdasarkan aturan kebijakan (*policies*) yang terpasang di sistem.

## Fitur Utama
* **Pengecekan Tindakan**: Memvalidasi tindakan terhadap aturan kebijakan yang didasarkan pada prioritas dan pencocokan pola (*wildcard/suffix/prefix*).
* **Bundle Policy**: Mengelola sekelompok kebijakan aktif yang menentukan perilaku keamanan seluruh swarm.

## Cara Penggunaan
```toml
[dependencies]
claw10-policy = { workspace = true }
```
