# 2.10 Skills

> [!WARNING]
> **Experimental Feature:** The skill system is still in development and may change in future releases.

## Overview

Skills are reusable behaviors that agents can learn and use. Unlike tools which are predefined, skills allow agents to create and store custom instructions for later reuse.

## Creating Skills

Agents create skills using the `create_skill` tool with these parameters:

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string | Skill name in snake_case (e.g., `my_skill`) |
| `description` | string | Short description of what the skill does |
| `instruction` | string | The actual content/instruction for the skill |

### Example

```
create_skill(name="code_review", description="Reviews code for bugs and style issues", instruction="You are a code reviewer. Check the provided code for: 1) Logic errors, 2) Style consistency, 3) Security issues, 4) Performance concerns. Provide actionable feedback.")
```

## How Skills Work

1. **Creation**: Agent uses `create_skill` tool to store a skill
2. **Storage**: Skills are stored in Vizier's storage backend (filesystem or SurrealDB)
3. **Loading**: On agent startup, skills are loaded from storage and converted to tools
4. **Usage**: Skills become callable tools with the prefix `SKILL__`

### Tool Naming

Skills are exposed as tools with the `SKILL__` prefix:

| Skill Name | Full Tool Name |
|------------|----------------|
| `code_review` | `SKILL__code_review` |
| `task_planner` | `SKILL__task_planner` |

## Agent Scoping

Skills are scoped to individual agents:
- Each agent has their own skill storage
- Skills created by one agent are not automatically available to others
- An agent can only access skills they created or that are shared with them

## Skill Management

### Creating a Skill

```python
# Example: Agent creates a skill
create_skill(
    name="daily_summary",
    description="Generates a summary of daily activities",
    instruction="Create a concise summary of the user's day, highlighting key accomplishments and pending tasks."
)
```

### Using a Skill

After creation, the skill becomes available as a tool:

```
SKILL__daily_summary() -> triggers the skill's instruction
```

### Listing Skills

Skills are automatically available as tools. The agent can invoke any skill it has created using `SKILL__<name>`.

## Storage

Skills are persisted in Vizier's storage backend:

- **Filesystem**: Stored in `.vizier/skills/` directory
- **SurrealDB**: Stored in the `skill` table

## Future Development

The skill system is under active development. Planned improvements include:

- Skill sharing between agents
- Skill versioning
- Skill marketplace
- Built-in skill templates

## Example Use Cases

- **Code Review**: Create a skill with coding standards
- **Email Drafting**: Create a skill with email templates
- **Research**: Create a skill for literature review
- **Planning**: Create a skill for project planning