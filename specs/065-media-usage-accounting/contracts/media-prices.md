# Contract: the shipped media price table

File: `crates/vizier-core/assets/media-prices.json`, embedded in the binary, edited with releases.
Prices are USD and estimates (spec Assumptions).

```json
{
  "as_of": "2026-11-24",
  "entries": [
    {
      "provider": "openai",
      "model": "gpt-image-1",
      "kind": "image",
      "tokens": { "input": 5.0, "output": 40.0 },
      "per_image": {
        "default": 0.042,
        "variants": { "1024x1024/low": 0.011, "1024x1024/medium": 0.042, "1024x1024/high": 0.167 }
      },
      "source": "https://openai.com/api/pricing/",
      "as_of": "2026-11-24"
    },
    {
      "provider": "elevenlabs",
      "model": "eleven_*",
      "kind": "speech",
      "per_million_characters": 0.0,
      "source": "https://elevenlabs.io/pricing/api",
      "as_of": "2026-11-24"
    }
  ]
}
```

The figures above show the shape only; real prices are filled from each `source` when the table is
written.

## Rules

- `provider`: a `ProviderId` string (aliases accepted, as everywhere).
- `model`: an exact model id, or a prefix ending in `*`. Exact beats prefix; the longest prefix wins.
- `kind`: `image | speech | transcription | embedding`. One entry prices one kind.
- At least one of `tokens`, `per_image`, `per_million_characters`, `per_audio_minute`.
- `tokens`: the catalogue's `Cost` shape (`input`, `output`, optional `cache_read`/`cache_write`),
  USD per 1M tokens.
- `per_image.variants` keys are the record's `variant` (`<size>` or `<size>/<quality>`); a variant
  not listed uses `default`.
- `source` (URL) and `as_of` (date) are required on every entry.
- Parsing failures fail the build's tests (`media_prices_parse`), never startup: a bad entry is
  logged and skipped at runtime.

## Use (research R6)

Catalogue token price → table token rates → table unit price → unpriced. Local models are never
looked up.

## Guard

`every_default_media_model_is_priced_or_local`: for each variant of `ImageGenProvider`,
`TtsProvider`, `SttProvider`, `EmbeddingProvider`, its default model is local, or the catalogue
prices it, or this table does.
