# Contract: Library scripts

Command-line contracts for the scripts in `vizier-lab/design`. There is deliberately no script that reads or writes FigCraft files (FR-023). Each one is a single Node ESM file with no network access. Each exits `0` on success, and on failure exits `1` with a one-line `error: …` message on stderr.

## `npm run lint:tokens` → `scripts/lint-tokens.mjs`

FR-015 check.

- **Scans**: `src/**/*.{ts,tsx,css}` except `src/tokens/**`.
- **Fails on**: hex, `rgb()` or `hsl()` literals; Tailwind arbitrary values (`-[…]`) for colour, spacing, radius or font size; `px` literals in CSS outside `src/tokens/`.
- **Allows**: colours inside `src/components/ProviderLogo/brands.ts` and `src/components/IntegrationLogo/brands.ts` (FR-015 exception), and `0`/`1px` hairlines.
- **Output**: `file:line: <match>` per violation.

## `npm test`

Runs every story headless (Vitest + Playwright Chromium, `@storybook/addon-vitest`) with axe. Fails on a critical a11y violation, on a render error, or on a story missing `parameters.vizier.id`.

## `npm run storybook` / `npm run build-storybook`

Catalogue on `http://localhost:6006` / static copy in `storybook-static/` (gitignored, not hosted).
