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
api_contract: — (endpoint @ versi; pemilik: data)
freshness:    — (realtime | cached TTL)
p95_budget:   — ms (ambang kapan skeleton/transisi muncul jadi angka, bukan selera)
```

## Tabel

14 baris. Kolom QA diisi oleh QA dan ditempel apa adanya; kolom desain dan data masih `open`.

| ID | state | dipicu kapan | digambar apa | butuh data apa | lulusnya kayak apa |
|---|---|---|---|---|---|
| LV-ST-IDLE | `off` — pill | belum ada sesi. **State key di kode = `off`**, bukan `idle` (`webui/app/lib/liveVoice/session.ts:97`); `rowWord()` tidak punya cabang `idle`, dan pill dirender oleh cabang `view.state === 'off'` (`LiveVoiceRow.tsx:189`). Tidak ada `data-state` di pill. | open | open | FR-001 — Buka percakapan, jangan tekan apa pun: pill `Live voice` tinggi 24 px, tak ada `data-state`, nol baris tambahan; refresh 3x tetap mati. |
| LV-ST-UNAVAILABLE | cabang `!capability` — **bukan state baris** | salah satu dari `recognizer`/`voice` tidak ada untuk agent itu (FR-004). Bukan state sesi: cabang ini merender pill **non-tombol**, tidak pernah menulis `data-state` (`LiveVoiceRow.tsx:169-186`). | open | open | FR-004 — Agent `capability:false`: sesi tidak menyala, `mic off`, menyebut yang kurang **dan cara memasangnya**. Hari ini hanya `title` + `sr-only` (isi: "a voice is not configured..."), nol jalan perbaikan: baris ini **gagal apa adanya**. Lihat temuan O2. |
| LV-ST-CONNECTING | `connecting` | sesi dimulai, transport belum siap | open | open | FR-006 — Tekan kontrol: kata `Connecting…`, `data-state="connecting"`, `End` sudah bisa ditekan, tanpa reload. |
| LV-ST-WAITING | `waiting` (label `Thinking`) | status `waiting`, agent sedang bekerja (FR-042) | open | open | FR-042 — Tanya yang butuh lookup: `Thinking` + `•••` (aria-hidden, tetap ada saat reduced motion), warna beda dari `reconnecting`, diumumkan **sekali** lewat `role="status"` (`:214-216`); hilang saat segmen pertama terdengar. |
| LV-ST-LISTENING | `listening` | audio user masuk | open | open | FR-010/011 — Bicara lalu diam 800 ms: transkrip jadi pesan user bertanda spoken; baris kembali `Listening`, `mic on`, detik jalan. |
| LV-ST-SPEAKING | `speaking` | audio agent keluar | open | open | FR-012/015 — Agent menjawab: teks muncul di percakapan **bukan per karakter**, audio = teks; habis: kembali `Listening` tanpa menekan apa pun. |
| LV-ST-INTERRUPTED | `interrupted` | user memotong agent di tengah | open | open | FR-022/023/045 — Potong saat bicara: audio stop seketika, nol sisa kalimat diucapkan, pesan tetap ada bertanda interrupted; **tak ada tombol interrupt** di layar (C7). |
| LV-ST-RECONNECTING | `reconnecting` | transport putus, sesi masih dianggap hidup | open | open | FR-033 — Matikan jaringan: **≤2 detik** `Reconnecting` + `•` + amber (`text-retrying-text`), audio berhenti; pulih <30 detik: sesi sama lanjut tanpa pesan ganda; lewat 30 detik (`RECONNECT_WINDOW_MS`): `ended` beralasan. |
| LV-ST-ENDED | `ended` | sesi ditutup normal, aksi `Start again` (`:265`) | open | open | FR-029/034/046 — Tekan `End` (`:275`): mic dilepas, baris balik jadi pill, percakapan identik (jumlah/urutan/teks); diam 3 menit: `ended` **dengan alasan yang terlihat**, bukan hilang senyap. |
| LV-ST-FAILED | `failed` | sesi gagal permanen, aksi `Try again` (`:265`) | open | open | FR-030 — Tolak izin mic: `Failed` + alasan menyebut penyebab, tidak setengah-mulai, `Try again` berhasil tanpa reload. |
| LV-ST-OVERFLOW | `overflow` (geometri, bukan state sesi) | pane menyempit di bawah kebutuhan baris. Urutan menyerah **hanya satu ini**: (1) chip model menyusut ke **96 px** lalu hilang, dipotong di segmen terakhir; (2) waveform **39 bar → 12** lalu hilang, jarak antar-bar tidak pernah <6 px; (3) context ring hilang utuh. _(FR-049, `spec.md:570`; di permukaan B1-1 ring sudah diserahkan ke state word, jadi jalannya mulai dari chip.)_ | open | open | FR-049 — Sempitkan pane bertahap sampai **240 px**: chip model 96 px lalu hilang → waveform 39→12 lalu hilang → context ring hilang utuh; state word/mic/detik/End tidak menyusut, tidak wrap, **tanpa elipsis**; tinggi tetap 24 px. |
| LV-ST-NO-SPEECH | `no-speech` | bicara tidak terdengar jelas (FR-031). **Menggantikan usul `error-validation`** — lihat O1. | open | open | FR-031 — Bicara tak terdengar jelas: user diberi tahu, **tak ada pesan terkirim**, sesi kembali `Listening`. **422 = N/A + alasan**: tak ada input yang bisa dibetulkan user di baris ini; `error-validation` di sini bakal jadi kolom kosong yang dipaksa. |
| LV-ST-ERROR-FORBIDDEN | `error-forbidden` (403) | izin ditolak / tidak ada device saat mulai (FR-030) | open | open | FR-030 — Izin ditolak / tak ada device: `Failed` + cara memperbaiki + retry tanpa reload. **`indistinguishable` dengan FAILED** sampai alasan keduanya dibedakan di kolom data. |
| LV-ST-ERROR-TRANSIENT | `error-transient` (5xx/timeout) | 5xx atau timeout saat mulai; putus melewati `RECONNECT_WINDOW_MS` (FR-032/033) | open | open | FR-033/032 — 5xx/timeout saat mulai: `Failed` beralasan, retry tidak menggandakan pesan; putus >30 detik: `ended` beralasan. **`indistinguishable` dengan FAILED/ENDED** sampai ada pembeda. |

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
- **O2 — FR-004 minta "how to set it", kode tidak memberikannya.** `LiveVoiceRow.tsx:169-186` hanya
  menyebut *apa* yang kurang (`a voice is not configured for this agent`); komentar di `:170` justru
  mengklaim "and where the settings are". Selisih kode vs requirement, **temuan baca kode, belum
  diuji di layar.** **Pemilik: @mamang-lead-be** — jalurnya lewat Lead, bukan diputuskan QA atau SA.
- **O3 — `design_ref` belum ada.** Kolom desain 0/14. Selama ini kosong, dokumen ini belum boleh
  dipakai menyatakan "gagal".

## Status (2026-11-10 08:58 WIB)

- baris: **14** · kolom QA terisi: **14** · kolom data terisi: **0** · kolom desain terisi: **0**
- pemilik pengisian: desain (kol 3, terhadap permukaan satu-baris) · data (kol 4) · QA (kol 5, sudah).
- sumber angka: `spec.md@ef846fbd` (FR-004 `:505`, FR-049 `:570`), kode di `LiveVoiceRow.tsx`
  (`ROW` `:32`, cabang capability `:169`, `role="status"` `:214`, aksi `:265`/`:275`).
