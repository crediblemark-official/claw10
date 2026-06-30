# claw10-lineage

Pelacakan silsilah keturunan pembentukan agen (*spawning lineage*) untuk **Claw10 OS**.

Crate ini merekam grafik hubungan hirarki keturunan antara induk agen (*parent agent*) dan anak agen (*child agent*) yang mereka spawn untuk menyelesaikan sub-tugas secara terdistribusi.

## Fitur Utama
* **Grafik Silsilah**: Menyimpan dan memetakan struktur pohon (tree) agen dalam repositori.
* **Pelacakan Rekursif**: Memvalidasi riwayat hubungan hierarki dari root agen hingga ke daun swarm terdalam.

## Cara Penggunaan
```toml
[dependencies]
claw10-lineage = { workspace = true }
```
