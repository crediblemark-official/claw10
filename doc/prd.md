# PRODUCT REQUIREMENTS DOCUMENT

# Claw10 OS

## Recursive, Persistent, and Ephemeral Agent Swarm Operating System

**Versi dokumen:** 1.0
**Status:** Product Baseline
**Tanggal:** 27 Juni 2026
**Model pengembangan:** Build from zero
**Bahasa utama:** Rust
**Antarmuka utama:** Ratatui TUI, CLI, API, Webhook, dan Messaging Gateway
**Model deployment:** Local, private server, VPS, cloud, edge
**Nama kerja produk:** Claw10 OS

---

# 1. Ringkasan Eksekutif

Claw10 OS adalah sistem operasi untuk mengelola organisasi agen AI yang dapat:

1. menerima tujuan dari manusia;
2. menyusun rencana;
3. membentuk tim secara mandiri;
4. membuat agen anak;
5. membagi pekerjaan kepada swarm internal;
6. menjalankan tool pada komputer, server, browser, cloud, dan perangkat;
7. bekerja paralel;
8. memeriksa hasil;
9. mengendalikan biaya;
10. meminta persetujuan manusia;
11. mempelajari pola kerja yang berhasil;
12. membentuk skill baru;
13. berjalan sementara atau terus-menerus;
14. menghentikan agen dan tim secara aman;
15. mempertahankan jejak lengkap setelah agen dihentikan.

Claw10 tidak memakai sekumpulan agen statis.

Setiap agen dapat menjadi pemimpin tim. Agen dapat mengusulkan pembuatan agen anak berdasarkan kebutuhan tugas. Agen anak juga dapat membuat tim baru jika policy, anggaran, dan batas kedalaman mengizinkannya.

Claw10 mendukung dua model kehidupan utama.

## 1.1 Ephemeral Agent

Ephemeral Agent hidup untuk satu task, objective, atau mission.

Setelah pekerjaan selesai, sistem:

* membekukan agen;
* menyimpan hasil;
* menyimpan artifact;
* menyimpan lineage;
* menyimpan audit;
* mengekstrak memory yang valid;
* mencabut credential;
* menghapus workspace sementara;
* menghentikan runtime.

## 1.2 Persistent Agent

Persistent Agent mempertahankan identitas dan tanggung jawab selama:

* 24 jam;
* beberapa hari;
* beberapa bulan;
* satu periode kampanye;
* tanpa batas tanggal tertentu.

Persistent Agent tidak harus menjalankan model secara terus-menerus. Agen dapat tidur saat tidak ada pekerjaan, lalu bangun melalui:

* event;
* heartbeat;
* cron;
* webhook;
* pesan;
* perubahan kondisi;
* perintah manusia.

Identitas, tugas, checkpoint, memory, budget, dan reporting line tetap hidup meskipun proses runtime dipindahkan atau dimulai ulang.

Persistent Agent dapat membentuk:

* ephemeral swarm untuk pekerjaan sementara;
* persistent child team untuk fungsi jangka panjang;
* scheduled team untuk pekerjaan berkala;
* emergency team untuk insiden.

---

# 2. Definisi Produk

Claw10 OS adalah:

> Platform agent swarm yang memungkinkan agen membentuk tim secara rekursif, bekerja sementara atau jangka panjang, menjalankan tindakan nyata, belajar dari pengalaman, dan tetap berada di bawah kendali policy serta manusia.

Claw10 terdiri atas lima bagian utama.

| Bagian             | Fungsi                                                                        |
| ------------------ | ----------------------------------------------------------------------------- |
| Control Plane      | Mengelola identity, organization, mission, task, budget, policy, dan approval |
| Agent Plane        | Menjalankan reasoning, planning, coordination, review, dan delegation         |
| Execution Plane    | Menjalankan tool pada worker terisolasi                                       |
| Intelligence Plane | Mengelola model, context, memory, skill, dan evaluasi                         |
| Operator Plane     | Memberikan kontrol manusia melalui Ratatui, CLI, API, dan kanal komunikasi    |

---

# 3. Latar Belakang

Agen AI modern dapat membaca dokumen, menulis kode, menggunakan browser, memanggil API, mengelola komunikasi, dan menjalankan shell.

Namun, sebagian besar sistem masih memiliki masalah berikut.

## 3.1 Agen terlalu monolitik

Satu agen sering memegang terlalu banyak tool, memory, credential, dan tanggung jawab.

## 3.2 Tim agen bersifat statis

Pengembang harus menentukan seluruh agen sejak awal. Sistem tidak dapat membentuk tim baru berdasarkan kebutuhan aktual.

## 3.3 Delegasi tidak memiliki batas yang jelas

Agen dapat membuat subtugas, tetapi sistem tidak selalu membatasi:

* kedalaman spawn;
* jumlah agen;
* biaya;
* masa hidup;
* privilege;
* context inheritance.

## 3.4 Agen jangka panjang sulit dipelihara

Agen yang berjalan berbulan-bulan membutuhkan:

* checkpoint;
* restart;
* credential rotation;
* policy renewal;
* memory maintenance;
* version migration;
* handover;
* incident recovery.

## 3.5 Agen sementara meninggalkan sampah runtime

Agen dapat meninggalkan:

* process;
* container;
* temporary file;
* browser session;
* token;
* credential;
* cache.

## 3.6 Penyelesaian task tidak memiliki bukti

Agen sering menyatakan tugas selesai tanpa menunjukkan bahwa hasil benar-benar berhasil.

## 3.7 Self-improvement meningkatkan risiko

Memory atau skill yang salah dapat digunakan berulang kali dan mencemari keputusan berikutnya.

## 3.8 Biaya tidak menjadi batas operasional

Recursive swarm dapat berkembang tanpa batas jika tidak memiliki budget dan circuit breaker.

Claw10 menyelesaikan masalah tersebut melalui recursive swarm, governed spawning, dual lifecycle, deterministic policy, evidence-based completion, dan secure teardown.

---

# 4. Visi Produk

Membangun sistem tenaga kerja digital yang dapat membentuk organisasinya sendiri berdasarkan pekerjaan, tetapi tetap transparan, terbatas, terukur, dan dapat dihentikan manusia.

---

# 5. Sasaran Produk

## 5.1 Sasaran utama

1. Memungkinkan agen membuat swarm sendiri.
2. Memungkinkan child agent membuat swarm lanjutan.
3. Mendukung agen sementara dan agen jangka panjang.
4. Memisahkan logical agent dari runtime process.
5. Menjalankan setiap tindakan melalui policy engine.
6. Membatasi privilege, biaya, kedalaman, dan masa hidup.
7. Menyimpan complete agent lineage.
8. Menyimpan evidence dan artifact setiap pekerjaan.
9. Menyediakan persistent memory yang terkontrol.
10. Memungkinkan pembentukan skill yang aman.
11. Mendukung omnichannel interaction.
12. Menyediakan terminal control center.
13. Menyediakan observability terstruktur.
14. Mendukung deployment lokal hingga terdistribusi.
15. Membangun seluruh core dari nol.

## 5.2 Hasil yang diharapkan

```text
Human Goal
→ Root Agent
→ Mission Plan
→ Dynamic Team Formation
→ Recursive Child Swarms
→ Tool Execution
→ Review and Verification
→ Final Handoff
→ Memory and Skill Extraction
→ Continue, Sleep, or Secure Teardown
```

---

# 6. Non-Goals

Versi awal tidak bertujuan untuk:

1. melatih foundation model;
2. membiarkan agen mengubah policy sendiri;
3. memberi unrestricted root access;
4. memberi agen akses global ke seluruh tenant;
5. melakukan transaksi keuangan tanpa approval;
6. mengizinkan spawn tanpa batas;
7. menjalankan physical action tanpa policy;
8. memakai TOON sebagai database;
9. memakai Vector sebagai task broker;
10. memakai ICVS langsung sebagai runtime authorization engine;
11. menjamin semua pekerjaan dapat berjalan tanpa manusia;
12. menyalin source code sistem referensi;
13. membuat public skill marketplace pada MVP;
14. mempertahankan semua raw conversation selamanya;
15. menyimpan secret dalam memory atau prompt.

---

# 7. Prinsip Desain

## 7.1 Human authority

Manusia memiliki kewenangan tertinggi.

Manusia dapat:

* menghentikan agent;
* menghentikan descendant;
* membatalkan mission;
* mencabut tool;
* mengurangi budget;
* menolak approval;
* menghapus memory;
* mengarantina skill;
* mengambil alih task.

## 7.2 Agents propose, kernel decides

Agen dapat mengusulkan:

* team;
* child agent;
* tool call;
* budget allocation;
* skill;
* memory;
* policy amendment.

Kernel deterministik memutuskan apakah usulan boleh dijalankan.

## 7.3 Logical agent is not a process

Agen adalah identity dan state yang persisten.

Container, process, session, atau worker hanya menjadi runtime sementara.

Agen jangka panjang dapat berpindah worker tanpa kehilangan identitas.

## 7.4 Least privilege

Child Agent hanya menerima izin yang diperlukan.

```text
child_permissions ⊆ parent_delegable_permissions
```

## 7.5 Bounded recursion

Recursive spawning selalu dibatasi.

## 7.6 Evidence over claims

Task tidak selesai hanya karena agen mengatakan selesai.

## 7.7 Memory is untrusted until verified

Memory baru harus melalui admission pipeline.

## 7.8 Skills cannot grant privilege

Skill tidak memiliki privilege sendiri.

## 7.9 Cost is part of execution

Setiap mission, task, agent, model, dan tool memiliki batas biaya.

## 7.10 Fail closed

Kegagalan policy, identity, atau approval harus menghentikan tindakan.

## 7.11 Reversible by default

Sistem memilih draft, snapshot, transaction, versioning, dan soft delete sebelum tindakan permanen.

## 7.12 Complete lineage

Setiap agent harus memiliki root mission dan parent yang dapat dilacak.

---

# 8. Target Pengguna

## 8.1 System Owner

Mengelola deployment, provider, worker, storage, dan backup.

## 8.2 Board Operator

Menetapkan goal, budget, batas risiko, dan prioritas.

## 8.3 Human Manager

Mengawasi agent department dan approval.

## 8.4 Security Administrator

Mengelola policy, credential, sandbox, dan incident.

## 8.5 Agent Developer

Membuat template agent, tool, skill, dan evaluator.

## 8.6 Operations Operator

Mengawasi task, worker, heartbeat, dan error.

## 8.7 Auditor

Membaca lineage, keputusan, approval, dan artifact.

## 8.8 End User

Memberikan instruksi melalui terminal, API, chat, atau aplikasi lain.

---

# 9. Use Case Utama

## UC-01 Software Development Swarm

Root Agent membentuk:

* Product Analyst;
* Architect;
* Backend Team;
* Frontend Team;
* Testing Team;
* Security Team;
* Documentation Team.

Setelah milestone selesai, sebagian team dihentikan. Maintenance Agent tetap berjalan sebagai persistent agent.

## UC-02 Research Swarm

Research Lead membuat child agent untuk:

* source discovery;
* data extraction;
* citation verification;
* statistical analysis;
* report writing;
* independent review.

## UC-03 Business Operations

Persistent Operations Manager berjalan berbulan-bulan. Ia membentuk ephemeral swarm untuk setiap campaign atau incident.

## UC-04 Monitoring 24/7

Persistent Monitoring Agent:

* tidur saat tidak ada event;
* bangun saat alarm muncul;
* membuat incident swarm;
* mengoordinasikan mitigasi;
* menutup tim setelah incident selesai.

## UC-05 Sales Organization

Persistent Sales Director mengelola:

* prospect research team;
* proposal team;
* CRM team;
* communication team;
* performance review team.

## UC-06 Personal Digital Staff

Pengguna memiliki persistent Personal Chief of Staff. Agent membuat temporary team untuk perjalanan, riset, administrasi, atau proyek.

## UC-07 Edge Operations

Persistent Edge Supervisor memantau perangkat dan membentuk diagnostic swarm ketika terjadi anomali.

---

# 10. Terminologi

| Istilah          | Definisi                                                   |
| ---------------- | ---------------------------------------------------------- |
| Logical Agent    | Identity dan state agen yang tidak terikat satu process    |
| Runtime Instance | Process atau container yang menjalankan agen               |
| Root Agent       | Agen pertama dalam satu mission                            |
| Parent Agent     | Agen yang meminta pembuatan child                          |
| Child Agent      | Agen yang dibuat untuk objective tertentu                  |
| Descendant       | Semua child dan sub-child di bawah agen                    |
| Swarm Team       | Sekumpulan agen yang mengerjakan satu objective            |
| Spawn            | Proses membuat logical agent baru                          |
| Fork             | Membuat child dari Agent Genome dengan context terpilih    |
| Spawn Broker     | Service yang memvalidasi dan membuat agent                 |
| Agent Genome     | Template pembentukan agent                                 |
| Ephemeral Agent  | Agent yang berakhir bersama task atau objective            |
| Persistent Agent | Agent yang mempertahankan identity lintas task dan restart |
| Legacy Trace     | Jejak final agen setelah termination                       |
| Lineage          | Hubungan root, parent, child, dan descendant               |
| Heartbeat        | Sinyal periodik status logical agent atau worker           |
| Checkpoint       | State yang dapat dipakai untuk resume                      |
| Hibernation      | Persistent agent tidur tanpa runtime aktif                 |
| Secure Teardown  | Proses penutupan runtime dan pencabutan akses              |
| Mission          | Tujuan tingkat tinggi                                      |
| Task             | Unit pekerjaan yang dapat diverifikasi                     |
| Artifact         | Hasil file atau objek pekerjaan                            |
| Evidence         | Bukti bahwa acceptance criteria terpenuhi                  |
| Skill            | Prosedur reusable yang telah diuji                         |
| Policy IR        | Representasi kebijakan internal yang deterministik         |

---

