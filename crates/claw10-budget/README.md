# claw10-budget

Layanan pelacakan anggaran (*budget tracking*) agen untuk **Claw10 OS**.

Crate ini menyediakan mekanisme pelacakan, reservasi, dan pelaporan biaya operasional token LLM serta eksekusi perkakas (*tools*) oleh agen, untuk mencegah pembengkakan biaya di luar batas yang ditentukan.

## Fitur Utama
* **Reservasi Anggaran**: Memvalidasi dan menyisihkan biaya prapemanggilan (`reserve`) LLM berdasarkan saldo alokasi.
* **Batas Keras/Lunak (Hard/Soft Limits)**: Menolak eksekusi agen saat saldo melampaui `hard_limit_usd` atau memberi notifikasi saat melampaui `soft_limit_usd`.
* **Perekaman Biaya**: Utilitas pembuatan entri rekam biaya (`CostRecord`) secara real-time.

## Cara Penggunaan
```toml
[dependencies]
claw10-budget = { workspace = true }
```
