pub const EXTRACTION_PROMPT: &str = r#"You are reviewing your recent conversation history to extract valuable insights.

Review the provided session history and extract the following into a structured report:

## Extracted Insights

### Facts & Preferences
- User preferences, project details, technical decisions, environment details

### Feedback & Corrections
- Things the user corrected, praised, or complained about

### Task Progress
- What was accomplished, what's pending, blockers, next steps

### Relationship Context
- Communication style preferences, emotional cues, relationship dynamics

### Learnings
- New patterns discovered, useful information, lessons learned

### Action Items
- Follow-ups needed, reminders, unresolved questions

Write your extraction report using the write_dream_journal tool with stage="extraction"."#;

pub const CONSOLIDATION_PROMPT_TEMPLATE: &str = r#"You are consolidating insights from your recent dream extractions into your long-term knowledge.

Here are the extraction reports from each session this dream cycle:

{extraction_content}

Now do the following:

1. **Create or update memories** — Write extracted facts, learnings, and context to your vector memory using memory_write. Use [[slug]] links to connect related memories.

2. **Update your documents** — Modify AGENT.md, IDENTITY.md, or HEARTBEAT.md if you've learned new patterns about yourself or your user.

3. **Schedule follow-up tasks** — If the extraction identified action items, pending work, or recurring check-ins, create tasks using schedule_cron_task or schedule_one_time_task.

4. **Create new skills** — If you identified recurring workflows, patterns, or specialized knowledge that could be reused, create a new skill using create_skill. First check list_skills to avoid duplicating existing skills.

5. **Link knowledge** — Connect new memories to existing ones. Check memory_list and memory_graph for related memories.

6. **Clean up redundancies** — If new information duplicates existing memories, update the existing ones rather than creating duplicates.

7. **Write your consolidation report** using write_dream_journal with stage="consolidation".

Note: The extraction reports above are already provided — you do NOT need to read them from the dream journal. Use read_dream_journal only if you want to reference older dream entries for context."#;
