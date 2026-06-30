# claw10-event

Abstraksi dan implementasi bus acara (*event bus*) untuk **Claw10 OS**.

Crate ini menyediakan sistem pengiriman pesan (*pub/sub*) untuk memicu aksi, koordinasi internal, dan pertukaran informasi antar-agen dalam swarm secara asinkron.

## Fitur Utama
* **Event Broker**: Antarmuka bus acara asinkron.
* **Driver InMemory**: Implementasi bus acara lokal di memori untuk pengujian cepat.
* **Driver NATS (Opsional)**: Integrasi dengan sistem perpesanan terdistribusi NATS untuk skalabilitas tingkat tinggi.

## Cara Penggunaan
```toml
[dependencies]
claw10-event = { workspace = true }
```
