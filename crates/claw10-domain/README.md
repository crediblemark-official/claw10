# claw10-domain

Model domain terpusat dan tipe data fundamental untuk **Claw10 OS**.

Crate ini mendefinisikan seluruh struktur data, tipe data baru (Newtypes), dan enum utama yang digunakan bersama oleh seluruh komponen di dalam ekosistem Claw10 OS.

## Fitur Utama
* **Tipe Data Agen & Tugas**: Model untuk `Agent`, `AgentId`, `Task`, `TaskId`, `TaskState`.
* **Keamanan & Otorisasi**: Struktur data untuk `Identity`, `IdentityId`, `Credential`, `Permission`, `RoleId`.
* **Anggaran & Pelacakan**: Struktur `Budget`, `CostRecord`, dan enum `CostCategory`.
* **Persetujuan & Hasil**: Representasi untuk `ToolApprovalRequest` dan `Evidence`.

## Cara Penggunaan
Tambahkan ke `Cargo.toml` sub-crate Anda:
```toml
[dependencies]
claw10-domain = { workspace = true }
```
