# State matrix — Live Voice 055

Lampiran `specs/055-live-voice/spec.md`. Satu tabel, tiga kolom terisi oleh tiga meja.
Dokumen ini **bukan** checklist uji selama `spec_ref`/`design_ref` belum ber-versi (aturan titik nol).

```
flow:         Live Voice — panggilan sebagai satu baris di dalam percakapan (C11 / surface B1-1)
spec_ref:     specs/055-live-voice/spec.md@ef846fbd (isi yang sama saat dibaca di tree 9256036a)
design_ref:   — (pemilik: desain; wajib frame/file@versi sebelum dokumen ini dipakai menguji)
approved:     — (tanggal + siapa)
viewport:     375 / 768 / 1440
PIC:          desain = kolom "digambar apa" · data = kolom "butuh data apa" · QA = kolom "lulusnya kayak apa"
api_contract: — (endpoint @ versi; pemilik: data)
freshness:    — (realtime | cached TTL)
p95_budget:   — ms (ambang kapan skeleton/transisi muncul jadi angka, bukan selera)
```

## Tabel

| ID | state | dipicu kapan | digambar apa | butuh data apa | lulusnya kayak apa |
|---|---|---|---|---|---|
| LV-ST-IDLE | idle (pill) | live voice belum dinyalakan | open | open | open |
| LV-ST-UNAVAILABLE | unavailable | `capability: false` — salah satu dari `recognizer`/`voice` tidak ada (FR-004) | open | open | open |
| LV-ST-CONNECTING | connecting | sesi dimulai, transport belum siap | open | open | open |
| LV-ST-WAITING | waiting | state key `waiting` — kata tampil "Thinking" + 3 titik (FR-042) | open | open | open |
| LV-ST-LISTENING | listening | audio user masuk | open | open | open |
| LV-ST-SPEAKING | speaking | audio agent keluar | open | open | open |
| LV-ST-INTERRUPTED | interrupted | user memotong agent di tengah | open | open | open |
| LV-ST-RECONNECTING | reconnecting | transport putus, sesi masih dianggap hidup (amber, FR-042) | open | open | open |
| LV-ST-ENDED | ended | sesi ditutup normal, aksi `Start again` | open | open | open |
| LV-ST-FAILED | failed | sesi gagal permanen, aksi `Try again` | open | open | open |
| LV-ST-OVERFLOW | overflow | jendela menyempit: waveform 39 bar → 12 bar → hilang; item lain tidak menyusut | open | open | open |

`state` diambil dari `rowWord()` di `webui/app/screens/chat/LiveVoiceRow.tsx` (state key = `waiting`,
label = `Thinking`), bukan dari nama frame desain. Kolom 3-5 sengaja kosong: itu bukan wilayah
pemilik spek, dan mengisinya sendiri = mengarang kontrak.

## Aturan isi

1. **ID** = `LV-ST-<STATE>`; satu baris = satu state = satu klaim yang bisa diuji.
2. **"lulusnya kayak apa"** wajib bisa diterjemahkan jadi *langkah + hasil yang diharapkan*. Kalau
   isinya "tampil dengan baik", baris itu belum selesai.
3. **`error` dipecah tiga**, bukan satu baris: `error-validation` (422, user bisa betulkan sendiri),
   `error-forbidden` (403, user butuh jalan keluar), `error-transient` (5xx/timeout, retry wajib
   idempoten). Barisnya dibuat saat pemilik data mengisi, bukan didahulukan di sini.
4. **`indistinguishable`** — kalau dua baris state dijawab response yang identik, tandai di kolom
   data. Itu temuan, bukan coretan: pilihannya menambah field di API atau menggabung state di UI,
   dan dua-duanya harus diputuskan sadar.
5. **Baris terbuka wajib punya pemilik** — `open -> PIC + tanggal`. Open question tanpa nama pemilik
   bukan open question, itu utang.
6. **Batas eksplorasi** — alur yang belum fixed: state bolong = open question, bukan defect.
7. **Titik nol** — selama `spec_ref`/`design_ref` belum ber-versi, dokumen ini draf; belum boleh
   dipakai menyatakan "gagal".

## Status (2026-11-10)

- baris: **11** · kolom desain terisi: **0** · kolom data terisi: **0** · kolom QA terisi: **0**
- `design_ref` belum ada -> dokumen ini belum boleh dipakai menyatakan "gagal".
- pemilik pengisian: desain (kol 2, terhadap permukaan satu-baris) · data (kol 3) · QA (kol 4 + format).
