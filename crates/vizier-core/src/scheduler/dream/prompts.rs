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

### Reusable Workflows
- Sequences of tool calls you ran more than once, or that the person asked you to repeat.
- For each: the steps in order, the inputs that changed between runs, whether any step needed
  your judgment (and which), how many times you saw it, and the script if one ran.

### Action Items
- Follow-ups needed, reminders, unresolved questions
- For each item, note:
  - **Type**: task (specific deadline), recurring (periodic check-in), behavioral (preference/correction to remember), or reference (general context)
  - **Urgency**: immediate, soon, or eventually

### Source Files
You may examine the files of the conversation(s) this cycle reflects on. Use
`list_session_files(session="<source slug>")` to see them and `read_document_file` /
`read_image_file` with the same `session` to open one. If a file is worth keeping beyond that
conversation, copy it into a permanent artifact with
`promote_artifact(filename="...", source_session="<source slug>")`; the source file is left
untouched. Do not try to change or clear source files — the dream toolset cannot.

Output your extraction report directly as your response."#;

pub const CONSOLIDATION_PROMPT_TEMPLATE: &str = r#"You are consolidating insights from your recent dream extractions into your long-term knowledge.

Here are the extraction reports from each session this dream cycle:

{extraction_content}

Now do the following:

1. **Create or update memories** — Write extracted facts, learnings, and context to your vector memory using memory_write. Use [[slug]] links to connect related memories. **Prefer several small, single-subject memories over one large one**: one memory is one thing, and a document covering three subjects competes with itself for the slots a search can return.

2. **Update your Core** — `core/soul` in your memory. Rewrite it with `memory_write` if you've learned new patterns about yourself or your user.

3. **Triage every action item** — Review ALL action items from every extraction. For EACH item, choose the correct persistence mechanism:

   | Type | Action |
   |------|--------|
   | Specific deadline or one-time follow-up | `schedule_one_time_task` |
   | Recurring check-in at specific times | `schedule_cron_task` |
   | Behavioral correction or user preference | `memory_write` to `core/soul` |
   | General fact or context worth remembering | `memory_write` |

   Do NOT skip any action item. Every item must be persisted or explicitly noted as intentionally dropped in your final report.

4. **Create or update skills and custom tools** — For every reusable workflow in the reports:
   - steps fixed, only the inputs vary → a custom tool (`create_custom_tool`; `input` holds the
     arguments, the last expression is the result);
   - steps need judgment → a skill (`create_skill`);
   - a one-off → nothing.
   First check `list_custom_tools` and `list_skills`; update an existing one rather than create a
   near-duplicate. Put a new custom tool in a fitting existing category before making a new one,
   and if categories hold only one or two related tools, merge them
   (`update_custom_tool` with another `category`, `update_custom_tool_category`). Specialized
   knowledge worth reusing is a skill too.

5. **Link knowledge** — Connect new memories to existing ones. Check memory_list and memory_graph for related memories.

6. **Clean up redundancies** — If new information duplicates existing memories, update the existing ones rather than creating duplicates.

7. **Keep and attach artifacts** — Use `list_artifacts` to see what is already permanent. A
   valuable file you found in a source conversation can be kept with
   `promote_artifact(filename="...", source_session="<source slug>")`. Attach a permanent
   artifact to a memory with `attach_artifact_to_memory(path="...", artifact="...")`, and
   remove one with `detach_attachment_from_memory(path="...", name="...")`. Do not update,
   delete, send or clear anything.

8. **Audit action items** — Re-read all extraction reports. Verify every action item was handled:
   - State the count: N tasks scheduled, X Core updates, Y memories written, N skills and M custom tools created or updated
   - List any action items you intentionally dropped and why, and the reusable workflows you left alone and why

9. **Output your consolidation report** as your final text response. Summarize what you did, key decisions made, and anything noteworthy, and list the custom tools you created or updated by name."#;

#[cfg(test)]
mod tests {
    use super::*;

    /// `specs/061-agent-custom-tools` contracts/prompt-and-dream.md §2–§3.
    #[test]
    fn the_dream_looks_for_reusable_workflows_and_says_where_each_goes() {
        assert!(EXTRACTION_PROMPT.contains("### Reusable Workflows"));
        for needle in ["create_custom_tool", "list_custom_tools", "a one-off → nothing", "M custom tools"] {
            assert!(CONSOLIDATION_PROMPT_TEMPLATE.contains(needle), "{needle}");
        }
        assert!(CONSOLIDATION_PROMPT_TEMPLATE.contains("{extraction_content}"));
    }
}
