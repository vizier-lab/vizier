# Text for the #82 checklist (T051)

Post this as a checklist item on https://github.com/vizier-lab/vizier/issues/82 (the maintainer posts it).

- [x] **Office addon backfill** (spec 036, FR-025): **done** — `VizierDependencies::seed_office_defaults`, the `OFFICE_DEFAULT_SEEDED` key and its test were removed together with the creation seed, when the repository owner made the addon opt-in (FR-015a). No build seeds the Office addon any more; `integrations::service::seed_default` stays because the HTTP addon backfill (`seed_tool_addons`) still uses it.
