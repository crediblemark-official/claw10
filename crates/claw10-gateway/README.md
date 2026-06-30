# claw10-gateway

Gateway saluran pesan (*message channel gateway*) untuk **Claw10 OS**.

Crate ini bertindak sebagai jembatan komunikasi antara swarm agen internal Claw10 OS dengan saluran komunikasi eksternal seperti webhook, API gateway, dan platform perpesanan.

## Fitur Utama
* **Webhook Receiver**: Menangani permintaan pesan masuk (*IncomingMessage*) secara aman.
* **Integrasi E2E Gateway**: Memfasilitasi pengujian integrasi alur gateway dari webhook hingga ke agen.

## Cara Penggunaan
```toml
[dependencies]
claw10-gateway = { workspace = true }
```
