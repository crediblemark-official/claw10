# claw10-auth

Manajemen identitas internal, kredensial, dan otorisasi peran (RBAC) untuk **Claw10 OS**.

Crate ini menyediakan sistem kontrol hak akses internal untuk agen dalam swarm. Ini memastikan agen anak yang dibentuk (*spawned*) hanya memiliki wewenang yang aman dan terbatas dari delegasi induknya.

## Fitur Utama
* **RBAC Agen**: Pemetaan peran ke kumpulan hak akses (*permissions*) dan penggabungan/deduplikasi peran.
* **Manajemen Identitas**: Pembentukan objek identitas unik untuk agen (`create_agent_identity`).
* **Verifikasi Kredensial**: Penerbitan, pembatasan cakupan (*scope verification*), masa kadaluarsa, serta pencabutan kredensial tugas agen anak.

## Cara Penggunaan
```toml
[dependencies]
claw10-auth = { workspace = true }
```
