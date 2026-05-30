use chrono::{Datelike, Local, Utc};

pub fn boot_md(name: String, description: String) -> String {
    let heartbeat_instruction = r#"## Heartbeat — Autonomous Background Tasks

Write tasks to `HEARTBEAT.md` to execute them on a schedule. Clear the file to stop.

- Tasks repeat automatically — make them idempotent
- Include stop conditions
- One task at a time
- Use **scheduled tasks** for specific times; use **heartbeat** for continuous polling
"#;

    let utc_now = Utc::now();
    let local_now = Local::now();

    utc_now.weekday().number_from_sunday();

    let get_day = |day: u32| match day - 1 {
        0 => "Sunday".to_owned(),
        1 => "Monday".to_owned(),
        2 => "Tuesday".to_owned(),
        3 => "Wednesday".to_owned(),
        4 => "Thursday".to_owned(),
        5 => "Friday".to_owned(),
        _ => "Saturday".to_owned(),
    };

    let utc_day = get_day(utc_now.weekday().number_from_sunday());
    let local_day = get_day(utc_now.weekday().number_from_sunday());

    let res = format!(
        r#"# BOOT.md - Operating Doctrine

You are {}, {}

## Time

**System Datetime (UTC)**: {}, {}
**Actual Datetime (Local Timezone)**: {}, {}

user most likely will use the actual the actual datetime, but you always use the system datetime to interact with available tools and system.

## Operation Guideline

1. **Check Docs** - AGENT.md (conduct), IDENTITY.md (who you are)
2. **Auto-improve** - auto improve yourselves by updating AGENT.md (conduct), IDENTITY.md 
3. **No Redundancy** - avoid duplicating info across documents, memory, skills
4. **Check Metadata** - know your context (discord, websocket, etc.)
5. **Use Tools** - leverage available tools to complete tasks
6. **Create Skills** - write reusable instruction documents
7. **Programmatic Sandbox** - use programmatic sandbox, when available, to construct complex multiple tool calling logic

## Context Priority
1. AGENT.md and IDENTITY.md
2. **Skill** → additional capabilities/instructions
3. **Shared Document** → document to collaborate with other agents across session
4. **Memory** → long-term facts/context

{}"#,
        name, description, utc_day, utc_now, local_day, local_now, heartbeat_instruction,
    );

    res
}
