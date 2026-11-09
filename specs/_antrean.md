# Antrean Kerja (papan keputusan)

Pemilik: `@mamang-lead-be`. Disepakati 9 Nov 2026 (kang muruyung ACC di Workspace `squad-leluhur`).
Tujuan: satu tempat yang bisa dibaca siapa pun, supaya "antre" bukan sopan-sopanan.

Status dokumen: **DISAHKAN** — kang muruyung ACC semua butir pada 9 Nov 2026 20:42 WIB
(*"gw acc semua, setuju kok gw sama pernyataan kalian dan layak dicoba juga untuk flow kedepannya"*).
Berlaku untuk semua project, bukan hanya vizier.

## Aturan dasar
- **WIP = 1 per agen**, bukan per tim. `@sepuh-be` tidak pernah pegang dua brief sekaligus; QA/desainer boleh jalan paralel.
- Task baru hanya masuk `aktif` kalau slot agennya kosong. Kalau penuh -> `antre`, dan **`@bro-pm` yang melapor ke kang**, bukan agen eksekusi.
- Status: `antre` -> `aktif` -> `gate` -> `siap-kirim` -> `selesai` (plus `batal`).
  - `gate` = tiga tanda tangan belum lengkap. `siap-kirim` = tiga tanda tangan lengkap, **kang belum menerima**. `selesai` = kang menerima. Tidak ada `selesai` tanpa kang.
- Yang mengubah baris: Lead (status teknis) dan `@bro-pm` (prioritas). Agen eksekusi hanya melapor.
- Setiap baris `aktif`/`gate` wajib punya DoD (perintah verifikasi + output harapan). Tanpa DoD = belum boleh `aktif`.

## Gate: satu gerbang, tiga tanda tangan
Tidak ada gate terpisah per disiplin. Satu status `gate`, tiga syarat, satu kebenaran.

| Tanda tangan | Pemilik | Bukti wajib |
|---|---|---|
| Teknis | Lead | perintah + exit code + commit SHA + `file:line` |
| Perilaku vs spec | `@teteh-qa` | langkah repro + aktual vs harapan + severity |
| Visual vs handoff | `@teteh-desener` | screenshot + selisih angka (px/token) + rujukan handoff |

- Tanda tangan **visual hanya untuk task berlabel `ui`**; task backend tidak diberi status kosong.
- Veto desainer **wajib berbukti** (tiga butir di atas). Veto tanpa bukti = selera, bukan gate.
- Verdict QA dikirim **satu pesan, mention Lead + `@bro-pm`** — bukan berantai, bukan sembunyi.
- QA tidak menyentuh kode produksi, tidak mengubah tes agar lulus, tidak memutuskan prioritas.
- **QA dan desainer menerima task dari Lead**, lengkap dengan DoD + acceptance criteria dari spec. Bukan dari PM.

## Ronde temuan
- **Ronde = satu paket** (semua blocker + major dalam satu kiriman). Minor tidak masuk hitungan; minor jadi task lanjutan ber-ID.
- **Maks 2 ronde fix.** Ronde ke-3 = **eskalasi ke `@bro-pm`**, bukan ronde lagi. PM memutuskan "kirim dengan catatan" vs "rework".
- Tenggat perbaikan per ronde: **satu chunk kerja**, bukan "seminggu".
- P0/blocker (crash, data loss, security) dilapor **segera**, tidak menunggu paket.

## qa-plan (tahap kontrak)
- Bertingkat: task yang menyentuh perilaku user / kontrak API -> **qa-plan 3-5 skenario kritis**; refactor internal -> `qa-plan: N/A - no behavior change` + alasan.
- **qa-plan hanya menurunkan, tidak menambah.** Skenario tanpa rujukan requirement di spec tidak ditulis di qa-plan; dilaporkan ke Lead -> PM -> spec ditambah.
- Skenario ditulis QA, kode tes ditulis `@sepuh-be`, dan **QA me-review diff file tes** (baca saja) untuk mencegah assert yang melunak.

## Bug lolos ke kang
Wajib satu baris di gate: **"kenapa lolos"**, pilih satu kategori:
(a) tidak ada DoD untuk kasus itu · (b) DoD ada tapi tidak dijalankan · (c) dijalankan tapi alat tidak menangkap · (d) known issue yang sadar diterima.
- (d) wajib langsung jadi task ber-ID di papan ini. Tanpa ID, known issue cuma known sampai lupa.
- Yang diperbaiki gate-nya, bukan orangnya.

## Catatan baris pertama (AQ-001)
Kenapa ini yang pertama: `tasks.md` 055 saat ini menampilkan **101 `[ ]` vs 9 `[x]`**, padahal `git log` menunjukkan T074–T109 sudah di-commit. Angka status yang salah = klaim tanpa bukti, dan itu persis penyakit yang aturan ini obati. Deliverable tunggal: tiap `[x]` punya commit hash pendukung; selisih sebelum/sesudah dilaporkan.
Batas uji: baris ini menguji **papan + tanda tangan teknis** saja (pemilik Lead, tidak menyentuh kode). Untuk menguji jalur Sepuh + QA + desainer end-to-end, butuh satu deliverable kode nyata dari kang/PM.

## Format baris
`ID . judul . status . pemilik . sejak . diblokir oleh`

## Papan

| ID | Judul | Status | Pemilik | Sejak | Diblokir oleh |
|---|---|---|---|---|---|
| `AQ-001` | Rekonsiliasi checkbox `specs/055-live-voice/tasks.md` dengan riwayat git (label `docs`) | `aktif` | Lead | 9 Nov 2026 20:45 WIB | - |
