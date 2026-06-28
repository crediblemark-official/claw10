# Contoh Tugas untuk Uji Coba (Testing) ClawHive OS

Dokumen ini berisi kumpulan contoh instruksi/tugas (*prompts*) yang dapat digunakan untuk menguji fungsionalitas inti ClawHive OS melalui TUI Chat maupun CLI Swarm.

---

## 1. Uji Coba Pembentukan Tim Rekursif (Recursive Spawning)
Menguji kemampuan agen utama (`Root Agent`) untuk memecah masalah besar, memanggil `Spawn Broker` secara otomatis, dan mengelola child agent spesialis.

* **Prompt / Tugas:**
  > "Pecah tugas pembuatan spesifikasi teknis web e-commerce menjadi dua sub-tugas. Buat child agent spesialis 'Security Engineer' untuk menulis modul keamanan, dan child agent 'DB Designer' untuk menyusun skema database. Gabungkan laporan mereka menjadi satu dokumen laporan arsitektur."
* **Apa yang Diverifikasi:**
  * Apakah agen utama membuat child agent baru di tab **Agents**.
  * Apakah request masuk di tab **Broker** / **Spawn Requests** di sidebar kanan TUI.
  * Batas kedalaman spawn (`max_depth`) tetap dipatuhi.

---

## 2. Uji Coba Eksekusi Tindakan Nyata (Execution Plane & Tools)
Menguji kemampuan agen berinteraksi dengan dunia luar melalui HTTP requests dan memanipulasi file lokal.

* **Prompt / Tugas:**
  > "Gunakan tool HTTP untuk memanggil data JSON profil dari `https://httpbin.org/json`. Setelah berhasil, baca data tersebut, kemudian tulis ringkasannya ke dalam file baru bernama `http_profil.md` di workspace proyek."
* **Apa yang Diverifikasi:**
  * Agen berhasil memanggil API publik eksternal.
  * Teks respon ditulis dengan benar ke file `http_profil.md` di direktori proyek.

---

## 3. Uji Coba Kendali Anggaran & Kebijakan (Budgets & Governance)
Menguji fitur keamanan, pembatasan biaya operasional LLM, dan persetujuan tindakan sensitif oleh manusia.

* **Prompt / Tugas:**
  > "Jalankan riset tren AI terbaru menggunakan HTTP tool, namun batasi anggaran token maksimal setara $0.02. Jika ada tindakan penulisan file di luar folder workspace, minta persetujuan (approval) saya terlebih dahulu."
* **Apa yang Diverifikasi:**
  * Apakah sistem membatasi eksekusi saat limit budget tercapai.
  * Munculnya prompt approval sebelum melakukan tindakan sensitif.

---

## 4. Uji Coba Agen Terjadwal (Scheduled & Persistent Lifecycle)
Menguji daur hidup agen yang persisten dan kemampuannya untuk berhibernasi serta dibangunkan secara berkala oleh scheduler.

* **Prompt / Tugas:**
  > "Jadwalkan sebuah task cron: Setiap 5 menit, lakukan pengecekan status internet dengan menembak `https://httpbin.org/get`. Catat log status keberhasilannya ke file `heartbeat_log.txt`."
* **Apa yang Diverifikasi:**
  * Agen terdaftar di tab **Pool** atau **Broker**.
  * Agen masuk ke mode `Hibernating` ketika idle.
  * File `heartbeat_log.txt` ter-update setiap 5 menit secara berkala.

---

## 5. Uji Coba Bukti Hasil Kerja (Evidence-based Completion)
Menguji kriteria penerimaan tugas secara deterministik sebelum menandai tugas selesai.

* **Prompt / Tugas:**
  > "Buat script Python sederhana untuk menghasilkan deret Fibonacci hingga angka ke-10. Jalankan script tersebut di worker lokal, dan berikan bukti (evidence) stdout eksekusinya ke saya untuk verifikasi."
* **Apa yang Diverifikasi:**
  * Status tugas tidak langsung ditandai `Completed` sebelum agen menyertakan bukti log stdout eksekusi python yang nyata.

---

## Tip Pengujian di TUI
Setelah mendapatkan jawaban asisten di chat, Anda dapat mengekspor dan menyimpan jawaban terakhir tersebut menjadi file fisik `.md` langsung dari TUI menggunakan perintah:
```text
:save nama_file.md
```
