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

**Lubang aturan yang terbuka karena ini:** baris milik Lead tidak punya peninjau kedua — Lead adalah pelaksana *dan* pemilik tanda tangan teknis.

> **Aturan: peninjau kedua untuk baris `docs` milik Lead — DISAHKAN kang muruyung, 10 Nov 2026 09:37** (*"Ok setuju"*, item 3). Status **uji coba**, bukan permanen.

Empat pagar, dan melanggarnya membuat peninjauan itu batal:

1. **Baca saja, tanpa tes.** Peninjau membaca diff dan bukti yang ditulis; ia tidak menjalankan perintah, tidak menyentuh kode, tidak menambah tes.
2. **Hanya baris berlabel `docs` yang pemiliknya Lead.** Baris kode, atau baris milik agen lain, tidak masuk cakupan ini.
3. **Hanya boleh menahan kalau bukti tidak mendukung klaim** — bukan karena selera, bukan karena format, bukan karena "menurut saya lebih rapi". Menahan wajib menyebut bukti mana yang tidak cocok.
4. **Uji coba:** kalau aturan ini dalam praktiknya menahan task tanpa alasan yang bisa dicek mesin, ia dicabut di `@bro-pm`/kang, bukan dilonggarkan diam-diam.

Peninjau yang wajar: `@teteh-qa` (ia sudah membaca diff tes sebagai bagian gate-nya). Untuk AQ-001 aturan ini **tidak berlaku surut** — kang sudah menerima baris itu, jadi tidak ada yang perlu dibuka ulang.

## Papan

| ID | Judul | Status | Pemilik | Sejak | R: | Diblokir oleh |
|---|---|---|---|---|---|---|
| `AQ-001` | Rekonsiliasi checkbox `specs/055-live-voice/tasks.md` dengan riwayat git (label `docs`) | **`selesai`** | Lead | 9 Nov 2026 20:45 WIB | `tanpa-spec:docs` — allowlist `specs/**` | - · diterima kang **10 Nov 2026 09:36** (*"Ok terima"*) |
| `T110` | Kunci yang dicabut/kedaluwarsa mengakhiri sesi live yang ia buka (FR-063, SC-013) | `gate` (ronde 2 selesai) | `@sepuh-be` | 9 Nov 2026 21:32 WIB | `FR-063` | **menunggu tanda tangan perilaku `@teteh-qa`** (`qa-plan` 4 skenario, entry 813). Gate teknis saya ✅ untuk `30500bf5` + `093c2b92`. Terbuka dan tidak menghalangi: `GI-01` (SC-013 ≤1 s), `GI-05` (pencabutan tidak dicek per frame) |
| `GI-01` | SC-013: sesi berakhir **≤1 detik** setelah kunci dicabut — belum ada alat yang mengukur jeda nyata dari HTTP revoke sampai frame sampai ke klien | `antre` | `@sepuh-be` | 10 Nov 2026 09:20 WIB | `FR-063` | butuh uji waktu dengan server hidup; bukan tes unit |
| `GI-02` | FR-063: **turn yang berjalan dibatalkan** saat kunci dicabut — kode memanggil `abort_turn` (`session.rs:541-542`), tapi tak satu pun tes `key_tests` meng-assert bahwa turn berhenti | `antre` | `@sepuh-be` | 10 Nov 2026 09:20 WIB | `FR-063` | satu assert baru di `key_tests`; belum ditulis |
| `GI-03` | **Kedaluwarsa ditegakkan pada frame berikutnya**, bukan pada detiknya (`topic.rs:410-415`). Sesi yang **diam** hidup melewati masa berlaku kuncinya dan tidak ada yang mengakhirinya — bukan "salah label `idle`", karena idle-stop (`T062`) belum ada | `antre` | `@sepuh-be` | 10 Nov 2026 09:40 WIB | **`FR-063a`** | **diputuskan 10 Nov 09:48:** perilaku ini sekarang **dijanjikan** oleh FR-063a (`spec.md` @ `28ed1668`), bukan cacat. Yang tersisa cuma interaksi dengan `T062` (ordering dependency), bukan timer penyapu |
| `GI-04` | Skenario 4 qa-plan (alasan `key_revoked` sampai ke layar) lulus **secara komposisi**, bukan satu assert: kalimatnya diuji `session.test.ts:210-213`, mekanisme terminal diuji dengan alasan `idle` di `:702` | `antre` | `@sepuh-be` | 10 Nov 2026 09:40 WIB | `FR-063` | satu tes klien yang mengirim `key_revoked` dan membaca kalimatnya di baris |
| `GI-05` | FR-064: **pencabutan tidak dicek per frame.** `key_lapsed` (`topic.rs:67-74`) hanya membaca `principal.method` yang distempel sekali saat koneksi dibuka (`topic.rs:243`); pencabutan hanya didorong `delete_api_key`. Socket yang masih terbuka bisa mengirim `live_voice_start` lagi dengan kunci yang sudah dicabut | `antre` | `@sepuh-be` | 10 Nov 2026 09:50 WIB | `FR-064` | **belum dibuktikan** (tidak ada tes). Satu tes "revoke → `start` lagi di socket yang sama" menyelesaikannya; temuan `@system-analyst`, bukan asumsi |
| `GI-06` | `lsp_tests` 8 kegagalan **bukan** karena fixture belum dibangun: `fake_lsp()` menghitung path dari `current_exe()`, dan di mesin ini binary tes lib hidup di `target/debug/build/vizier-core/<hash>/out/`, jadi ia mencari `target/debug/build/vizier-core/examples/fake_lsp` yang tidak ada | `antre` | `@sepuh-be` | 10 Nov 2026 11:05 WIB | `FR-041` | terbukti: contoh `fake_lsp` **sudah** dibangun di `target/debug/examples/` dan 8 tes tetap gagal (`the server never became ready`, 20 s); perbaikan = path fixture tahan-layout, bukan `cargo build --example` |
| `GI-07` | **Jalur kedaluwarsa di socket tidak punya tes yang melewati socket.** | **`selesai`** | `@sepuh-be` | 10 Nov 2026 11:20 WIB | `FR-063` | ditutup `093c2b92` (ronde 2 T110): dua tes in-crate, 6 passed, dan terbukti gagal-nya dengan mutasi — lihat §Gate teknis — T110 ronde 2 |

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



## DoD — T110 ronde 1 (10 Nov 2026 09:40): jalur kedaluwarsa wajib membatalkan turn

**Cacatnya, dari kode bukan dari laporan:** dua jalur menuju `KeyExpired`, hanya satu yang membatalkan turn.
- **Cabut** — `delete_api_key` → `end_owned_by_key` (`session.rs:651-675`): `abort_turn(BargeIn)` → `end(reason)` → `remove`. Turn berhenti.
- **Kedaluwarsa** — `topic.rs:417-419`: `session.end(EndReason::KeyExpired)` → `sessions.remove(...)`. **Tanpa `abort_turn`.** `end()` hanya membatalkan utterance user (`session.rs:219-221`), tidak mengirim `Command("abort")`; itu pekerjaan `abort_turn` (`session.rs:246-262`).

Akibatnya persis yang diperingatkan `session.rs:236-238`: agen tetap bekerja, jawabannya tetap ditulis dan tetap masuk riwayat, padahal sesi sudah `ended`. FR-063 menuntut tiga: alasan, mic dilepas, **turn dibatalkan** — jalur kedaluwarsa memenuhi dua.

**Perbaikan yang diminta:** satu tempat. Jalur kedaluwarsa di `topic.rs` memakai urutan yang sama dengan `end_owned_by_key` — `abort_turn` dulu, lalu `end`, lalu `remove`. Tidak ada API baru, tidak ada tipe baru. Kalau `topic.rs` tidak punya akses ke `abort_turn` yang async di titik itu, pindahkan urutannya ke satu method registry (`end_one(&session, reason)`) supaya dua jalur memakai satu fungsi — **jangan** menyalin tiga baris itu.

**DoD (perintah + keluaran yang harus kelihatan):**

| # | Perintah | Output harapan |
|---|---|---|
| 1 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running ≥5 tests`, semua lulus — jalur cabut tidak rusak |
| 2 | Tes **baru** di modul yang sama: sesi dengan turn berjalan, lalu alasan `KeyExpired` | meng-assert **tiga hal**: `ended` diberitakan dengan `key_expired`; `busy` kembali `false`; **`Command("abort")` benar-benar terkirim** ke transport (tangkap lewat transport uji — mekanismenya sudah ada di `session_on_transport`) |
| 3 | `cargo test -p vizier-core --features provider-live-voice` (crate berfitur) | `0 failed`; sebut jumlah `passed` apa adanya |
| 4 | `cd webui && npm test -- session` | tidak berubah: 65 passed / 1 skipped |

**Batas:** tes #2 harus gagal di kode sekarang dan lulus sesudahnya — kalau ia lulus sebelum perbaikan, tesnya salah tempat, bukan kodenya benar. Lampirkan **dua** keluaran itu di laporan.

**WIP:** `T110` kembali ke `aktif` (ronde 1 dari maks 2). Ini ronde yang sama, bukan task baru — tidak menambah slot.


## Kode ronde 1 di working tree — penulis tidak diketahui (10 Nov 2026 11:05)

`git status` 11:00 menunjukkan perubahan **belum commit** yang isinya persis DoD ronde 1, filenya milik T110:

| File | mtime | Isi |
|---|---|---|
| `crates/vizier-core/src/live_voice/session.rs` | 10 Nov 09:53 | method baru `end_one()` (abort → end → remove) + tes `a_lapsed_key_cancels_the_turn_it_was_running` |
| `crates/vizier-http/src/topic.rs` | 10 Nov 09:54 | jalur kedaluwarsa memanggil `sessions.end_one(..., KeyExpired)` — tiga baris lama dihapus |

**Yang sudah saya buktikan (11:02):** `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` → `running 6 tests`, **6 passed / 0 failed**, exit 0 — termasuk tes baru yang meng-assert `busy` kembali `false` dan `Command("abort")` benar-benar terkirim lewat transport uji.

**Yang belum:** siapa penulisnya. Git tidak bisa membedakan agen (author identik), dan saya **tidak** menyatakan "bukan saya" tanpa bukti. Bobot bukti saat ini: (a) jeda 09:53 hanya **6 menit** setelah commit saya `28ed1668` (09:47); (b) alibi `@sepuh-be` (10:43) menyebut sesinya terpotong **21:45 semalam** dan tidak menyebut jendela 09:40–10:00 — jadi klaimnya belum menutup jendela itu. Saya tanyakan satu hal tertutup ke dia sebelum menyimpulkan.

**Batas gate:** tes baru memakai API yang belum ada di HEAD, jadi "harus gagal sebelum perbaikan" terpenuhi secara kompilasi, **bukan** sebagai bukti perilaku. Bukti perilaku sesungguhnya ada di `topic.rs`: jalur kedaluwarsa tidak lagi memanggil `end()` sendiri. Itu yang harus dibaca reviewer, bukan exit code-nya.


## Gate teknis — T110 ronde 1 (10 Nov 2026 11:15–11:20)

Kode: **`30500bf5`** — `crates/vizier-core/src/live_voice/session.rs` + `crates/vizier-http/src/topic.rs`, 2 file, +140/−9. `git status --short crates/` **kosong** sesudah commit *dan* sesudah gate, jadi gate dijalankan pada tree yang di-commit, bukan working copy.

| # | Perintah | Keluaran |
|---|---|---|
| 1 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 6 tests` · **6 passed / 0 failed**, exit 0 — termasuk `a_lapsed_key_cancels_the_turn_it_was_running` |
| 2 | `cargo check -p vizier-core` | `Finished`, exit 0 — build tanpa fitur tidak melihat kode ini (FR-041) |
| 3 | `cargo check -p vizier-core --features provider-live-voice` | `Finished`, exit 0 |
| 4 | `cargo test -p vizier-http --features live-voice --test live_voice_session` | `running 7 tests` · 7 passed / 0 failed, exit 0 — jalur socket yang ada tidak rusak |

**Batas bukti, ditulis dengan kata-kata `@sepuh-be`:** tes baru **merah karena kompilasi, bukan karena assertion** — `end_one` belum ada di HEAD, jadi "test fails before the fix" di sini berarti "tidak bisa dikompilasi sebelum perbaikan". Kalau kalimat itu dikutip tanpa batasnya, ia jadi bentuk lain dari hijau palsu. Bukti perilakunya ada di `topic.rs`: tiga baris `end()` + `remove()` hilang, diganti satu panggilan `end_one`.

**Atribusi:** penulis kode tidak bisa dipastikan dari git (author identik antar-agen). Yang terbukti: isinya cocok dengan DoD yang saya tulis 09:40, mtime 09:53 = 6 menit setelah commit saya `28ed1668` (09:47), dan `@sepuh-be` menyatakan tidak punya sesi di jendela itu. Saya gate dan commit atas dasar itu, dan mencatatnya sebagai **cacat proses** — kode semestinya ditulis `@sepuh-be`, bukan Lead.

**Yang belum dibuktikan:** SC-013 "within 1 s" (butuh uji waktu dengan server hidup) dan **`GI-07`** (tidak ada tes yang melewati socket). Keduanya wajib ikut di laporan gate, bukan dirapikan.


## DoD — T110 ronde 2 (GI-07): jalur socket diuji lewat socket, bukan lewat registry

**Kenapa ronde 2 dan bukan baris baru:** `GI-07` lahir dari T110 dan QA tidak boleh menandatangani T110 selama lubangnya terbuka. Satu slot (`WIP=1`), satu `R: FR-063`. `GI-07` di papan tetap ada sebagai jejak, statusnya **dilebur ke ronde ini**.

**Opsi A/B yang diajukan `@sepuh-be` tidak perlu dipilih — ada opsi C, dan biayanya nol perubahan produksi.** Buktinya, ketiganya saya periksa sendiri:

| Fakta | Bukti |
|---|---|
| `HTTPState` tidak bisa disebut dari `tests/` | `crates/vizier-http/src/lib.rs:26` → `mod state;` (privat ke crate) |
| Tapi **bisa** disebut dari tes di dalam crate | `crates/vizier-http/src/api/v1/catalogue.rs:249-…` membangun `HTTPState { … }` apa adanya; `new_state()` juga `pub(crate)` (`lib.rs:63`) |
| Tes in-crate async sudah jadi kebiasaan di crate ini | `#[tokio::test]` di `api/v1/catalogue.rs:282`, `notifications.rs:393`, `health.rs` |
| `Principal` + `AuthMethod::ApiKey { expires_at }` bisa dibangun tes | `crates/vizier-core/src/auth/mod.rs:52-57`, `:15-32`; contoh tangan: `topic.rs:93-99` |
| Sesi bisa disuntik ke registry proses | `vizier_core::live_voice::sessions()` (`live_voice/mod.rs:45`), `set_key_origin` (`session.rs:157`), `run_turn` (`:512`) |

**Keputusan saya: tes diletakkan di `crates/vizier-http/src/topic.rs`, `#[cfg(test)] mod tests`** — modul yang sudah ada, bukan file baru. Tidak ada perubahan perilaku produksi. **Kalau ternyata mustahil tanpa menyentuh kode produksi (visibility/refactor): berhenti dan lapor, jangan ubah visibility sendiri** — itu keputusan gate, bukan langkah kerja.

**Yang harus dibuktikan** — dua tes baru, nama persis ini:

1. `a_lapsed_key_at_the_socket_cancels_the_turn_it_was_running`
   - `TopicConnection` dengan principal `AuthMethod::ApiKey { key_id: Some(..), expires_at: Some(masa lalu), .. }`.
   - Sesi di registry untuk conversation yang sama, `key_origin` = kunci itu, **turn berjalan** (`run_turn` dengan engine yang tidak bersuara, pola `SilentVoice` di `session.rs` ronde 1).
   - Satu frame masuk lewat `handle_live_voice` (mis. `live_voice_audio`).
   - Assert **lima**: `state() == Ended`; alasan yang dipublikasikan `key_expired`; `busy == false`; `command:abort` terkirim **tepat 1**; registry tidak lagi memuat sesi itu.
2. `a_session_a_person_opened_survives_the_same_call`
   - Sesi yang sama tapi `key_origin == None`, principal yang sama (kunci kedaluwarsa).
   - Assert: sesi **tetap** hidup dan tidak ada `abort` — ini yang membedakan "semua sesi dipangkas" dari "sesi milik kunci itu".


**Catatan urutan yang harus dibaca sebelum menulis tes** (saya periksa sendiri, supaya tidak ada kejutan di tengah):
- Cabang kedaluwarsa ada **sebelum** `match action` dan **tidak** `return` (`topic.rs:415-425`). Setelah `end_one`, frame yang sama tetap masuk ke arm-nya: `ClientAction::Audio` langsung `return` karena `sessions.get(...)` sudah kosong (`:553-556`). Jadi tidak ada frame liar — tapi assertion-nya harus pada **state sesi + abort + registry**, bukan pada "tidak ada frame lain".
- Tes negatif jangan menuntut frame audio berhasil diproses: yang di-assert cuma **sesi tidak berhenti** dan **tidak ada `abort`**.

**Perintah + keluaran harapan:**

| # | Perintah | Output harapan |
|---|---|---|
| 1 | `cargo test -p vizier-http --features live-voice --lib topic::tests` | `running 6 tests` · **6 passed / 0 failed** · nama kedua tes baru muncul di daftar. Baseline hari ini **4 passed** — angka 6 wajib, bukan "ok" saja |
| 2 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 6 tests` · 6 passed (ronde 1 tidak rusak) |
| 3 | `cargo check -p vizier-http --features live-voice` | `Finished`, exit 0 — tidak ada kode produksi yang berubah |
| 4 | `git status --short crates/` | **kosong** sesudah commit, sebelum dan sesudah gate dijalankan |

**Batas yang wajib ikut di laporan:** tes ini membuktikan jalur socket untuk **kedaluwarsa**. Ia **tidak** membuktikan `SC-013` "within 1 s" (butuh uji waktu dengan server hidup, `GI-01`), dan **tidak** membuktikan `GI-05` (pencabutan tidak dicek per frame) — tapi harness yang sama membuat `GI-05` murah sesudah ini: satu tes tambahan "cabut kunci → `live_voice_start` lagi di socket yang sama". Kalau lu bisa menutup keduanya dalam satu file, sebut di laporan; kalau tidak, `GI-05` tetap terbuka dan itu tidak menghalangi ronde ini.

**WIP:** `T110` tetap satu-satunya slot lu. Ronde ini ronde ke-2 dari maksimum 2 — kalau setelah ini masih ada blocker, ronde ke-3 bukan ronde lagi, tapi eskalasi ke `@bro-pm`.


## Gate teknis — T110 ronde 2 (10 Nov 2026 11:48)

Kode: **`093c2b92`** — `crates/vizier-http/src/topic.rs` saja, +307/−1. Satu-satunya baris yang dihapus adalah `use` di dalam `mod tests`; **nol perubahan kode produksi** (dua hunk: `@@ -85` perluasan `use`, `@@ -234` isi tes).

**Diukur ulang oleh saya, bukan diterima dari laporan:**

| # | Perintah | Keluaran |
|---|---|---|
| 1 | `cargo test -p vizier-http --features live-voice --lib topic::tests` | `running 6 tests` · **6 passed / 0 failed** · kedua nama baru muncul (`a_lapsed_key_at_the_socket_cancels_the_turn_it_was_running`, `a_session_a_person_opened_survives_the_same_call`) |
| 2 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 6 tests` · 6 passed — ronde 1 utuh |
| 3 | `cargo check -p vizier-http --features live-voice` | `Finished`, exit 0 |
| 4 | `git status --short crates/` | kosong, sebelum dan sesudah gate |

**"Gagal sebelum perbaikan" saya reproduksi sendiri, bukan dikutip.** Cabang kedaluwarsa saya kembalikan sementara ke bentuk pra-`30500bf5` (`session.end()` + `sessions.remove()`, tanpa `end_one`), lalu tesnya dijalankan:

```
test topic::tests::a_lapsed_key_at_the_socket_cancels_the_turn_it_was_running ... FAILED
panicked at crates/vizier-http/src/topic.rs:491:
assertion `left == right` failed: the running turn is cancelled by exactly one abort, got []
  left: 0   right: 1
```

Gagal di **assertion perilaku**, bukan kompilasi — persis yang `GI-07` minta. File dipulihkan dengan `git checkout --`, md5 **identik** (`5db9cd91…`) dan `git status` bersih sesudahnya.

**Satu assert lebih lemah dari DoD, dan saya putuskan: diterima.** DoD meminta literal `busy == false`; `busy` privat di `vizier-core` (`session.rs:71`). Yang dipakai sebagai gantinya: `run_turn` menerbitkan `Waiting` sebelum frame (di-assert), dan `abort_turn` **hanya** mengirim `Command("abort")` kalau flag menyala lalu menghapusnya di swap yang sama (`session.rs:250-251`) — jadi "tepat satu abort" membuktikan turn tadi jalan dan sekarang tidak. Saya terima karena bukti perilaku lebih kuat dari bacaan flag.

**Koreksi untuk catatan:** pilihannya **tidak** biner "accessor produksi atau tidak sama sekali". Ada jalan ketiga tanpa menyentuh produksi — panggil `abort_turn` sekali lagi sesudah frame (API publik, `session.rs:246`) dan assert **tidak ada** abort kedua; itu membuktikan `busy == false` secara literal. Harganya satu assert. Dicatat sebagai **minor, tidak memblokir**; kalau `GI-05` menyentuh harness yang sama, tambahkan di situ.

## Keputusan rotasi kunci (FR-063), 10 Nov 2026 09:20

**Temuan `@teteh-qa`:** `grep -rn rotate crates/vizier-http/src/api/v1` → **0 hit**. FR-063 menyebut kunci "can be rotated", tapi tidak ada route rotasi.

**Keputusan saya, dengan buktinya:** rotasi **tidak butuh route sendiri** — ia komposisi dua route yang sudah ada: `create_api_key` (`crates/vizier-http/src/api/v1/auth.rs:294`) dan `delete_api_key` (`:404`). Kontrak sudah menuliskannya begitu: *"key rotated (new key, old one retired) → the old key stops working; a session opened with it ends | reason `key_revoked`"* (`contracts/live-voice-frames.md:362`). Jadi requirement punya pembawa; yang kurang hanya **satu baris di `spec.md`** yang menyebut "rotate = buat kunci baru + cabut yang lama, dengan akibat yang sama seperti revoke". Itu semantik spec → `@system-analyst`, bukan saya.

**Yang tidak saya putuskan:** apakah rotasi harus punya endpoint atomik sendiri (supaya tidak ada jendela dua kunci aktif). Kalau kang mau itu, ia requirement baru, bukan penafsiran FR-063.
**Catatan 10 Nov 11:05:** laporan `@sepuh-be` 10:43 masih memakai DoD versi 9 Nov (#3 `end_owned_by_key`, #4 `cargo test -p vizier-core`). Keduanya **sudah diganti di bagian ini jam 09:15**, sebelum laporannya masuk. Yang perlu ditambahkan hanya akar masalah #4, di `GI-06`.

## DoD — T110 (v2, 10 Nov 2026 09:15 — DoD #3 lama diganti karena **gate hijau palsu**)

**Kenapa diganti:** `cargo test -p vizier-core --features provider-live-voice end_owned_by_key` keluar dengan **exit 0 sambil menjalankan nol tes** — `end_owned_by_key` adalah nama *method* (`session.rs:519`), bukan nama tes, jadi filter mencocokkan nol fungsi dan Rust tetap melaporkan `test result: ok`. Diukur ulang: **7 target, 7x `running 0 tests`, exit 0.** Temuan `@teteh-qa` 9 Nov 21:45; saya reproduksi sendiri 10 Nov.

> **Aturan baru (berlaku semua DoD):** perintah `cargo test <filter>` yang menjalankan **0 tes tetap exit 0**. DoD wajib menyebut **jumlah tes yang harus jalan** (`running N tests`, N > 0) atau memakai path modul lengkap. Exit code saja bukan bukti.

| # | Perintah | Output harapan | Diukur 10 Nov 09:15 |
|---|---|---|---|
| 1 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 5 tests` · 5 lulus · 0 gagal | ✅ **5 passed / 0 failed**, 1.33 s, exit 0 |
| 2 | `cargo check -p vizier-core` (tanpa fitur) | `Finished`, exit 0 — FR-041: build tanpa `provider-live-voice` tetap jalan | ✅ exit 0, 44,85 s |
| 3 | `cargo test -p vizier-core --lib live_voice` (tanpa fitur) | `running 6 tests` — **6 `schema::agent`/`service::agents::live_voice_input_tests`, 0 dari modul `live_voice`**. Angka 6 tidak boleh dibaca sebagai "modul live_voice lulus tanpa fitur" | ✅ 6 passed / 0 failed, 0 dari modul `live_voice` |
| 4 | `cd webui && npm test -- session` | `key_revoked` beda kalimat dari `key_expired` dan dari `pressed_again` | ✅ diukur `@teteh-qa`: 2 file, **65 passed / 1 skipped**, 922 ms |

**Yang DoD ini tetap belum buktikan — wajib disebut di laporan gate:**
- **SC-013 "within 1 s"** — belum ada alat. Tes #1 membuktikan jalur, alasan, dan pembersihan registry; tidak ada yang mengukur jeda nyata dari HTTP revoke sampai frame sampai ke klien. Butuh uji waktu dengan server hidup.
- **"Turn in flight is cancelled" tidak di-assert.** Kode memanggil `abort_turn` sebelum `end` (`session.rs:541-542`), tapi lima tes `key_tests` hanya menyatakan jumlah, alasan, state, dan isi registry.
- **Sisi klien diuji lewat jalur generik**, bukan lewat alasan kunci (`session.test.ts:692` `provider_failed`, `:702` `idle`). Jalurnya sama, tapi "sama" itu inferensi.

**Batas gate yang lebar, dan bukan milik fitur ini:** `cargo test -p vizier-core` (seluruh crate, tanpa fitur) = **1733 lulus, 8 gagal**; kedelapan kegagalan semuanya `projects::tools::lsp_tests` + `service::project_lsp` ("the server never became ready", 20 s), **repro sendirian** setelah `cargo build -p vizier-core --example fake_lsp`. Itu fitur 054, bukan 055 — makanya DoD #2/#3 di atas dipersempit, bukan gate dibiarkan merah karena warisan fitur lain.

**Pemilik:** gate teknis saya **✅ 10 Nov** (baris #1–#3) · tanda tangan perilaku `@teteh-qa` (task perilaku → `qa-plan` 4 skenario sudah ditulis QA; belum tanda tangan).

## Q12 dan T111 — apa yang dijawab kang 10 Nov 09:37, dan apa yang **tidak** berubah

Kang menjawab item 4 (*"Boleh deh"*). Ini yang berubah dan yang tidak:

- **Berubah:** arah *"jawaban datang sambil dibuat"* (level 1, FR-065…FR-070) **dikonfirmasi kedua kalinya** oleh User — yang pertama 10 Nov 06:39 (*"level 1 dulu aja deh bro, tapi kalo pas gw tes terus latency nya masih kerasa ganggu baru gw minta level 2"*, sudah tercatat di `spec.md:691` pada `ef846fbd`). Konfirmasi kedua ini tidak menambah syarat, ia mengulang yang sudah ada.
- **Tidak berubah:** `T111` tetap `[PROVISIONAL]`. Label itu **bukan soal izin** — izinnya sudah ada sejak 06:39 — melainkan soal **angka**: belum ada pengukuran yang mengatakan berapa yang dibeli. Spec menulisnya sendiri: *"until those numbers are re-measured against a real turn, a latency target would be a number invented to look finished"* (`spec.md:263-267`).
- **Karena itu, yang mengubah T111 jadi bisa dijadwalkan bukan "boleh", tapi tiga hal ini:** (1) asesmen saya dengan angka yang diukur ulang pada satu turn nyata; (2) DoD tertulis; (3) slot WIP `@sepuh-be` kosong — sekarang tidak, karena `T110` di `gate` dan `GI-01`/`GI-02` di `antre`.

**Kalau yang kang maksud "Boleh" adalah *mulai bangun sekarang*:** itu keputusan prioritas dan sah — tapi tetap butuh DoD lebih dulu, dan tetap antre di belakang T110. Sebut satu baris, dan saya tulis brief-nya hari ini.
