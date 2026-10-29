# Text for the #82 checklist (T051)

Post this as a checklist item on https://github.com/vizier-lab/vizier/issues/82 (the maintainer posts it).

- [ ] **Office addon backfill** (spec 036, FR-025): remove `VizierDependencies::seed_office_defaults` and the `OFFICE_DEFAULT_SEEDED` state key (`migration.office_default_seeded`) in `src/dependencies.rs`, plus its test `the_office_backfill_seeds_each_agent_once_and_leaves_removals_alone`. It gave every agent that existed when the Office addon shipped the addon, once, so none lost the Word and Excel reading that `read_document_file` used to do. Depends on: `integrations::service::seed_default` (stays: `finish_create` in `src/agents/mod.rs` still uses it for new agents) and the `office` kind in `src/integrations/kinds/office.rs` (stays). Safe to remove once no pre-034 database is expected to upgrade.
