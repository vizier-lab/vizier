# State matrix — Live Voice 055

Lampiran `specs/055-live-voice/spec.md`. Satu tabel, tiga kolom terisi oleh tiga meja.
Dokumen ini **bukan** checklist uji selama `design_ref` belum ber-versi (aturan titik nol).

```
flow:         Live Voice — panggilan sebagai satu baris di dalam percakapan (C11 / surface B1-1)
spec_ref:     specs/055-live-voice/spec.md@ef846fbd
design_ref:   — (pemilik: desain; wajib frame/file@versi sebelum dokumen ini dipakai menguji)
approved:     — (tanggal + siapa)
viewport:     375 / 768 / 1440
PIC:          desain = kolom "digambar apa" · data = kolom "butuh data apa" · QA = kolom "lulusnya kayak apa"
api_contract: WS `/api/v1/agents/{channel_id}/topic/{topic_id}/chat` (`crates/vizier-http/src/api/v1/agents/channel.rs:52`); frame `state` / `speech` / `error` / `stall` / `transcript` @ `frame.rs` `b6cc0b40` (pemilik: data)
freshness:    realtime — push per frame, tanpa cache klien (T063: state lama tidak diingat)
p95_budget:   — ms (ambang kapan skeleton/transisi muncul jadi angka, bukan selera) open → PIC Lead
```

## Tabel

14 baris. Kolom QA dan kolom data terisi; kolom desain masih `open`. Kolom data diisi sepuh-be 2026-11-10 dari kode @ `b6cc0b40`.

| ID | state | dipicu kapan | digambar apa | butuh data apa | lulusnya kayak apa |
|---|---|---|---|---|---|
| LV-ST-IDLE | `off` — pill | belum ada sesi. **State key di kode = `off`**, bukan `idle` (`webui/app/lib/liveVoice/session.ts:97`); `rowWord()` tidak punya cabang `idle`, dan pill dirender oleh cabang `view.state === 'off'` (`LiveVoiceRow.tsx:189`). Tidak ada `data-state` di pill. | open | Agent detail `live_voice` (dua half: `recognizer`, `voice`) → props `capability` + `missing` (`LiveVoiceRow.tsx:20-22`). **Nol frame**: pill tidak pernah membaca socket. Ada. | FR-001 — Buka percakapan, jangan tekan apa pun: pill `Live voice` tinggi 24 px, tak ada `data-state`, nol baris tambahan; refresh 3x tetap mati. |
| LV-ST-UNAVAILABLE | cabang `!capability` — **bukan state baris** | salah satu dari `recognizer`/`voice` tidak ada untuk agent itu (FR-004). Bukan state sesi: cabang ini merender pill **non-tombol**, tidak pernah menulis `data-state` (`LiveVoiceRow.tsx:169-186`). | open | `missing: 'recognizer'` / `'voice'` untuk memilih kalimat (`LiveVoiceRow.tsx:20-22`, `humanReason` `session.ts:540-586`). **Yang tidak ada: cara memasangnya** — payload agent detail tidak membawa nama setting/alamat halaman; FR-004 minta itu. open → PIC data + Lead (O2). | FR-004 — Agent `capability:false`: sesi tidak menyala, `mic off`, menyebut yang kurang **dan cara memasangnya**. Hari ini hanya `title` + `sr-only` (isi: "a voice is not configured..."), nol jalan perbaikan: baris ini **gagal apa adanya**. Lihat temuan O2. |
| LV-ST-CONNECTING | `connecting` | sesi dimulai, transport belum siap | open | Frame `state { state, generation }` (`frame.rs:35-46`). **Batas waktu tidak ada**: klien hanya punya `RECONNECT_WINDOW_MS` (`session.ts:82`); kalau server diam, `Connecting…` menggantung tanpa ambang. open → PIC data + Lead (budget). | FR-006 — Tekan kontrol: kata `Connecting…`, `data-state="connecting"`, `End` sudah bisa ditekan, tanpa reload. |
| LV-ST-WAITING | `waiting` (label `Thinking`) | status `waiting`, agent sedang bekerja (FR-042) | open | `thinking`, `thinkingMs`, `elapsedMs` (`session.ts:27-46`); server `set_state(Waiting)` lewat `thinking()` (`session.rs:199-201`); filler 400 ms `a03004fc` (G2) mengisi jeda pertama. Ada. | FR-042 — Tanya yang butuh lookup: `Thinking` + `•••` (aria-hidden, tetap ada saat reduced motion), warna beda dari `reconnecting`, diumumkan **sekali** lewat `role="status"` (`:214-216`); hilang saat segmen pertama terdengar. |
| LV-ST-LISTENING | `listening` | audio user masuk | open | `micOpen`, `transcript` + flag `submitted` (`session.ts:249-255`), `level` 0..1. **`level` tanpa penulis**: `session.ts:373` menulis tetap 0 walau `onWindow` sudah punya `decision.voiced` → meter tanpa data. open → PIC data. | FR-010/011 — Bicara lalu diam 800 ms: transkrip jadi pesan user bertanda spoken; baris kembali `Listening`, `mic on`, detik jalan. |
| LV-ST-SPEAKING | `speaking` | audio agent keluar | open | `speech { generation, seq, sample_rate, format, base64, final }` (`frame.rs:166-178`); `stall { dropped }` → notice; latensi pertama terdengar lewat `markFirstAudible(queued.audibleInMs)`. Ada. | FR-012/015 — Agent menjawab: teks muncul di percakapan **bukan per karakter**, audio = teks; habis: kembali `Listening` tanpa menekan apa pun. |
| LV-ST-INTERRUPTED | `interrupted` | user memotong agent di tengah | open | **Tidak ada produsen.** `abort_turn` langsung ke `Listening` (`session.rs:271-273`); di `crates/` tidak ada `set_state(SessionState::Interrupted)`, satu-satunya rujukan = tes `frame.rs:413`. Pilihan: frame `interrupted` baru, atau state klien berdurasi. open → PIC Lead + desain. | FR-022/023/045 — Potong saat bicara: audio stop seketika, nol sisa kalimat diucapkan, pesan tetap ada bertanda interrupted; **tak ada tombol interrupt** di layar (C7). |
| LV-ST-RECONNECTING | `reconnecting` | transport putus, sesi masih dianggap hidup | open | `onSocketState` + `RECONNECT_WINDOW_MS = 30_000` (`session.ts:82`), alasan `connection_lost` → notice. Sisa waktu tidak ada di wire. Ada (tanpa hitungan mundur). | FR-033 — Matikan jaringan: **≤2 detik** `Reconnecting` + `•` + amber (`text-retrying-text`), audio berhenti; pulih <30 detik: sesi sama lanjut tanpa pesan ganda; lewat 30 detik (`RECONNECT_WINDOW_MS`): `ended` beralasan. |
| LV-ST-ENDED | `ended` | sesi ditutup normal, aksi `Start again` (`:265`) | open | `state { ended, reason }`; 17 nilai `EndReason` (`frame.rs:73-100`) → satu kalimat per alasan (`humanReason`, `session.ts:540-586`). Ada. | FR-029/034/046 — Tekan `End` (`:275`): mic dilepas, baris balik jadi pill, percakapan identik (jumlah/urutan/teks); diam 3 menit: `ended` **dengan alasan yang terlihat**, bukan hilang senyap. |
| LV-ST-FAILED | `failed` | sesi gagal permanen, aksi `Try again` (`:265`) | open | `error { kind, message }` + `state { failed, reason }`. `kind` memakai kosakata `EndReason`, **bukan** kelas HTTP — transport WS (C4). Ada. | FR-030 — Tolak izin mic: `Failed` + alasan menyebut penyebab, tidak setengah-mulai, `Try again` berhasil tanpa reload. |
| LV-ST-OVERFLOW | `overflow` (geometri, bukan state sesi) | pane menyempit di bawah kebutuhan baris. Urutan menyerah **hanya satu ini**: (1) chip model menyusut ke **96 px** lalu hilang, dipotong di segmen terakhir; (2) waveform **39 bar → 12** lalu hilang, jarak antar-bar tidak pernah <6 px; (3) context ring hilang utuh. _(FR-049, `spec.md:570`; di permukaan B1-1 ring sudah diserahkan ke state word, jadi jalannya mulai dari chip.)_ | open | Nol data server: geometri murni `ROW`/`BAR` (`LiveVoiceRow.tsx:32-34`), kontrak terukur Lead. Ada. | FR-049 — Sempitkan pane bertahap sampai **240 px**: chip model 96 px lalu hilang → waveform 39→12 lalu hilang → context ring hilang utuh; state word/mic/detik/End tidak menyusut, tidak wrap, **tanpa elipsis**; tinggi tetap 24 px. |
| LV-ST-NO-SPEECH | `no-speech` | bicara tidak terdengar jelas (FR-031). **Menggantikan usul `error-validation`** — lihat O1. | open | `transcript { submitted: false }` + teks kosong → notice "that was too short to send" (`session.ts:250-253`); alasan `unintelligible` juga ada di `EndReason` (`frame.rs:73-100`). Dua jalur, satu kata: pilih satu, jangan dua-duanya. Ada; **yang perlu diputuskan: jalur mana** (open → PIC data). | FR-031 — Bicara tak terdengar jelas: user diberi tahu, **tak ada pesan terkirim**, sesi kembali `Listening`. **422 = N/A + alasan**: tak ada input yang bisa dibetulkan user di baris ini; `error-validation` di sini bakal jadi kolom kosong yang dipaksa. |
| LV-ST-ERROR-FORBIDDEN | `error-forbidden` (403) | izin ditolak / tidak ada device saat mulai (FR-030) | open | `error.kind` = `permission_withdrawn` / `no_device` + kalimat `humanReason` (`session.ts:540-586`). **Cara memperbaiki tidak ada di payload** (sama seperti O2) — tanpa itu `Try again` cuma mengulang penolakan. open → PIC Lead + data. | FR-030 — Izin ditolak / tak ada device: `Failed` + cara memperbaiki + retry tanpa reload. **`indistinguishable` dengan FAILED** sampai alasan keduanya dibedakan di kolom data. |
| LV-ST-ERROR-TRANSIENT | `error-transient` (5xx/timeout) | 5xx atau timeout saat mulai; putus melewati `RECONNECT_WINDOW_MS` (FR-032/033) | open | `error.kind` = `provider_failed` / `server_restart` / `connection_lost`; `end()` sudah idempoten (`session.rs:215-231`, alasan pertama menang) tapi **restart belum dibuktikan** idempoten dari sisi klien. open → PIC data + Lead. | FR-033/032 — 5xx/timeout saat mulai: `Failed` beralasan, retry tidak menggandakan pesan; putus >30 detik: `ended` beralasan. **`indistinguishable` dengan FAILED/ENDED** sampai ada pembeda. |

`state` diambil dari `rowWord()` + `data-state` di `LiveVoiceRow.tsx`. **Dua pengecualian yang sudah
dikoreksi QA dan diterima:** `off` (bukan `idle`) — dan `unavailable`, yang bukan state baris sama
sekali melainkan cabang `!capability` yang merender pill non-tombol. Menguji `data-state="idle"` atau
`"unavailable"` tidak akan pernah lulus, karena nilainya tidak pernah ditulis.

## Aturan isi

1. **ID** = `LV-ST-<STATE>`; satu baris = satu state = satu klaim yang bisa diuji.
2. **"lulusnya kayak apa"** wajib bisa diterjemahkan jadi *langkah + hasil yang diharapkan*. Kalau
   isinya "tampil dengan baik", baris itu belum selesai.
3. **`error` dipecah tiga**, bukan satu baris: `error-validation` (422), `error-forbidden` (403),
   `error-transient` (5xx/timeout, retry wajib idempoten). Untuk baris ini pecahan 422 masih
   diperdebatkan — lihat O1; barisnya `LV-ST-NO-SPEECH` sampai ada keputusan lain.
4. **`indistinguishable`** — kalau dua baris state dijawab response yang identik, tandai di kolom
   data. Itu temuan, bukan coretan: pilihannya menambah field di API atau menggabung state di UI,
   dan dua-duanya harus diputuskan sadar.
5. **Baris terbuka wajib punya pemilik** — `open -> PIC + tanggal`. Open question tanpa nama pemilik
   bukan open question, itu utang.
6. **Batas eksplorasi** — alur yang belum fixed: state bolong = open question, bukan defect.
7. **Titik nol** — selama `spec_ref`/`design_ref` belum ber-versi, dokumen ini draf; belum boleh
   dipakai menyatakan "gagal".

## Temuan terbuka

- **O1 — 422 di baris ini: N/A atau tidak?** QA: tak ada input user yang bisa dibetulkan, jadi
  `error-validation` kosong yang dipaksa, diganti `no-speech` (FR-031). Data (sepuh-be) mengusulkan
  pecahan 422/403/5xx sebagai aturan umum. **Pemilik: data (sepuh-be).** Baris `NO-SPEECH` tetap ada
  sampai keputusan itu; kalau 422 menang, tambah baris, jangan hapus yang ini.

  **Jawaban data (sepuh-be, 2026-11-10) — O1 ditutup dari sisi data:** 422 tidak eksis di 055.
  Transport-nya WS per-agent (C4), tidak ada response ber-status; kosakata kegagalannya `EndReason`,
  17 nilai (`frame.rs:73-100`), dan tiap nilai sudah punya kalimat (`humanReason`, `session.ts:540-586`).
  Jadi pecahan tiga kelas **bukan tiga baris tambahan**: ia sumbu *apa yang bisa dilakukan orangnya* —
  (a) ulangi sendiri: `unintelligible`, `no_device`, `unsupported_audio`; (b) butuh jalan keluar, retry
  pasti gagal: `key_revoked`, `key_expired`, `permission_withdrawn`, `no_recognizer`, `no_voice`;
  (c) layak dicoba lagi: `provider_failed`, `connection_lost`, `server_restart`. Baris `NO-SPEECH` tetap
  (ia state sesi), dan `FORBIDDEN`/`TRANSIENT` tetap karena visualnya beda. Yang belum ada: peta baku
  `reason → kelas` — itu keputusan pemilik `api_contract`. open → PIC Lead.
- **O2 — FR-004 minta "how to set it", kode tidak memberikannya.** `LiveVoiceRow.tsx:169-186` hanya
  menyebut *apa* yang kurang (`a voice is not configured for this agent`); komentar di `:170` justru
  mengklaim "and where the settings are". Selisih kode vs requirement, **temuan baca kode, belum
  diuji di layar.** **Pemilik: @mamang-lead-be** — jalurnya lewat Lead, bukan diputuskan QA atau SA.
- **O3 — `design_ref` belum ada.** Kolom desain 0/14. Selama ini kosong, dokumen ini belum boleh
  dipakai menyatakan "gagal".

## Status (2026-11-10 08:58 WIB)

- baris: **14** · kolom QA terisi: **14** · kolom data terisi: **14** · kolom desain terisi: **0**
- pemilik pengisian: desain (kol 3, terhadap permukaan satu-baris) · data (kol 4, sudah) · QA (kol 5, sudah).
- tiga hal yang **tidak** diisi tapi diputuskan: `level` tanpa penulis, `interrupted` tanpa produsen, `no-speech` punya dua jalur data.
- sumber angka: `spec.md@ef846fbd` (FR-004 `:505`, FR-049 `:570`), kode di `LiveVoiceRow.tsx`
  (`ROW` `:32`, cabang capability `:169`, `role="status"` `:214`, aksi `:265`/`:275`).
