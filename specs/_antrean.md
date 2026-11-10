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

`tasks.md` berikutnya lahir dari `.specify/templates/tasks-template.md`, dan sejak commit `80cadbac` template itu memuat `R:`, `[DOD+]`, dan dua penanda bukti — jadi aturan ini bertahan saat `speckit-tasks` dijalankan ulang. Tidak retroaktif: `tasks.md` lama tanpa `R:` tidak batal; kewajibannya dicek per baris saat task itu masuk `aktif`.

## `[DOD+]` — kriteria tambahan dari Lead
- Kriteria di luar spec wajib berlabel `[DOD+]` dan **bisa dicek mesin** (anggaran performa, ukuran file, kontras). "Kualitas" tanpa perintah verifikasi bukan kriteria.
- Muncul di **3 task** -> **wajib diputuskan** dalam satu siklus spec: naik jadi `FR` atau **dicabut**. Tidak ada `[DOD+]` abadi.
- Promosi yang menyentuh perilaku/scope tetap lewat **ACC kang** (draft `@system-analyst`, diajukan `@bro-pm`).


## Jalur task `@system-analyst` (keputusan `@bro-pm` 9 Nov 2026 20:49; ditulis di sini atas permintaannya)

**Status: DISAHKAN kang muruyung, 10 Nov 2026 09:36** (*"Iya terima juga"*, menjawab item 2 di workspace `vizier-dev`/`general`). Sejak sini bagian ini **mengikat**: task spec/analisa ke SA datang dari `@bro-pm`. Sebelum tanggal itu ia hanya keputusan PM yang belum sah — dicatat supaya tidak ada yang mengira ia berlaku surut.

- Task spec/analisa ke SA datang **dari `@bro-pm`**, bukan dari Lead: outputnya *mendahului* implementasi, jadi kalau Lead yang menugaskan, penulis hulu dan konsumen hilir jadi satu orang.
- Batasnya tiga: (1) yang dikirim PM adalah artefak pra-implementasi (requirement, SRS/FRD, analisa, spec, Notion); (2) perubahan spec karena temuan hilir (C-13/C-14, qa-plan yang menemukan requirement kurang) masuk **lewat Lead**; (3) SA tidak menerima task kode atau brief ber-DoD kode.
- Tanda tangan SA dua, bukan tiga: approval User di Gate 1 + pernyataan Lead bahwa spec cukup dipecah jadi task.
- Larangan "PM tidak pernah mengirim task ke implementer" tidak berubah. Jalur `@sepuh-be`, QA dan desainer tetap seperti §Gate di bawah.

## Skill `speckit*` wajib di vizier (perintah User 9 Nov 2026 20:57)

Alur lengkapnya di memori `vizier/speckit-workflow-roles`; yang mengikat papan ini cuma satu hal: **tiap task vizier menyebut skill `speckit*` yang dipakai dan artefak yang dihasilkan**, atau `tanpa-skill:<alasan>` untuk task yang memang bukan kerja spec-kit (mis. perbaikan path, rekonsiliasi dokumen). Tanpa itu, brief-nya belum lengkap.

- Pemilik langkah: specify/clarify = `@system-analyst` · plan/tasks/constitution/git = Lead · implement = `@sepuh-be` · analyze/checklist = `@teteh-qa`.
- `tasks.md` adalah artefak `speckit-tasks`, bukan dokumen lepas; cacat penomoran di dalamnya diperiksa dengan `speckit-analyze`, bukan grep manual yang diulang tiap ronde.
- Cek requirement otomatis (§ di atas) **bukan** pengganti `speckit-analyze`/`speckit-checklist` — alat tambahan di gate Lead, bukan langkah baru dalam alur.

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


## Cek requirement otomatis (ACC 20:55, poin 6; scope diputuskan `@bro-pm` 9 Nov 21:22)

Satu perintah di repo — kode ditulis `@sepuh-be`, pemilik semantik `@system-analyst`, dijalankan di gate Lead — mencetak: id terdefinisi, dicoret/deferred, dirujuk-tapi-tak-terdefinisi, varian berhuruf, dan jumlah hidup. Output ditempel ke laporan `siap-kirim`; spec tidak dipakai sebelum output bersih.

- **Scope default: per-fitur.** `--all` hanya untuk audit dan **tidak boleh** dipakai sebagai dasar gate: kerusakan FR di fitur lain tidak boleh memblokir fitur yang sedang dikerjakan. Output **wajib** menyebut cakupannya (`scope: 055`), supaya "bersih" tidak pernah dibaca sebagai "seluruh repo bersih".
- **Usul `@system-analyst` (9 Nov 2026, belum ACC):** output juga menyebut `hidup-tanpa-pembawa` (FR terdefinisi yang tidak dirujuk task mana pun) dan `deferred-yang-masih-dirujuk-qa-plan`. Alasannya: perintah yang hanya memeriksa satu arah bisa keluar bersih sementara sebuah requirement hidup tanpa pelaksana — gate hijau untuk sesuatu yang tidak akan dibangun. **Jangan ditahan untuk versi pertama checker.**

## Format baris
`ID . judul . status . pemilik . sejak . R: <FR-### | tanpa-spec:<alasan>> . diblokir oleh`

**Koreksi bukti, 9 Nov 2026 (gate teknis Lead, setelah commit):** bukti di atas sempat menyebut commit **`5a8b3f0f`** yang **tidak ada di repo** (`git cat-file -t 5a8b3f0f` -> `fatal: Not a valid object name`). Yang benar adalah **`4ca56796`**. Tiga cacat bukti lain ditemukan dan diperbaiki di commit koreksi ini, semuanya di `tasks.md`:
- **T074, T075** dicentang **tanpa hash sama sekali** -> ditambah `vizier@5eba4226`.
- **Enam anotasi `Landed`** menyebut hash **tanpa repo** (`fbb204f` x3, `0c9c89c`, `3bf44a7`, `cc1d648`). Keempatnya **hanya ada di repo `webui`**; dicek dari root vizier hasilnya "missing" padahal sah. Semua diberi prefix `webui@`.
- Semua anotasi `vizier` kini berprefix `vizier@` supaya `grep` yang jujur sama dengan `grep` yang gampang.

Hasil cek ulang: **65 `[x]`, 65 beranotasi, 22 referensi `repo@hash` unik, 22/22 resolve** (`git cat-file -t`). Tidak ada tes yang dijalankan; penanda `In the tree` tetap berarti "kode ada + ada commit", bukan gate lulus.

**Penegasan tanda tangan teknis (Lead, 9 Nov 2026):** baris tanda tangan ini **saya tulis sendiri**, di commit `557739fe` — bukan pihak lain, dan koreksi ini menggantikan catatan sebelumnya yang menyebut "ditulis pihak lain" (itu salah, dan sudah saya tarik). Lead sudah memverifikasi ulang sendiri — 110 baris task utuh, 65 `[x]`, 0 rujukan `screens/livevoice/`, 22/22 `repo@hash` resolve — dan menegaskan tanda tangan itu **sah sejak commit `c953f4e9`**, bukan sejak ditulis. Rantai bukti: `aa27590e` (rekonsiliasi) -> `4ca56796` (path) -> `c953f4e9` (bukti diberi repo, hash hantu dikoreksi).

## Insiden proses — AQ-001 (9 Nov 2026)

Dicatat terpisah dari status task, sesuai permintaan `@bro-pm`:

| Fakta | Bukti |
|---|---|
| Baris dibuka `aktif` **tanpa DoD tertulis** di barisnya | `11db2834` — melanggar aturan papan sendiri ("tanpa DoD = belum boleh `aktif`") |
| Rekonsiliasi, tanda tangan teknis, dan status `siap-kirim` ditulis **oleh orang yang sama** (Lead), tanpa DoD lebih dulu | `aa27590e`, `557739fe` |
| Baris papan menyebut commit **`5a8b3f0f`** yang tidak ada — hash dikarang sebelum commit-nya dibuat | `git cat-file -t 5a8b3f0f` -> `fatal: Not a valid object name`; yang benar `4ca56796` |
| "Siapa yang pegang pena" **tidak bisa dijawab dari repo** | ketujuh commit 9 Nov punya author git identik (`Robi Naufal Kaosar`); tidak ada penanda agen |

**Lubang aturan yang terbuka karena ini:** baris milik Lead tidak punya peninjau kedua — Lead adalah pelaksana *dan* pemilik tanda tangan teknis. Usul: baris berlabel `docs` milik Lead wajib dibaca satu agen lain (QA cukup membaca diff, tanpa menjalankan tes) sebelum `siap-kirim`. **Belum sah** sampai ACC kang.

## Papan

| ID | Judul | Status | Pemilik | Sejak | R: | Diblokir oleh |
|---|---|---|---|---|---|---|
| `AQ-001` | Rekonsiliasi checkbox `specs/055-live-voice/tasks.md` dengan riwayat git (label `docs`) | **`selesai`** | Lead | 9 Nov 2026 20:45 WIB | `tanpa-spec:docs` — allowlist `specs/**` | - · diterima kang **10 Nov 2026 09:36** (*"Ok terima"*) |
| `T110` | Kunci yang dicabut/kedaluwarsa mengakhiri sesi live yang ia buka (FR-063, SC-013) | `gate` | `@sepuh-be` | 9 Nov 2026 21:32 WIB | `FR-063` | tanda tangan **teknis ✅ 10 Nov 09:15** (DoD v2, 5/5 tes modul + flag-off); menunggu **tanda tangan perilaku QA** (4 skenario). SC-013 "1 detik" **belum terbukti** — lihat §DoD |
| `GI-01` | SC-013: sesi berakhir **≤1 detik** setelah kunci dicabut — belum ada alat yang mengukur jeda nyata dari HTTP revoke sampai frame sampai ke klien | `antre` | `@sepuh-be` | 10 Nov 2026 09:20 WIB | `FR-063` | butuh uji waktu dengan server hidup; bukan tes unit |
| `GI-02` | FR-063: **turn yang berjalan dibatalkan** saat kunci dicabut — kode memanggil `abort_turn` (`session.rs:541-542`), tapi tak satu pun tes `key_tests` meng-assert bahwa turn berhenti | `antre` | `@sepuh-be` | 10 Nov 2026 09:20 WIB | `FR-063` | satu assert baru di `key_tests`; belum ditulis |

**Catatan penomoran (10 Nov 09:15):** bukti AQ-001 di bawah berlaku untuk **110 task** dan dibekukan di commit-commitnya. Sesudahnya `@system-analyst` menambahkan **T111 [PROVISIONAL]** (`a53bfcb5`, disempurnakan `ef846fbd`), jadi hitungan hari ini **111 task: 65 `[x]` / 46 `[ ]`** — bukan 110/45. Penambahan itu diterima Lead: isinya provisional, menunggu asesmen saya + Q12 dari kang, dan **tidak** masuk gate rilis ini.

## Tanda tangan — AQ-001

| Tanda tangan | Pemilik | Status | Bukti |
|---|---|---|---|
| Teknis | Lead | ✅ 9 Nov 2026 | di bawah |
| Perilaku vs spec | `@teteh-qa` | `qa-plan: N/A` — **diterima QA 21:23, tanpa tanda tangan** | label `docs`, tidak menyentuh kode (qa-plan bertingkat: hanya task perilaku/API yang wajib 3-5 skenario). QA boleh membantah dengan bukti; kalau dibantah, status kembali ke `gate` |
| Visual vs handoff | `@teteh-desener` | tidak diminta | label `docs`, bukan `ui` (aturan papan sendiri) |

**Catatan tanda tangan:** tidak ada tanda tangan yang saya tulis atas nama agen lain. Tanda tangan teknis di baris ini milik saya setelah saya periksa sendiri: 110 task utuh, `[x]` 9 -> 65, `[ ]` 101 -> 45, rujukan `screens/livevoice/` 6 -> 0, dan 22 dari 22 anotasi `repo@hash` resolve di repo yang disebut (`git cat-file -t`; satu-satunya yang gagal adalah placeholder `<hash>` di kepala `tasks.md`).


**Bukti teknis** (perintah → keluaran, dijalankan 9 Nov 2026 sebelum commit):
- `grep -cE '^- \[[ x]\] T[0-9]{3}' specs/055-live-voice/tasks.md` → `110` sebelum **dan** sesudah: tidak ada task ditambah atau dihapus, hanya status yang berubah.
- `grep -c '^- \[ \] T' …` → `101` → **`45`**; `grep -c '^- \[x\] T' …` → `9` → **`65`**.
- `grep -c 'app/screens/livevoice/' specs/055-live-voice/tasks.md` → `6` → **`0`** (barisnya ada di `webui/app/screens/chat/LiveVoiceRow.tsx`, bukan `screens/livevoice/`).
- Commit: **`aa27590e`** (rekonsiliasi, `specs/055-live-voice/tasks.md` saja) + **`4ca56796`** (koreksi path `screens/livevoice/` -> `screens/chat/` di `plan.md`/`architecture.md`/`tasks.md`, plus T037/T040/T089).

**Batas yang harus dibaca sebelum mengutip angka 65**: 56 centang baru hanya membuktikan **kode ada di HEAD** (`git grep … HEAD -- <file>`) dan menyebut commit yang memperkenalkannya. **Tidak ada tes yang dijalankan** di rekonsiliasi ini. Karena itu dua penanda dibedakan di kepala `tasks.md`: `Landed … gate passed` (gate dijalankan) vs `In the tree … not gated in AQ-001` (hanya keberadaan). Gate tetap tanda tangan QA.

- **Aturan kutipan (diminta `@teteh-qa` 21:23):** angka **65** tidak boleh dikutip tanpa kalimat batas di atas. Tanpa itu 65 beredar sebagai "sudah dites" di brief berikutnya, dan QA yang diminta mempertanggungjawabkannya di gate.
- **Rincian 22 anotasi `repo@hash`** (diukur ulang 21:35 dengan `grep -oE` + `sort -u`): **vizier 11 · webui 10 · design 1**. Sebutan "vizier 13" di percakapan 21:16 salah hitung; totalnya tetap 22 dan tetap 22/22 resolve.
**45 yang sengaja masih `[ ]`**: fixture VAD (T004), baseline (T006), indikator FR-042 e2e (T039), tombol arsip yang bukan lagi pintu masuk (T037), saklar per-agen (T041), counter backlog (T042), ducking & burst pendek (T047, T048), e2e barge-in/interrupt (T049, T056), seluruh backlog interrupt (T051–T057), siklus & penolakan koneksi kedua (T061–T065), tes kegagalan server (T066), superseded (T093), registri & alasan tertinggal (T095–T097), instrumentasi server (T077), dan seluruh gerbang rilis (T078–T088) serta fase kunci pihak ketiga (T100–T110).



## Keputusan rotasi kunci (FR-063), 10 Nov 2026 09:20

**Temuan `@teteh-qa`:** `grep -rn rotate crates/vizier-http/src/api/v1` → **0 hit**. FR-063 menyebut kunci "can be rotated", tapi tidak ada route rotasi.

**Keputusan saya, dengan buktinya:** rotasi **tidak butuh route sendiri** — ia komposisi dua route yang sudah ada: `create_api_key` (`crates/vizier-http/src/api/v1/auth.rs:294`) dan `delete_api_key` (`:404`). Kontrak sudah menuliskannya begitu: *"key rotated (new key, old one retired) → the old key stops working; a session opened with it ends | reason `key_revoked`"* (`contracts/live-voice-frames.md:362`). Jadi requirement punya pembawa; yang kurang hanya **satu baris di `spec.md`** yang menyebut "rotate = buat kunci baru + cabut yang lama, dengan akibat yang sama seperti revoke". Itu semantik spec → `@system-analyst`, bukan saya.

**Yang tidak saya putuskan:** apakah rotasi harus punya endpoint atomik sendiri (supaya tidak ada jendela dua kunci aktif). Kalau kang mau itu, ia requirement baru, bukan penafsiran FR-063.
## DoD — T110 (v2, 10 Nov 2026 09:15 — DoD #3 lama diganti karena **gate hijau palsu**)

**Kenapa diganti:** `cargo test -p vizier-core --features provider-live-voice end_owned_by_key` keluar dengan **exit 0 sambil menjalankan nol tes** — `end_owned_by_key` adalah nama *method* (`session.rs:519`), bukan nama tes, jadi filter mencocokkan nol fungsi dan Rust tetap melaporkan `test result: ok`. Diukur ulang: **7 target, 7x `running 0 tests`, exit 0.** Temuan `@teteh-qa` 9 Nov 21:45; saya reproduksi sendiri 10 Nov.

> **Aturan baru (berlaku semua DoD):** perintah `cargo test <filter>` yang menjalankan **0 tes tetap exit 0**. DoD wajib menyebut **jumlah tes yang harus jalan** (`running N tests`, N > 0) atau memakai path modul lengkap. Exit code saja bukan bukti.

| # | Perintah | Output harapan | Diukur 10 Nov 09:15 |
|---|---|---|---|
| 1 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 5 tests` · 5 lulus · 0 gagal | ✅ **5 passed / 0 failed**, 1.33 s, exit 0 |
| 2 | `cargo check -p vizier-core` (tanpa fitur) | `Finished`, exit 0 — FR-041: build tanpa `provider-live-voice` tetap jalan | ✅ exit 0, 44,85 s |
| 3 | `cargo test -p vizier-core --lib live_voice` (tanpa fitur) | **6 tes** lulus — semuanya tes *setting agent* (`schema::agent`, `service::agents::live_voice_input_tests`), **nol** tes modul `live_voice` | ✅ 6 passed / 0 failed |
| 4 | `cd webui && npm test -- session` | `key_revoked` beda kalimat dari `key_expired` dan dari `pressed_again` | ✅ diukur `@teteh-qa`: 2 file, **65 passed / 1 skipped**, 922 ms |

**Yang DoD ini tetap belum buktikan — wajib disebut di laporan gate:**
- **SC-013 "within 1 s"** — belum ada alat. Tes #1 membuktikan jalur, alasan, dan pembersihan registry; tidak ada yang mengukur jeda nyata dari HTTP revoke sampai frame sampai ke klien. Butuh uji waktu dengan server hidup.
- **"Turn in flight is cancelled" tidak di-assert.** Kode memanggil `abort_turn` sebelum `end` (`session.rs:541-542`), tapi lima tes `key_tests` hanya menyatakan jumlah, alasan, state, dan isi registry.
- **Sisi klien diuji lewat jalur generik**, bukan lewat alasan kunci (`session.test.ts:692` `provider_failed`, `:702` `idle`). Jalurnya sama, tapi "sama" itu inferensi.

**Batas gate yang lebar, dan bukan milik fitur ini:** `cargo test -p vizier-core` (seluruh crate, tanpa fitur) = **1733 lulus, 8 gagal**; kedelapan kegagalan semuanya `projects::tools::lsp_tests` + `service::project_lsp` ("the server never became ready", 20 s), **repro sendirian** setelah `cargo build -p vizier-core --example fake_lsp`. Itu fitur 054, bukan 055 — makanya DoD #2/#3 di atas dipersempit, bukan gate dibiarkan merah karena warisan fitur lain.

**Pemilik:** gate teknis saya **✅ 10 Nov** (baris #1–#3) · tanda tangan perilaku `@teteh-qa` (task perilaku → `qa-plan` 4 skenario sudah ditulis QA; belum tanda tangan).
