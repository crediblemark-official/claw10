# claw10-mission

Manajemen misi dan tujuan tingkat tinggi (*high-level goals*) untuk **Claw10 OS**.

Crate ini mendefinisikan dan mengelola siklus hidup misi yang diberikan oleh pengguna manusia. Misi ini bertindak sebagai tujuan induk yang kemudian akan dipecah oleh agen menjadi tugas-tugas (*tasks*) yang lebih kecil.

## Fitur Utama
* **Mission Lifecycle**: Pengelolaan status misi (Created, Active, Suspended, Completed, Failed).
* **Target & Bukti**: Menentukan kriteria keberhasilan misi secara deterministik berdasarkan pemenuhan bukti kerja (*evidence*).

## Cara Penggunaan
```toml
[dependencies]
claw10-mission = { workspace = true }
```
