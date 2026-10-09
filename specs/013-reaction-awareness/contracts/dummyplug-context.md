# Contract: Dummyplug §6, `context`

**Feature**: `013-reaction-awareness` · An additive amendment to `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`

## Behaviour

When the command text of the incoming user message, trimmed, is exactly `context`, dummyplug replies with one text message:

- the verbatim text of the per-request context block, which is the user-content block starting with `# Context\n` (`CONTEXT_HEADER`); or
- `(no context block)` if the message didn't carry one.

Usage is zero, as in §5.

## Precedence

§6 is checked after §1 (`tools`) and before §2 (tool name), §3 (JSON) and §5 (lorem). No built-in tool is named `context`, so §2 can't shadow it.

## Compatibility

This is additive. Every message that produced a §1 to §5 reply before still produces the same reply, so all existing quickstart scripts are unaffected. The protocol document gets a §6 section in the same change, as the constitution requires.

## Why

FR-007 and SC-002/003 are about what the agent **receives**. Dummyplug otherwise answers prose with lorem ipsum, so §6 is the only offline way to assert on the context block, including the new `## Reactions` section.
