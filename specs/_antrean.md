# Antrean Kerja (papan keputusan)

Pemilik: `@mamang-lead-be`. Disepakati 9 Nov 2026 (kang muruyung ACC di Workspace `squad-leluhur`).
Tujuan: satu tempat yang bisa dibaca siapa pun, supaya "antre" bukan sopan-sopanan.

Status dokumen: **DISAHKAN** — ACC kang 9 Nov 2026 **20:42 WIB** (flow gate/QA/desainer) dan **20:55 WIB** (kepemilikan spec + field `R:` + `[DOD+]` + tripwire). Berlaku semua project.

## Aturan dasar
- **WIP = 1 per agen**, bukan per tim. `@sepuh-be` tidak pernah pegang dua brief sekaligus; QA/desainer boleh jalan paralel.
- Task baru hanya masuk `aktif` kalau slot agennya kosong. Kalau penuh -> `antre`, dan **`@bro-pm` yang melapor ke kang**, bukan agen eksekusi.
- Status: `antre` -> `aktif` -> `gate` -> `siap-kirim` -> `selesai` (plus `batal`).
  - `gate` = tiga tanda tangan belum lengkap. `siap-kirim` = tiga tanda tangan lengkap, **kang belum menerima**. `selesai` = kang menerima. Tidak ada `selesai` tanpa kang.
- Yang mengubah baris: Lead (status teknis) dan `@bro-pm` (prioritas). Agen eksekusi hanya melapor.
- Setiap baris `aktif`/`gate` wajib punya DoD (perintah verifikasi + output harapan) **dan** field `R:` (lihat bawah). Tanpa itu = belum boleh `aktif`.

## Kepemilikan artefak (ACC 20:55)
| Artefak | Pemilik | Isi |
|---|---|---|
| `spec.md` | `@system-analyst` | `FR-###`, acceptance criteria, scope in/out, aktor, Delta Log |
| `tasks.md` | Lead | pecahan teknis + DoD (perintah + output harapan) |
| Prioritas & trade-off | `@bro-pm` | apa penting, apa ditunda |

Spec = **apa yang benar**. DoD = **bagaimana dibuktikan**. `specs/**`: Lead boleh edit **mekanis** (path, tautan, format); **semantik requirement = `@system-analyst`**. Penomoran memakai `FR-###` yang sudah ada — **dilarang** membuat skema kedua. Registry wajib membedakan **hidup / dicoret / deferred** (termasuk varian berhuruf seperti `FR-036a`).

## Field `R:` — wajib di tiap baris
- `R: FR-0xx` **atau** `R: tanpa-spec:<alasan>`. Dua-duanya kosong = **tidak boleh `aktif`**.
- `tanpa-spec` hanya sah kalau `git diff --name-only` **seluruhnya** ada di allowlist: `specs/**`, `docs/**`, `*.md`, `scripts/**`, file tes. Satu path di `crates/**`, `webui/**`, `app/**` -> **wajib FR**, bukan diskusi. Diverifikasi **path**, bukan kalimat alasan.
- Nilai `R:` wajib muncul di laporan `siap-kirim`.

## `[DOD+]` — kriteria tambahan dari Lead
- Kriteria di luar spec wajib berlabel `[DOD+]` dan **bisa dicek mesin** (anggaran performa, ukuran file, kontras). "Kualitas" tanpa perintah verifikasi bukan kriteria.
- Muncul di **3 task** -> **wajib diputuskan** dalam satu siklus spec: naik jadi `FR` atau **dicabut**. Tidak ada `[DOD+]` abadi.
- Promosi yang menyentuh perilaku/scope tetap lewat **ACC kang** (draft `@system-analyst`, diajukan `@bro-pm`).

## Gate: satu gerbang, tiga tanda tangan
Tidak ada gate terpisah per disiplin. Satu status `gate`, tiga syarat, satu kebenaran.

| Tanda tangan | Pemilik | Bukti wajib |
|---|---|---|
| Teknis | Lead | perintah + exit code + commit SHA (sebut repo: `vizier@…` / `webui@…`) + `file:line` |
| Perilaku vs spec | `@teteh-qa` | langkah repro + aktual vs harapan + severity |
| Visual vs handoff | `@teteh-desener` | screenshot + selisih angka (px/token) + rujukan handoff |

- Tanda tangan **visual hanya untuk task berlabel `ui`**; task backend tidak diberi status kosong.
- Veto desainer **wajib berbukti**. Veto tanpa bukti = selera, bukan gate.
- Verdict QA dikirim **satu pesan, mention Lead + `@bro-pm`**.
- QA tidak menyentuh kode produksi, tidak mengubah tes agar lulus, tidak memutuskan prioritas.
- **QA dan desainer menerima task dari Lead**, lengkap dengan DoD + acceptance criteria dari spec.

## Ronde temuan
- **Ronde = satu paket** (semua blocker + major dalam satu kiriman). Minor tidak masuk hitungan; minor jadi task lanjutan ber-ID.
- **Maks 2 ronde fix.** Ronde ke-3 = **eskalasi ke `@bro-pm`**, bukan ronde lagi.
- Tenggat perbaikan per ronde: **satu chunk kerja**.
- P0/blocker (crash, data loss, security) dilapor **segera**.

## qa-plan (tahap kontrak)
- Bertingkat: menyentuh perilaku user / kontrak API -> **3-5 skenario kritis**; refactor internal -> `qa-plan: N/A - no behavior change`.
- **qa-plan hanya menurunkan, tidak menambah.** Skenario tanpa rujukan `FR` tidak ditulis di qa-plan; lapor ke Lead.
- Skenario ditulis QA, kode tes ditulis `@sepuh-be`, **QA me-review diff file tes** (baca saja).

## Bug lolos ke kang
Wajib satu baris **"kenapa lolos"**: (a) tidak ada DoD untuk kasus itu · (b) DoD ada tapi tidak dijalankan · (c) dijalankan tapi alat tidak menangkap · (d) known issue yang sadar diterima.
- (d) wajib langsung jadi task ber-ID. Yang diperbaiki gate-nya, bukan orangnya.

## Tripwire (bukan aturan baru)
```
tripwire: task 1-3 | diblokir-spec: 0/3 | tanpa-spec: 0/5
```
Kalau **2 dari 3** task diblokir "spec belum siap", atau **>2 dari 5** baris pakai `tanpa-spec`, aturan dibawa ke `@bro-pm`/kang untuk direvisi — bukan dilonggarkan sendiri.

## Format baris
`ID . judul . status . pemilik . sejak . R: <FR-### | tanpa-spec:<alasan>> . diblokir oleh`

## Papan

| ID | Judul | Status | Pemilik | Sejak | R: | Diblokir oleh |
|---|---|---|---|---|---|---|
| `AQ-001` | Rekonsiliasi checkbox `specs/055-live-voice/tasks.md` dengan riwayat git (label `docs`) | `aktif` | Lead | 9 Nov 2026 20:45 WIB | `tanpa-spec:docs` — allowlist `specs/**` | - |
