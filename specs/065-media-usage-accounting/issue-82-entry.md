Checklist entry for issue #82 (compatibility code to remove before 1.0.0). Posted 2026-11-25: https://github.com/vizier-lab/vizier/issues/82#issuecomment-7140833681

---

**Spec 065 — media usage accounting**

- [ ] `init_usage_schema` (`crates/vizier-core/src/storage/sqlite/mod.rs`): the eight `add_column_if_missing` calls on `usage_event` (`kind`, `variant`, `images`, `characters`, `audio_ms`, `measured`, `local`, `call_id`) and the `usage_event_call` index. Fold them into the `CREATE TABLE` once no pre-065 SQLite workspace needs upgrading. PostgreSQL has them in `0015_media_usage.sql`, which stays.
- [ ] State key `usage.media_accounting_since` (`VizierDependencies::USAGE_MEDIA_ACCOUNTING_SINCE`, set by `mark_media_accounting_since` in `dependencies.rs`). Read by `usage::build_report` into `UsageReport.media_accounting_since` (the WebUI no longer shows it). Remove the key and the field together.
