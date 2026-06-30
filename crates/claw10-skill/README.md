# claw10-skill

Registri dan manajemen kemampuan/keterampilan (*skill registry*) untuk **Claw10 OS**.

Crate ini mengelola pemuatan, pencarian, dan pendaftaran modul skill baru (kumpulan kode program, script helper, atau instruksi kustom) yang dapat dipelajari dan dimuat secara dinamis oleh agen untuk menyelesaikan tugas tertentu.

## Fitur Utama
* **Skill Registration**: Memvalidasi integritas berkas skill (misalnya keberadaan berkas instruksi `SKILL.md`).
* **Dynamic Loading**: Menyediakan direktori referensi skill yang dapat diimpor oleh agen di lingkungan runtime.

## Cara Penggunaan
```toml
[dependencies]
claw10-skill = { workspace = true }
```
