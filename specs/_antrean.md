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
- Allowlist diverifikasi **`<base>..<commit>`**, bukan `git diff --name-only` di working tree: perubahan belum commit milik agen lain akan menuduh baris yang commit-nya bersih (9 Nov: 13 file `crates/**` modified saat AQ-001 diperiksa).

## `[DOD+]` — kriteria tambahan dari Lead
- Kriteria di luar spec wajib berlabel `[DOD+]` dan **bisa dicek mesin** (anggaran performa, ukuran file, kontras). "Kualitas" tanpa perintah verifikasi bukan kriteria.
- Muncul di **3 task** -> **wajib diputuskan** dalam satu siklus spec: naik jadi `FR` atau **dicabut**. Tidak ada `[DOD+]` abadi.
- Promosi yang menyentuh perilaku/scope tetap lewat **ACC kang** (draft `@system-analyst`, diajukan `@bro-pm`).


## Jalur task `@system-analyst` (keputusan `@bro-pm` 9 Nov 2026 20:49; ditulis di sini atas permintaannya)

- Task spec/analisa ke SA datang **dari `@bro-pm`**, bukan dari Lead: outputnya *mendahului* implementasi, jadi kalau Lead yang menugaskan, penulis hulu dan konsumen hilir jadi satu orang.
- Batasnya tiga: (1) yang dikirim PM adalah artefak pra-implementasi (requirement, SRS/FRD, analisa, spec, Notion); (2) perubahan spec karena temuan hilir (C-13/C-14, qa-plan yang menemukan requirement kurang) masuk **lewat Lead**; (3) SA tidak menerima task kode atau brief ber-DoD kode.
- Tanda tangan SA dua, bukan tiga: approval User di Gate 1 + pernyataan Lead bahwa spec cukup dipecah jadi task.
- Larangan "PM tidak pernah mengirim task ke implementer" tidak berubah. Jalur `@sepuh-be`, QA dan desainer tetap seperti §Gate di bawah.

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


## Cek requirement otomatis (ACC 20:55, poin 6)

Satu perintah di repo — kode ditulis `@sepuh-be`, pemilik semantik `@system-analyst`, dijalankan di gate Lead — mencetak: id terdefinisi, dicoret/deferred, dirujuk-tapi-tak-terdefinisi, varian berhuruf, dan jumlah hidup. Output ditempel ke laporan `siap-kirim`; spec tidak dipakai sebelum output bersih.

- **Usul `@system-analyst` (9 Nov 2026, belum ACC):** output juga menyebut `hidup-tanpa-pembawa` (FR terdefinisi yang tidak dirujuk task mana pun) dan `deferred-yang-masih-dirujuk-qa-plan`. Alasannya: perintah yang hanya memeriksa satu arah bisa keluar bersih sementara sebuah requirement hidup tanpa pelaksana — gate hijau untuk sesuatu yang tidak akan dibangun.


## Skill `speckit*` wajib (perintah User, 9 Nov 2026 20:57 — penegasan kedua)

Vizier memakai alur spec-driven: `spec.md`, `plan.md`, `tasks.md` bukan dokumen lepas, melainkan **artefak skill**. Karena itu tiap task vizier menyebut skill yang dipakai dan artefak yang dihasilkannya — di baris task atau di brief, bukan di kepala orang.

| Artefak | Skill | Pemilik |
|---|---|---|
| `spec.md` (FR, acceptance criteria) | `speckit-specify`, `speckit-clarify` | `@system-analyst` |
| `plan.md`, `research.md`, `data-model.md`, `contracts/` | `speckit-plan` | Lead |
| `tasks.md` | `speckit-tasks` | Lead |
| audit konsistensi spec/plan/tasks | `speckit-analyze` | `@teteh-qa` (dipanggil Lead) |
| daftar periksa mutu per requirement | `speckit-checklist` | `@teteh-qa` |
| commit yang mengikuti konvensi repo | `speckit-git-commit` | yang commit |

- Letak skill di repo ini: `.agents/skills/speckit-*` dan `.claude/skills/speckit-*` — **14 skill `speckit*`**, dua direktori identik (diverifikasi 9 Nov 2026 21:23 WIB).
- Aturan praktis: **jangan** mengganti alat dengan kerja manual yang diulang tiap ronde. Cacat penomoran di `tasks.md` (88 / 110 / 97 di tiga sumber) adalah **cacat artefak**, dan alat untuk itu `speckit-analyze` — bukan `grep` manual yang hasilnya harus dipercaya ulang setiap kali.
- `AQ-001` sendiri dikerjakan **sebagian** dengan cara manual (`grep`/`git grep`) sebelum aturan ini ditegaskan. Itu dicatat sebagai kekurangan baris ini, bukan dicetak sebagai preseden: rekonsiliasi berikutnya wajib lewat `speckit-analyze` dan melampirkan keluarannya.

## Format baris
`ID . judul . status . pemilik . sejak . R: <FR-### | tanpa-spec:<alasan>> . diblokir oleh`

**Koreksi bukti, 9 Nov 2026 (gate teknis Lead, setelah commit):** bukti di atas sempat menyebut commit **`5a8b3f0f`** yang **tidak ada di repo** (`git cat-file -t 5a8b3f0f` -> `fatal: Not a valid object name`). Yang benar adalah **`4ca56796`**. Tiga cacat bukti lain ditemukan dan diperbaiki di commit koreksi ini, semuanya di `tasks.md`:
- **T074, T075** dicentang **tanpa hash sama sekali** -> ditambah `vizier@5eba4226`.
- **Enam anotasi `Landed`** menyebut hash **tanpa repo** (`fbb204f` x3, `0c9c89c`, `3bf44a7`, `cc1d648`). Keempatnya **hanya ada di repo `webui`**; dicek dari root vizier hasilnya "missing" padahal sah. Semua diberi prefix `webui@`.
- Semua anotasi `vizier` kini berprefix `vizier@` supaya `grep` yang jujur sama dengan `grep` yang gampang.

Hasil cek ulang: **65 `[x]`, 65 beranotasi, 22 referensi `repo@hash` unik, 22/22 resolve** (`git cat-file -t`). Tidak ada tes yang dijalankan; penanda `In the tree` tetap berarti "kode ada + ada commit", bukan gate lulus.

**Penegasan tanda tangan teknis (Lead, 9 Nov 2026):** baris tanda tangan di tabel bawah sempat ditulis pihak lain atas nama Lead sebelum Lead memeriksa. Lead sudah memverifikasi ulang sendiri — 110 baris task utuh, 65 `[x]`, 0 rujukan `screens/livevoice/`, 22/22 `repo@hash` resolve — dan menegaskan tanda tangan itu **sah sejak commit `c953f4e9`**, bukan sejak ditulis. Rantai bukti: `aa27590e` (rekonsiliasi) -> `4ca56796` (path) -> `c953f4e9` (bukti diberi repo, hash hantu dikoreksi).

## Papan

| ID | Judul | Status | Pemilik | Sejak | R: | Diblokir oleh |
|---|---|---|---|---|---|---|
| `AQ-001` | Rekonsiliasi checkbox `specs/055-live-voice/tasks.md` dengan riwayat git (label `docs`) | `siap-kirim` | Lead | 9 Nov 2026 20:45 WIB | `tanpa-spec:docs` — allowlist `specs/**` | - |

## Tanda tangan — AQ-001

| Tanda tangan | Pemilik | Status | Bukti |
|---|---|---|---|
| Teknis | Lead | ✅ 9 Nov 2026 | di bawah |
| Perilaku vs spec | `@teteh-qa` | `qa-plan: N/A` (menunggu terima/tolak QA) | label `docs`, tidak menyentuh kode — `qa-plan: N/A` saya tetapkan sebagai Lead; kalau QA menilai baris ini butuh tanda tangannya, status kembali ke `gate` |
| Visual vs handoff | `@teteh-desener` | tidak diminta | label `docs`, bukan `ui` (aturan papan sendiri) |

**Catatan tanda tangan:** tidak ada tanda tangan yang saya tulis atas nama agen lain. Tanda tangan teknis di baris ini milik saya setelah saya periksa sendiri: 110 task utuh, `[x]` 9 -> 65, `[ ]` 101 -> 45, rujukan `screens/livevoice/` 6 -> 0, dan 22 dari 22 anotasi `repo@hash` resolve di repo yang disebut (`git cat-file -t`; satu-satunya yang gagal adalah placeholder `<hash>` di kepala `tasks.md`).


**Bukti teknis** (perintah → keluaran, dijalankan 9 Nov 2026 sebelum commit):
- `grep -cE '^- \[[ x]\] T[0-9]{3}' specs/055-live-voice/tasks.md` → `110` sebelum **dan** sesudah: tidak ada task ditambah atau dihapus, hanya status yang berubah.
- `grep -c '^- \[ \] T' …` → `101` → **`45`**; `grep -c '^- \[x\] T' …` → `9` → **`65`**.
- `grep -c 'app/screens/livevoice/' specs/055-live-voice/tasks.md` → `6` → **`0`** (barisnya ada di `webui/app/screens/chat/LiveVoiceRow.tsx`, bukan `screens/livevoice/`).
- Commit: **`aa27590e`** (rekonsiliasi, `specs/055-live-voice/tasks.md` saja) + **`4ca56796`** (koreksi path `screens/livevoice/` -> `screens/chat/` di `plan.md`/`architecture.md`/`tasks.md`, plus T037/T040/T089).

**Batas yang harus dibaca sebelum mengutip angka 65**: 56 centang baru hanya membuktikan **kode ada di HEAD** (`git grep … HEAD -- <file>`) dan menyebut commit yang memperkenalkannya. **Tidak ada tes yang dijalankan** di rekonsiliasi ini. Karena itu dua penanda dibedakan di kepala `tasks.md`: `Landed … gate passed` (gate dijalankan) vs `In the tree … not gated in AQ-001` (hanya keberadaan). Gate tetap tanda tangan QA.

**45 yang sengaja masih `[ ]`**: fixture VAD (T004), baseline (T006), indikator FR-042 e2e (T039), tombol arsip yang bukan lagi pintu masuk (T037), saklar per-agen (T041), counter backlog (T042), ducking & burst pendek (T047, T048), e2e barge-in/interrupt (T049, T056), seluruh backlog interrupt (T051–T057), siklus & penolakan koneksi kedua (T061–T065), tes kegagalan server (T066), superseded (T093), registri & alasan tertinggal (T095–T097), instrumentasi server (T077), dan seluruh gerbang rilis (T078–T088) serta fase kunci pihak ketiga (T100–T110).
