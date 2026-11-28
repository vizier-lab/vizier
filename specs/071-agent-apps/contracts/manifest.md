# Contract: the app manifest

The manifest is stored as JSON in `app_version.manifest`. The driver writes it through `app_edit` (see [app-tools.md](app-tools.md)), never as one raw blob, but the stored shape is the following:

```jsonc
{
  "manifest_version": 1,
  "name": "Habits",
  "icon": "square-check",              // IconName from design/src/icons/map.ts; unknown → monogram
  "description": "Track daily habits for the week.",
  "entry": "page",                      // the version's page blob; reserved for multi-page later

  "migrations": [                       // append-only across versions (FR-018a)
    {"seq": 1, "sql": "CREATE TABLE habit (id INTEGER PRIMARY KEY, name TEXT NOT NULL, optional_sunday INTEGER NOT NULL DEFAULT 0);"},
    {"seq": 2, "sql": "CREATE TABLE tick (habit_id INTEGER NOT NULL REFERENCES habit(id), day TEXT NOT NULL, PRIMARY KEY (habit_id, day));"}
  ],

  "functions": [
    {
      "name": "sync_calendar",
      "description": "Copy this week's ticks to the calendar.",
      "input_schema": {"type": "object", "properties": {}},
      "output_schema": {"type": "object"},
      "script": "events = google_calendar_list_events(...)\n...",
      "calls": ["google_calendar_list_events", "app_query"]   // derived on save; read-only
    }
  ],
  "page_callable": ["sync_calendar"],   // subset of functions the page may ask to run (FR-029)

  "schedules": [
    {"name": "morning", "cron": "0 8 * * *", "run": "sync_calendar"},
    {"name": "weekly-review", "cron": "0 18 * * 0", "ask": "Summarise this week's habits in the chat."}
  ],

  "tools": [                            // manifest tools answered by the open page (FR-033)
    {"name": "highlight_row", "description": "Highlight a habit row.",
     "input_schema": {"type":"object","properties":{"habit_id":{"type":"integer"}},"required":["habit_id"]},
     "ends_turn": false}
  ],

  "requires": {                         // declared; merged with derived needs
    "secrets": ["CAL_TOKEN"],
    "mcp": [{"server": "google-calendar", "tools": ["list_events"]}],
    "addons": [],
    "model": {"vision": false}
  }
}
```

## Rules

1. **Names.** Function and tool names match `^[a-z][a-z0-9_]{0,47}$` and are unique within the manifest. A function may not share a name with a manifest tool, an `app_*` tool or a Python builtin. The custom-tool reserved-name rules apply (`custom_tools::naming::reserved`).
2. **Functions.** Each function is validated with `custom_tools::validate::validate_definition`: name, schema subset, Python syntax, calls, unknown names and cycles. `calls` is recomputed on save, and a value supplied by the driver is ignored. A function may call another function of the same app; cycles are refused.
3. **Derived requirements.** The tool needs of the app are the union of every function's `calls` that are not functions of the app, not `app_*` tools and not Python builtins. Each call maps to a tool category through the driver's catalogue when the check runs. A name no agent could have is a validation error with `did_you_mean`.
4. **Declared requirements** cover only what cannot be derived:
   - secrets, by **name**;
   - MCP servers and the tools needed from them;
   - addons;
   - model features.
5. **Migrations.**
   - The list in version *n+1* must start with the list in version *n*: the same `seq` and byte-identical `sql`.
   - Only new entries may be added, at the end.
   - Each new entry runs once, in order, in one transaction, when its version is saved. A failure refuses the save.
   - Migration SQL may use DDL. Page and function SQL may not.
6. **`page_callable`** must name functions of the manifest. Adding a name needs a new owner approval before the page can call it (FR-029).
7. **Schedules.** A schedule has exactly one of `run` (a function name) or `ask` (an instruction of at most 2,000 characters). `cron` is evaluated in local time, as for tasks (spec 052). At most 16 schedules.
8. **Tools.** At most 32. Fields and limits follow `ClientToolDeclaration` (spec 019), except `deferrable`, which is always false: a call needs the open app.
9. **Limits.** 64 functions, 64 KiB per script, 256 KiB total manifest JSON.
10. **First version.** `name` must not be `Untitled app` and `icon` must be set (research D14).
