# Contract: Browser Storage and Page Application

## Keys

| Key | Value | Absent means |
|---|---|---|
| `vizier.theme` | `"light"` or `"dark"`, plain string, as today | System |
| `vizier.scheme` | Selected scheme id, plain string | `vizier` |
| `vizier.schemes` | `{ "version": 1, "schemes": Scheme[] }`, custom schemes only | None |
| `vizier.scheme.paint` | `{ "id": string, "light"?: Vars, "dark"?: Vars }` | No overrides |

`Vars` maps a `--vz-*` name to a `#rrggbb` value. `vizier.scheme.paint` is a cache, rebuilt from the other keys. It is removed when the Vizier scheme is selected.

No key is ever sent to the server. Signing out does not clear them.

## Writing

- Every write is in `try/catch`. A failed write keeps the change for the session and raises one toast: "Your theme couldn't be saved in this browser."
- Selecting a scheme writes `vizier.scheme` and `vizier.scheme.paint`.
- Saving a scheme that is selected also rewrites the paint cache.

## Pre-paint script (`webui/app/root.tsx`)

It extends the existing inline script and runs before the stylesheet, under these rules:

1. Read `vizier.theme` and apply `data-theme` as today.
2. Read and parse `vizier.scheme.paint`. If it is absent or unparsable, stop.
3. Pick the variant:
   - only one present: use it and set `data-theme` to its appearance;
   - both present: use `vizier.theme`, otherwise `matchMedia('(prefers-color-scheme: dark)')`.
4. For each entry, set it only if the name matches `^--vz-[a-z0-9-]+$` and the value matches `^#[0-9a-fA-F]{6}$`.
5. The whole script stays in `try/catch`, so it can never stop the page from loading.

## Applying (`apply.ts`)

A pure `resolve(scheme, mode, deviceIsDark)` returns `{ appearance, dataTheme, vars }`. `apply` then:

1. Removes every role variable from `<html>` that is not in `vars`.
2. Sets `vars` and `data-theme`.
3. Tells the desktop shell the `appearance`, so its frame matches.
4. Runs inside `startViewTransition` under the existing rule: not on the desktop, not with reduced motion.

`apply` runs on selection, on saving the selected scheme, when the device's setting changes while the mode is System, and on `storage` events.

## Other tabs

A `storage` event on any of the first three keys reloads state and re-applies. The paint cache is rebuilt locally rather than listened to, so tabs do not echo writes to each other.

## Observers

`useAppearanceKey()` in `lib/useDark.ts` returns a string that changes whenever the colors on screen change, by observing `data-theme` and `style` on `<html>` and the media query. Code that draws with resolved colors uses it as a dependency. `useDark()` stays for code that only needs light or dark.

## Transport

```ts
saveText(text: string, filename: string): Promise<void>
```

- **HTTP**: a blob link with `download`.
- **Desktop**: `invoke('save_text', { text, filename })`, which opens the save dialog using `desktop::safe_filename`, as `save_file` does, and writes the file. Cancelling is not an error.

Only `webui/app/api/transport/` may call it directly, as `boundary.test.ts` requires for this seam.
