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
- For each item, note:
  - **Type**: task (specific deadline), recurring (periodic check-in), behavioral (preference/correction to remember), or reference (general context)
  - **Urgency**: immediate, soon, or eventually

Output your extraction report directly as your response."#;

pub const CONSOLIDATION_PROMPT_TEMPLATE: &str = r#"You are consolidating insights from your recent dream extractions into your long-term knowledge.

Here are the extraction reports from each session this dream cycle:

{extraction_content}

Now do the following:

1. **Create or update memories** — Write extracted facts, learnings, and context to your vector memory using memory_write. Use [[slug]] links to connect related memories.

2. **Update your documents** — Modify your SOUL, IDENTITY, or HEARTBEAT if you've learned new patterns about yourself or your user.

3. **Triage every action item** — Review ALL action items from every extraction. For EACH item, choose the correct persistence mechanism:

   | Type | Action |
   |------|--------|
   | Specific deadline or one-time follow-up | `schedule_one_time_task` |
   | Recurring check-in at specific times | `schedule_cron_task` |
   | Continuous monitoring or polling | Write to your HEARTBEAT |
   | Behavioral correction or user preference | Update your SOUL |
   | General fact or context worth remembering | `memory_write` |

   Do NOT skip any action item. Every item must be persisted or explicitly noted as intentionally dropped in your final report.

4. **Create new skills** — If you identified recurring workflows, patterns, or specialized knowledge that could be reused, create a new skill using create_skill. First check list_skills to avoid duplicating existing skills.

5. **Link knowledge** — Connect new memories to existing ones. Check memory_list and memory_graph for related memories.

6. **Clean up redundancies** — If new information duplicates existing memories, update the existing ones rather than creating duplicates.

7. **Audit action items** — Re-read all extraction reports. Verify every action item was handled:
   - State the count: N tasks scheduled, M HEARTBEAT items, X SOUL updates, Y memories written
   - List any action items you intentionally dropped and why

8. **Output your consolidation report** as your final text response. Summarize what you did, key decisions made, and anything noteworthy."#;
