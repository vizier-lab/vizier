---
name: skill-maker
author: vizier
description: Guide for creating effective reusable skills for agents
keywords: [skill, create, template, instruction, workflow, reusable]
activation: contextual
version: 1
---

# Skill-Maker

This skill teaches you how to create effective, reusable skills for yourself and other agents.

## What Makes a Good Skill

A good skill is:

- **Focused** — does one thing well, not everything
- **Actionable** — clear instructions, not vague guidelines
- **Discoverable** — keywords match how tasks are described
- **Reusable** — useful across multiple sessions and contexts

## SKILL.md Structure

Every skill needs a `SKILL.md` file with YAML frontmatter:

```yaml
---
name: my-skill
author: your-name
description: Short description of what this skill does
keywords: [word1, word2, word3]
activation: on_demand
version: 1
---
```

### Frontmatter Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Slug format (lowercase, hyphens) |
| `author` | Yes | Who created it |
| `description` | Yes | One-line purpose (used in tool descriptions) |
| `keywords` | No | Words that trigger contextual activation |
| `activation` | No | `always`, `on_demand`, or `contextual` (default: `on_demand`) |
| `version` | No | Number for tracking updates |

### Content Structure

After frontmatter, write clear markdown instructions:

1. **Purpose** — what this skill does and when to use it
2. **Capabilities** — what the agent can do with this skill
3. **Guidelines** — step-by-step instructions or rules
4. **Examples** — concrete usage scenarios

## Keyword Selection

Keywords determine when contextual skills activate. Choose wisely:

### Good Keywords
- Task-specific: `deploy`, `review`, `debug`, `test`
- Action verbs: `build`, `format`, `lint`, `migrate`
- Domain terms: `api`, `database`, `docker`, `ci`

### Bad Keywords
- Too common: `the`, `and`, `with`
- Too vague: `help`, `do`, `stuff`
- Too specific: `fix-the-bug-in-login-handler` (won't match anything)

### Guidelines
- Use 3-8 keywords per skill
- Include word variations: `review`, `reviews`, `reviewing`
- Prefer lowercase, singular forms
- Test: would a user naturally say this word when asking for help?

## Activation Mode Guide

Choose based on how often the skill is needed:

| Mode | When to Use | Example |
|------|-------------|---------|
| `always` | Core instructions needed every task | Coding standards, response format |
| `on_demand` | Specialized, agent loads when asked | Calculator, specific tool usage |
| `contextual` | Auto-activates based on task keywords | Code review, deployment, debugging |

### Decision Tree

```
Is this needed for EVERY task?
  → Yes: always
  → No: Will the agent know when to ask for it?
        → Yes: on_demand
        → No: contextual (with good keywords)
```

## Good vs Bad Skills

### Bad: Too Broad
```yaml
name: coding
description: Help with coding
keywords: [code, programming, software]
```
**Problem**: Tries to do everything, useful for nothing specific.

### Good: Focused
```yaml
name: rust-error-handling
description: Patterns for handling errors in Rust code
keywords: [rust, error, result, anyhow, thiserror]
activation: contextual
```
**Why it works**: Clear scope, specific keywords, right activation.

### Bad: Missing Keywords
```yaml
name: api-design
description: Best practices for designing REST APIs
keywords: []
activation: contextual
```
**Problem**: No keywords → never activates contextually.

### Good: Discoverable
```yaml
name: api-design
description: Best practices for designing REST APIs
keywords: [api, rest, endpoint, route, http]
activation: contextual
```
**Why it works**: Keywords match natural task descriptions.

## Common Mistakes

1. **Too broad** — "coding help" is not a skill, it's a system prompt
2. **Too narrow** — "fix button alignment in login page" is a task, not a skill
3. **Wrong activation** — using `always` for rarely-needed skills clutters context
4. **Missing keywords** — contextual skills without keywords never activate
5. **Vague instructions** — "do a good job" is not actionable
6. **No examples** — agents learn better with concrete examples
7. **Duplicate existing** — check if a similar skill already exists before creating
