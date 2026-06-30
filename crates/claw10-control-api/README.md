# claw10-control-api

API Kontrol HTTP luar (*HTTP Control API*) untuk **Claw10 OS**.

Crate ini menyediakan web server internal berbasis `axum` untuk menerima permintaan kontrol, monitoring, dan interaksi dengan swarm agen dari sistem luar atau web UI.

## Fitur Utama
* **Endpoint HTTP**: API RESTful untuk memantau status misi, log tugas, dan ketersediaan agen.
* **Integrasi NATS (Opsional)**: Mendukung koordinasi terdistribusi melalui messaging NATS.

## Cara Penggunaan
```toml
[dependencies]
claw10-control-api = { workspace = true }
```
