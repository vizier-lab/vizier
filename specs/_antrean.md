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


## Jalur task per agen
- **`@sepuh-be` (kode)**: task dari Lead, dengan DoD tertulis.
- **`@teteh-qa` dan `@teteh-desener`**: task dari Lead, DoD + acceptance criteria dari spec. Bukan dari PM.
- **`@system-analyst` (spec/analisa)**: task **dari `@bro-pm`**, bukan dari Lead — karena outputnya *mendahului* implementasi. Kalau spec datang dari Lead, penulis hulu dan konsumen hilir jadi satu orang, dan independensi spec hilang di titik itu. Batasnya tiga:
  - yang dikirim PM adalah artefak yang mendahului implementasi (requirement, SRS/FRD, analisa, spec, Notion);
  - perubahan spec karena temuan hilir (C-13/C-14 di 055, qa-plan yang menemukan requirement kurang) masuk **lewat Lead** — pemicunya di hilir, jadi pintu masuknya di hilir;
  - SA **tidak** menerima task kode atau brief ber-DoD kode.
- Tanda tangan SA bukan tiga: approval User di Gate 1 + pernyataan Lead bahwa spec cukup dipecah jadi task.
- Larangan "PM tidak pernah mengirim task ke implementer" tidak berubah.

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
| `AQ-001` | Rekonsiliasi checkbox `specs/055-live-voice/tasks.md` dengan riwayat git (label `docs`) | `siap-kirim` | Lead | 9 Nov 2026 20:45 WIB | `tanpa-spec:docs` — allowlist `specs/**` | - |

## Tanda tangan — AQ-001

| Tanda tangan | Pemilik | Status | Bukti |
|---|---|---|---|
| Teknis | Lead | ✅ 9 Nov 2026 | di bawah |
| Perilaku vs spec | `@teteh-qa` | N/A | `qa-plan: N/A - no behavior change` (label `docs`, tidak menyentuh kode) |
| Visual vs handoff | `@teteh-desener` | N/A | label `docs`, bukan `ui` |

**Bukti teknis** (perintah → keluaran, dijalankan 9 Nov 2026 sebelum commit):
- `grep -cE '^- \[[ x]\] T[0-9]{3}' specs/055-live-voice/tasks.md` → `110` sebelum **dan** sesudah: tidak ada task ditambah atau dihapus, hanya status yang berubah.
- `grep -c '^- \[ \] T' …` → `101` → **`46`**; `grep -c '^- \[x\] T' …` → `9` → **`64`**.
- `grep -c 'app/screens/livevoice/' specs/055-live-voice/tasks.md` → `6` → **`0`** (barisnya ada di `webui/app/screens/chat/LiveVoiceRow.tsx`, bukan `screens/livevoice/`).
- Commit: **`aa27590e`**, path-scoped ke `specs/055-live-voice/tasks.md` saja.

**Batas yang harus dibaca sebelum mengutip angka 64**: 55 centang baru hanya membuktikan **kode ada di HEAD** (`git grep … HEAD -- <file>`) dan menyebut commit yang memperkenalkannya. **Tidak ada tes yang dijalankan** di rekonsiliasi ini. Karena itu dua penanda dibedakan di kepala `tasks.md`: `Landed … gate passed` (gate dijalankan) vs `In the tree … not gated in AQ-001` (hanya keberadaan). Gate tetap tanda tangan QA.

**46 yang sengaja masih `[ ]`**: fixture VAD (T004), baseline (T006), indikator FR-042 e2e (T039), mount (T040, T089), saklar per-agen (T041), counter backlog (T042), ducking & burst pendek (T047, T048), e2e barge-in/interrupt (T049, T056), seluruh backlog interrupt (T051–T057), siklus & penolakan koneksi kedua (T061–T065), tes kegagalan server (T066), superseded (T093), registri & alasan tertinggal (T095–T097), instrumentasi server (T077), dan seluruh gerbang rilis (T078–T088) serta fase kunci pihak ketiga (T100–T110).
