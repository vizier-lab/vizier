# Feature Specification: Python Sandbox & Code Mode (Programmatic Tool Calling)

**Feature Branch**: `007-code-mode-python-sandbox`

**Created**: 2026-09-16

**Status**: Draft

**Input**: User description: "i want to implement code-mode (alias programatic tool call, or rlm) by providing a python sandbox for agent to call tools and execute logics. i believe we could utilise https://github.com/pydantic/monty for this"

## Overview

Today an agent accomplishes a multi-step task by issuing one tool call per model turn: ask for a list, wait for the result, pick an item, call the next tool, wait again. Every intermediate result travels through the model's context, which is slow, expensive, and error-prone for tasks like "fetch these 30 pages and tell me which mention X" or "list my scheduled tasks and delete the ones older than a week". Agents also have no reliable way to do exact computation — arithmetic, date math, parsing, sorting — without a shell.

This feature adds two per-agent capabilities, each behind its own switch, where the second builds on the first:

1. **Python sandbox** — the agent gets a tool that runs a Python script inside a sandbox it cannot escape: no filesystem, no network, no environment, bounded time and memory. It is pure computation: the agent uses it to calculate, transform, parse, and reason with exact logic. The agent's regular tools are untouched and still called directly. This is the low-risk on-ramp: an operator who only wants "the agent can run some Python" turns on just this switch.

2. **Code mode (programmatic tool calling)** — requires the sandbox. The agent's regular tools become callable *from inside* a script as ordinary functions, so the agent can loop and branch over tool results locally and return only the final answer. Code mode is **exclusive**: while it is on, the model's tool list shrinks to the code-execution tool plus a small set of **documentation tools** for discovering which functions exist and what arguments they take; every other capability is reached by writing a script. Tool calls from scripts go through exactly the same permission and dispatch path as direct calls — code mode grants no new capabilities, it only changes *how* the agent orchestrates the ones it already has.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Agent Runs Pure Python in a Sandbox (Priority: P1)

An operator turns on the Python sandbox for an agent. From then on, when the agent needs exact computation — sum a column of numbers a user pasted, work out how many days until a date, parse a messy string, sort and de-duplicate a list, run a small algorithm — it writes a short script, runs it, and uses the printed output or return value in its reply. Nothing else about the agent changes: its regular tools are still offered to the model and called directly as before.

**Why this priority**: This is the smallest independently useful slice and the foundation everything else stands on. It delivers exact computation without a shell, and it proves the sandbox's safety and limits before any tool bridging is layered on.

**Independent Test**: Enable only the sandbox switch on an agent, ask it a computation question (e.g. "what's the 40th Fibonacci number?" or "how many weekdays between two dates?"), and verify it answers correctly via a single sandbox execution while its regular tool list is unchanged.

**Acceptance Scenarios**:

1. **Given** an agent with the sandbox switch on and code mode off, **When** the model inspects its tool list, **Then** it contains the code-execution tool *in addition to* all of the agent's regular tools.
2. **Given** an agent with the sandbox on, **When** it submits a script using in-script logic only (arithmetic, strings, lists/dicts, loops, conditionals, functions, common standard helpers such as math/date/JSON handling), **Then** the script runs and its return value (or printed output when there is no return value) is delivered to the agent.
3. **Given** a script with a syntax error or an unsupported language feature, **When** it is submitted, **Then** the agent receives a clear error identifying the problem (line/description) so it can correct and resubmit.
4. **Given** a script that loops forever, recurses without bound, or allocates without bound, **When** it hits the configured limit, **Then** it is terminated, the agent receives an error naming the limit that was hit, and the agent and hosting process keep serving requests.
5. **Given** a script that tries to read a file, open a network connection, read an environment variable, or import something outside the supported set, **When** it runs, **Then** the attempt fails inside the script with an error and has no effect on the host.
6. **Given** an agent with the sandbox on and code mode off, **When** a script tries to call one of the agent's tools as a function, **Then** the call fails inside the script with an error explaining that tool access from scripts is not enabled for this agent.

---

### User Story 2 - Agent Orchestrates Multiple Tool Calls in One Script (Priority: P2)

An agent with code mode on is asked to do something that requires several dependent tool calls (e.g. "search the web for the three latest Rust releases and summarise each release page"). Instead of issuing a search call, waiting, then issuing three fetch calls one at a time, the agent writes one script: search, loop over the top three results, fetch each, collect the text, and return the collected data. The whole script runs in a single tool call and the agent receives just the final result to reason about.

**Why this priority**: This is the headline value of the feature — fewer model round-trips, less context consumed by intermediate data, and the ability to express filtering, loops, conditionals, and aggregation over tool results. It depends on User Story 1's sandbox and is the reason for it.

**Independent Test**: Enable sandbox + code mode on an agent, ask it a task that needs 3+ dependent tool calls, and verify (a) the task completes correctly, (b) the session history shows a single code-execution tool call where previously it would have shown 3+ separate calls, and (c) the intermediate tool outputs never appear in the model's context.

**Acceptance Scenarios**:

1. **Given** an agent with code mode on and the web-search and fetch tools enabled, **When** the agent submits a script that calls search once and fetch three times in a loop, **Then** all four tool invocations execute, the script's return value is delivered to the agent as the tool result, and the agent produces a correct final answer.
2. **Given** an agent with code mode on, **When** the model inspects its tool list, **Then** it contains only the code-execution tool and the documentation tools — none of the agent's regular tools appear directly — yet every regular tool remains callable from within a script.
3. **Given** a script that calls a tool with invalid arguments, **When** the tool rejects the call, **Then** the error is raised inside the script as a catchable exception, and if uncaught, the script terminates and the agent receives an error message naming the tool and the reason.
4. **Given** a script that calls a tool the agent does not have enabled, **When** the call is attempted, **Then** the call is refused with an error stating the tool is unavailable, and no side effect occurs.
5. **Given** a tool that normally returns an attachment (e.g. generated image, TTS audio, document read), **When** it is called from inside a script, **Then** the attachment is still delivered to the user through the same channel mechanism as a direct call would use.
6. **Given** an agent whose tools include external (MCP) server tools, **When** a script calls one of those tools, **Then** it is dispatched to the external server exactly as a direct call would be.
7. **Given** a script that reaches the per-script tool-invocation limit, **When** it attempts one more tool call, **Then** that call raises an error inside the script, which the script may catch and still return a partial result.

---

### User Story 3 - Agent Discovers Available Tool Functions (Priority: P3)

Because an agent in code mode no longer sees its regular tools in its tool list, it needs a way to find out what functions it can call from a script and how. The agent asks the documentation tools for a catalogue of available functions (name and one-line purpose) and, when it needs detail, for a specific function's full documentation: parameters with types and descriptions, return shape, and a usage example. The same information is also reachable from inside a script, so a script can introspect before calling.

**Why this priority**: Without discovery, exclusive code mode is unusable — the model would be guessing function names and argument shapes. It is the minimum companion to User Story 2 for code mode to work end-to-end. It is not needed when only the sandbox switch is on.

**Independent Test**: Enable code mode on an agent with several tools (including at least one external MCP tool), ask the agent "what can you do?", and verify it lists the available functions from the catalogue; then ask it to perform a task using one of them and verify it looks up the function's documentation before writing a correct call on the first attempt.

**Acceptance Scenarios**:

1. **Given** an agent with code mode on, **When** it calls the catalogue documentation tool, **Then** it receives every function it may call from a script — built-in, conditionally enabled, and external MCP tools — each with its name and a short description, and nothing it may not call.
2. **Given** an agent with code mode on, **When** it asks for the documentation of a specific function, **Then** it receives that function's parameters (names, types, required/optional, descriptions), what it returns, and a short usage example expressed as script code.
3. **Given** an agent that asks for the documentation of a function that does not exist or is not enabled for it, **When** the request is made, **Then** it receives a clear "not available" answer, optionally with the closest matching names.
4. **Given** a running script, **When** it requests documentation for a function from inside the script, **Then** it receives the same information the documentation tool would return, without that counting as a tool invocation against the script's limit.
5. **Given** an operator who enables an additional tool (or adds an MCP server) on an agent with code mode on, **When** the agent next queries the catalogue, **Then** the new function appears without any further configuration.
6. **Given** an agent with the sandbox on but code mode off, **When** the model inspects its tool list, **Then** the documentation tools are not present (there is nothing script-callable to document).

---

### User Story 4 - Operator Configures the Two Switches and Their Limits (Priority: P4)

An operator managing agents through the WebUI (or HTTP API) decides, per agent, whether to turn on the Python sandbox and, separately, whether to turn on code mode on top of it. They can also bound what a script may consume: wall-clock timeout, memory ceiling, and — for code mode — maximum number of tool invocations per script. Both switches are off by default; turning on code mode requires the sandbox to be on.

**Why this priority**: The two capabilities carry different risk profiles, so operators need to opt in to each deliberately and bound them. The feature is still demonstrable with built-in defaults before this control surface exists.

**Independent Test**: Via the WebUI/API, turn on only the sandbox for agent A, sandbox + code mode for agent B, and nothing for agent C; verify each agent's tool list matches (A: regular tools + code execution; B: code execution + documentation only; C: regular tools only). Lower the timeout on A and verify a long-running script is cut off at the new limit.

**Acceptance Scenarios**:

1. **Given** a freshly created agent, **When** the operator inspects its tools configuration, **Then** both switches are off and the agent's tool list does not include code execution or documentation tools.
2. **Given** an agent with both switches off, **When** the operator turns on only the sandbox and saves, **Then** on the agent's next request the code-execution tool appears alongside its regular tools.
3. **Given** an agent with the sandbox on, **When** the operator turns on code mode and saves, **Then** on the agent's next request the model's tool list consists of the code-execution and documentation tools only, and the agent's regular tools are reachable solely from scripts.
4. **Given** an agent with the sandbox off, **When** the operator attempts to turn on code mode alone, **Then** the change is rejected (API) or prevented (WebUI) with a message explaining that code mode requires the sandbox.
5. **Given** an agent with both switches on, **When** the operator turns off the sandbox, **Then** code mode is turned off with it and the agent returns to direct tool calls on its next request.
6. **Given** an agent with the sandbox on, **When** the operator sets a timeout of N seconds and a memory limit, **Then** scripts exceeding either bound are terminated with an error that names which limit was hit; **and** with code mode on, a maximum tool-invocation count is additionally enforced the same way.
7. **Given** the operator's WebUI agent settings, **When** code mode is turned on, **Then** the operator is told that the agent's other tools will be reached only through scripts, so the change is not a surprise.
8. **Given** existing agents persisted from a prior version, **When** the system starts after upgrading, **Then** those agents load with both switches off and behave exactly as before.

---

### User Story 5 - Operator and User Can See What a Script Did (Priority: P5)

A user or operator reviewing a conversation wants to understand what happened during a code-execution call: which script ran, which tools it invoked (with what arguments, if code mode is on), what it printed, what it returned, how long it took, and whether it failed. This information appears in the session history and the WebUI in the same place other tool calls are shown.

**Why this priority**: A code-mode script can trigger many side-effecting tool calls in one go. Auditability matters for trust and debugging, but the feature is functional and valuable without the enriched view.

**Independent Test**: With code mode on, run a script that calls two tools and prints one line, then open the session in the WebUI and verify the script source, both tool invocations, the printed output, the return value, and the duration are visible.

**Acceptance Scenarios**:

1. **Given** a completed code-execution call, **When** the user expands it in the WebUI session view, **Then** they see the script source, an ordered list of every tool invocation made by the script (tool name, arguments, success/failure — empty when only the sandbox is on), captured printed output, the return value, and the execution duration.
2. **Given** a code-execution call that failed, **When** the user views it, **Then** the failure reason (limit exceeded, script error, tool error) and the tool invocations that completed before failure are visible.
3. **Given** a script whose printed output or return value is very large, **When** it is displayed or returned to the agent, **Then** it is truncated with a clear indicator rather than overwhelming the model context or the UI.

---

### Edge Cases

- **Runaway script** (infinite loop, deep recursion, unbounded memory growth): the sandbox terminates it at the configured limit; the agent receives an error naming the limit; the process hosting the agent is unaffected and continues serving other requests.
- **Attempted escape**: any attempt to read/write files, open sockets, read environment variables, import modules outside the supported set, or otherwise reach the host fails with an error inside the script — the sandbox provides none of these capabilities, in either mode.
- **Tool access with code mode off**: a script that calls a tool function when only the sandbox is on gets an error explaining tool access is not enabled; no tool runs.
- **Slow tool inside a script** (code mode): a script that calls a tool which takes a long time (e.g. fetching a slow page, delegating to another agent) counts that time against the script's wall-clock timeout; the existing per-tool timeout still applies to each individual invocation.
- **Tool invocation limit hit mid-script** (code mode): further tool calls raise an error inside the script; the script may catch it and still return a partial result.
- **Nested code execution**: a script attempting to call the code-execution tool itself is refused.
- **Tool name collisions** (code mode): two tools whose names would map to the same script function name (e.g. an MCP tool named like a built-in) are disambiguated deterministically and the catalogue shows the exact name to use.
- **Large catalogues** (code mode): an agent with many MCP servers may have hundreds of functions; the catalogue stays usable (short descriptions only, full docs on request) so it does not flood the model's context.
- **Switch dependency violations**: code mode on with sandbox off is never a valid persisted state — the API rejects it, the WebUI prevents it, and startup treats any such legacy record as code mode off.
- **Concurrent scripts**: two sessions for the same agent running scripts at the same time do not share state or interfere with each other's limits.
- **Non-serialisable return value**: if a script returns something that cannot be represented as structured data, the agent receives an error explaining the return value must be plain data (strings, numbers, booleans, lists, dictionaries, null).
- **Script that never returns a value**: the captured printed output (if any) is delivered as the result; an empty result is still a successful execution.
- **Agent restart or delete during execution**: the in-flight script is abandoned following the same lifecycle as any other in-flight tool call for that agent.

## Requirements *(mandatory)*

### Functional Requirements

**Switches & configuration**

- **FR-001**: Each agent MUST have two independent per-agent settings, both off by default and editable through the same WebUI and HTTP API surfaces used for the agent's other tool settings: a **sandbox** switch and a **code mode** (programmatic tool calling) switch.
- **FR-002**: Code mode MUST require the sandbox: the API MUST reject a configuration with code mode on and sandbox off, the WebUI MUST prevent it, and turning the sandbox off MUST turn code mode off with it.
- **FR-003**: When the sandbox is on and code mode is off, the model's tool list MUST include a code-execution tool that accepts a Python script and returns its result, *in addition to* every regular tool the agent has — the agent's direct tool calling is unchanged.
- **FR-004**: When code mode is on, the model's tool list MUST consist only of the code-execution tool and the documentation tools (FR-012); the agent's regular tools MUST NOT be offered to the model directly.
- **FR-005**: The operator MUST be able to configure, per agent, a wall-clock timeout and a memory ceiling for scripts, and — for code mode — a maximum number of tool invocations per script; each MUST have a sensible built-in default so turning a switch on with no further configuration works.
- **FR-006**: Existing persisted agents MUST load with both switches off after upgrade, with no change to their behaviour.
- **FR-007**: Both capabilities are available in the agent's normal request/response loop. Neither is available during the unattended dream cycle in this version.

**Script execution (both modes)**

- **FR-008**: The code-execution tool's description presented to the agent MUST explain what data types may be passed and returned, which language features are supported and unsupported, the active resource limits, and — when code mode is on — how to call tools from a script and how to look up available functions, so the agent can write valid scripts without trial and error.
- **FR-009**: Each script execution MUST be stateless: no variables, definitions, or data persist from one execution to the next.
- **FR-010**: The result delivered to the agent MUST include the script's return value (or captured printed output when there is no return value) and, on failure, a description of the failure and its category (script error / tool error / limit exceeded).
- **FR-011**: A script MUST NOT be able to invoke the code-execution tool itself (no nesting).

**Tool access from scripts (code mode only)**

- **FR-012**: When code mode is on, the agent MUST be offered documentation tools that (a) list every function callable from a script — built-in, conditionally enabled, and external MCP tools — with name and short description, and (b) return a single function's full documentation: parameters (name, type, required/optional, description), return shape, and a usage example as script code. The catalogue MUST reflect the agent's current tool configuration on every call and MUST NOT include tools the agent cannot call. These tools MUST NOT be offered when code mode is off.
- **FR-013**: The same documentation MUST be obtainable from inside a running script, and such lookups MUST NOT count toward the script's tool-invocation limit.
- **FR-014**: Asking for documentation of a function that does not exist or is not enabled MUST return a clear "not available" response rather than an error that terminates the turn.
- **FR-015**: A script MUST be able to invoke any tool the agent currently has available (built-in default tools, conditionally enabled tools, and tools from the agent's external MCP servers) as a function call, passing arguments as plain data and receiving the tool's result as plain data.
- **FR-016**: Tool invocations from within a script MUST go through the same dispatch, permission, and timeout path as a direct tool call; a script MUST NOT be able to reach any tool the agent could not call directly.
- **FR-017**: Tool errors raised inside a script MUST surface as catchable exceptions so the script can handle them; an uncaught exception MUST terminate the script and be reported to the agent with the tool name and reason.
- **FR-018**: Tools that produce user-facing attachments (images, audio, documents) MUST deliver those attachments to the user when called from within a script exactly as they would from a direct call.
- **FR-019**: When code mode is off, any attempt by a script to call a tool function MUST fail inside the script with an error stating that tool access from scripts is not enabled, and no tool MUST run.

**Sandbox guarantees (both modes)**

- **FR-020**: The sandbox MUST provide no access to the host filesystem, network, environment variables, or process — the only bridge to the outside world is the set of tool functions exposed under code mode.
- **FR-021**: The sandbox MUST enforce the configured wall-clock timeout, memory ceiling, recursion depth, and (in code mode) tool-invocation count, terminating the script and reporting which limit was exceeded.
- **FR-022**: A script that exhausts its limits or crashes MUST NOT affect the availability of the agent, other agents, or the hosting process.
- **FR-023**: Concurrent script executions (across sessions or agents) MUST be isolated from one another.
- **FR-024**: The sandbox MUST NOT require any external service, runtime, or interpreter to be installed on the host; it MUST ship inside the single binary and work in config-less mode and on every supported build target.

**Observability (both modes)**

- **FR-025**: The system MUST record, for each execution, the script source, every tool invocation made (name, arguments, outcome, order — empty when code mode is off), captured printed output, the return value or error, and the duration, and MUST expose this in the session history and WebUI where other tool calls are shown.
- **FR-026**: Return values and printed output exceeding a configurable size MUST be truncated with an explicit truncation marker both in what the agent receives and in what is stored.
- **FR-027**: Each execution and each tool invocation inside it MUST be logged through the existing logging facility with the agent and session identifiers, so operators can trace activity without the WebUI.

### Key Entities

- **Sandbox Settings**: per-agent configuration — sandbox switch, code-mode switch (valid only when sandbox is on), wall-clock timeout, memory ceiling, maximum tool invocations per script (code mode), output size limit. Lives alongside the agent's other tool settings.
- **Code Execution Request**: a single submission by an agent — the script source, the agent and session it belongs to, and the moment it was submitted.
- **Code Execution Result**: the outcome of one request — return value or captured output, error (if any) with its category (script error / tool error / limit exceeded), duration, and the ordered list of Tool Invocation Records (always empty when code mode is off).
- **Tool Invocation Record**: one tool call made from inside a script — tool name, arguments, success or failure, and its position in the script's execution order. Conceptually the same thing as a direct tool call, but nested under its parent execution.
- **Tool Documentation Entry**: the script-facing view of one tool — function name, short description, parameter list (name, type, required/optional, description), return shape, and a usage example. Derived from the tool's existing definition; never hand-maintained separately. Only exists under code mode.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With only the sandbox on, an agent answers exact-computation questions (arithmetic, date math, parsing, sorting) correctly in a single execution in at least 95% of cases, versus the error-prone mental arithmetic it must do today.
- **SC-002**: With code mode on, a task requiring N dependent tool calls (N ≥ 3) completes in a single model round-trip, versus N+1 round-trips without it, with the same correctness of final answer.
- **SC-003**: For such a task, the amount of intermediate tool output that enters the model's context is reduced by at least 80% compared to the direct-call approach (only the script's final return value enters context).
- **SC-004**: Sandbox start-up plus execution of a trivial script adds no more than 50 milliseconds of overhead beyond the time spent in any tools it calls.
- **SC-005**: 100% of attempts from within a script to access the filesystem, network, environment, or host process fail without effect, in either mode.
- **SC-006**: 100% of runaway scripts (infinite loop, unbounded allocation, deep recursion) are terminated within the configured limit, with the agent and hosting process continuing to serve requests afterwards.
- **SC-007**: With only the sandbox on, the agent's regular tool list is identical to before except for the addition of the code-execution tool — 0 regressions in direct tool calling.
- **SC-008**: With code mode on, the model's tool list contains a fixed small number of entries (code execution plus documentation) regardless of how many regular or MCP tools the agent has enabled.
- **SC-009**: Every tool invocation made from within a script is visible in the session history with the same fidelity (name, arguments, outcome) as a direct tool call.
- **SC-010**: An agent given a script error (syntax, unsupported feature, bad tool arguments) can correct and successfully resubmit within one additional turn in at least 90% of cases, because the error message is specific enough to act on.
- **SC-011**: After consulting a function's documentation, an agent writes a call with correct argument names and types on the first attempt in at least 90% of cases.
- **SC-012**: Turning on either switch requires a single setting change and no additional installation or configuration by the operator.

## Assumptions

- **Language**: scripts are written in Python, because it is the language models write most reliably and the one used by comparable "code mode" features elsewhere. A restricted subset of the language is acceptable; the exact supported subset (including which standard helpers are available) is decided during planning and surfaced to the agent via the tool description (FR-008).
- **Candidate engine**: the user suggested the Monty sandboxed Python interpreter (a Rust-native interpreter that ships as a library, provides no filesystem/network/environment access, enforces time/memory/recursion limits internally, and exposes host functions to the script as external calls). It is a strong fit for the single-binary and portability constraints; final selection and validation against every supported build target happens in the plan, per the constitution's dependency and cross-compilation rules.
- **Two switches, one dependency (user decision)**: the sandbox switch is the base capability and is additive to the agent's tool list; the code-mode switch layers programmatic tool calling on top and is exclusive (regular tools hidden from the model). Code mode without the sandbox is not a valid state.
- **Opt-in by default**: both switches are off for all agents unless explicitly enabled, consistent with how other conditional tools are handled.
- **Same tool, two behaviours**: the code-execution tool is one tool whose behaviour differs only in whether tool functions are bridged into the script; it does not become a second, separately named tool when code mode is turned on.
- **Defaults**: initial default limits are on the order of 30 seconds wall-clock, a memory ceiling in the tens of megabytes, 50 tool invocations per script (code mode), and 64 KB of result output before truncation. These are starting points to be tuned during implementation, not requirements.
- **Stateless executions**: no persistent sandbox session across calls in v1. If agents frequently need to carry state between scripts, a resumable-session variant can be a follow-up.
- **Dream cycle excluded**: unattended reflection keeps its current restricted tool subset; either switch can be extended there in a later revision once behaviour in attended sessions is understood.
- **Documentation is derived, not authored**: the documentation tools generate their output from each tool's existing name, description, and input/output definitions (including those reported by external MCP servers), so new tools are discoverable with no extra work.
- **Always-on housekeeping tools**: under code mode, a small number of existing tools that are about the model's own reasoning rather than acting on the world (e.g. the "think" scratchpad) may remain directly exposed alongside the code-execution and documentation tools; the plan decides the exact set, which MUST stay fixed and small.
- **Tool-call visibility in history**: a code execution is stored as one tool call in the session history whose result embeds the nested tool-invocation records, rather than being flattened into many top-level tool calls. This keeps the conversation replayable for the model and preserves the round-trip savings.
- **Existing surfaces reused**: agent tool settings (WebUI + `/api/v1` agent endpoints), the session history view, `tracing` logging, and the per-agent tool dispatch path are extended, not duplicated.
- **Shell tool interaction**: the sandbox is independent of the existing shell tool (local/Docker). Under code mode a script may call the shell tool if the agent has it enabled, subject to the same permissions as a direct call.
