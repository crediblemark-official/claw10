# claw10-spawn

*Spawn Broker* pengelolaan pembentukan agen anak secara rekursif untuk **Claw10 OS**.

Crate ini mengawasi proses pembentukan (*spawning*) tim agen anak secara mandiri oleh induk agen pelaksana. Ia memvalidasi parameter pembentukan agen seperti batas kedalaman tim, delegasi izin keamanan (RBAC), alokasi anggaran, dan ketersediaan slot dalam swarm.

## Fitur Utama
* **Spawn Broker**: Menerima, mengevaluasi, menyetujui, atau menolak permohonan inisiasi agen baru.
* **Validasi Kebijakan**: Memastikan batas ukuran swarm (`max_swarm_size`) dan kedalaman rekursif (`max_depth`) dipatuhi demi keamanan operasional LLM.

## Cara Penggunaan
```toml
[dependencies]
claw10-spawn = { workspace = true }
```
