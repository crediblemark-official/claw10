# claw10 (CLI Utama)

Paket biner dan antarmuka baris perintah (*Command Line Interface*) utama untuk **Claw10 OS**.

Paket ini menyatukan seluruh pustaka sub-crate di bawah naungan Claw10 OS menjadi aplikasi CLI mandiri. Ini digunakan oleh pengguna untuk menginisiasi swarm agen, menjalankan setup wizard, mengatur telemetri, dan memicu server TUI.

## Fitur Utama
* **Setup Wizard**: Membantu pengguna mengonfigurasi API Key LLM dan pengaturan lingkungan awal.
* **TUI Server Serve**: Menyediakan mode bawaan (*default serve mode*) untuk memulai antarmuka obrolan terminal interaktif.
* **Integrasi Log & Telemetri**: Menginisiasi perekaman log telemetri terpusat untuk sesi CLI.

## Cara Pemasangan Lokal
Anda dapat memasang biner ini langsung dari repositori lokal Anda:
```bash
cargo install --path crates/claw10-cli
```
atau setelah dirilis ke crates.io:
```bash
cargo install claw10
```

## Penggunaan
```bash
# Menjalankan wizard konfigurasi awal
claw10 setup

# Memulai TUI Chat
claw10
```
