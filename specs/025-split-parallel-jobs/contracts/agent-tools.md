# Contract: Subagent Tools

**Feature**: `025-split-parallel-jobs`

Both tools are in category `agents` and are reached from `execute_python` scripts (code mode). Neither is in `DREAM_TOOL_NAMES`. They take the same input.

## Input (both tools)

```json
{
  "tasks": [{"prompt": "self-contained task"}],
  "timeout_secs": 600
}
```

`timeout_secs` is optional, 1..=3600, default 600, per task. An empty `tasks` list or a blank prompt is refused with `tasks must not be empty` before anything runs. Outside the range: `timeout_secs must be between 1 and 3600`.

## `parallel_subagents`: waits

**Description (model-facing)**:

> Run several independent tasks in parallel as subagents and wait for all of them. Returns one result per task, in the order given: `{"results": [{"task", "state", "text", "truncated", "topic"}]}`, with `state` `answered`, `failed` or `timed_out`. Use it when you need the results for your current reply and the tasks are quick. The wait counts against this tool call's time limit: tasks still running when it is reached are stopped and returned as `timed_out`. For long work, use `background_subagents` instead.

**Output**:

```json
{"results": [
  {"task": 0, "state": "answered", "text": "…", "truncated": false, "topic": "6f1c…"},
  {"task": 1, "state": "timed_out", "text": "No answer within 600s", "truncated": false, "topic": "a93e…"}
]}
```

**Guarantees**:
- One entry per task, in input order, whatever happened to the others.
- No background job, tray entry or background report.
- When it returns, no piece is still running.
- When the call is dropped (turn aborted, agent stopped), every unfinished piece is aborted, and its answer never reaches this conversation.

**Refusals** (tool errors, catchable in a script):
- the nesting limit: `nesting limit (3) reached: this turn was itself started by subagent or background work`
- too little time left: `not enough time left in this tool call (Ns); call it earlier in the script or from a new execute_python`

## `background_subagents`: reports later

The behaviour, description and acknowledgement are exactly those of `paralel_subtasks` under `specs/012-background-subagent-results/contracts/agent-tools.md`, under the new name:

> Started background batch b-… with N tasks. Results will arrive as a background report in this conversation.

## `paralel_subtasks`: retired

Not in any toolset or category listing. From a script, calling it raises a `RuntimeError`:

> paralel_subtasks: paralel_subtasks was replaced by parallel_subagents (waits for the results) and background_subagents (results arrive later as a background report)

The leading `paralel_subtasks:` is the sandbox's standard prefix for a catchable tool error. The hint text itself is the sentence after that prefix.

`describe_tool_function {"name": "paralel_subtasks"}` returns `available: false` with the hint text (without the sandbox's tool-name prefix).

## Boot guidance ("Large tasks" section)

> Split work with independent parts (several topics, many files, comparing options) into subagents (category `agents`) so they run in parallel. Subagents don't share your context, so make each prompt self-contained. Use `parallel_subagents` when you need the results for this reply and the parts are quick: it waits and returns them. Use `background_subagents` for long work: it returns at once, and the results arrive later as a background report, so say what you started and don't wait. Don't split small or sequential work.
