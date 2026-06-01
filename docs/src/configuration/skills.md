# 2.10 Skills

Skills are reusable instruction documents that agents can load to perform specific tasks. They provide domain expertise, workflows, and checklists without modifying the agent's core system prompt.

## What Are Skills?

A skill is a **folder** containing a `SKILL.md` file with YAML frontmatter and markdown instructions. Skills can also include **resources** — additional files like templates, reference documents, or scripts.

**Example structure:**
```
skills/
  code-review/
    SKILL.md                    # Skill manifest + instructions
    resources/
      review-template.md        # Template for review comments
      checklist.md              # Review checklist
```

## Skill Format

### SKILL.md

Each skill has a `SKILL.md` file with YAML frontmatter:

```yaml
---
name: code-review
author: vizier
description: Guidelines for conducting thorough code reviews
keywords: [review, quality, security, code, pr]
activation: contextual
version: 1
---

# Code Review Skill

This skill provides guidelines and checklists for conducting effective code reviews.

## Review Checklist

### 1. Code Quality
- [ ] Code is readable and well-organized
- [ ] Functions are appropriately sized
- [ ] Variable names are descriptive

### 2. Security
- [ ] Input validation is present
- [ ] No SQL injection vulnerabilities
- [ ] Authentication is properly implemented
```

### Frontmatter Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | — | Skill identifier (slug format) |
| `author` | string | Yes | — | Who created the skill |
| `description` | string | Yes | — | Brief description of the skill's purpose |
| `keywords` | array | No | `[]` | Keywords for contextual matching |
| `activation` | enum | No | `on_demand` | How the skill is activated |
| `version` | number | No | `1` | Skill version number |

### Activation Modes

| Mode | Description |
|------|-------------|
| `always` | Skill is always injected into the agent's system prompt |
| `on_demand` | Agent must explicitly call `SKILL__<name>` to load the skill |
| `contextual` | Skill is automatically loaded when task matches keywords/embeddings |

**When to use each mode:**

- **Always**: Core instructions the agent needs for every task (e.g., coding standards)
- **On Demand**: Specialized skills the agent should load when needed (e.g., calculator)
- **Contextual**: Skills that should activate based on task content (e.g., code review when reviewing PRs)

## Resources

Resources are additional files in the skill folder (excluding `SKILL.md` and `.meta.json`). They can be:

- **Templates**: Reusable text templates
- **References**: Documentation, style guides, checklists
- **Data**: JSON, YAML, or other structured data
- **Scripts**: Shell, Python, or other executable files

Agents can access resources using the `read_skill_resource` tool:

```
read_skill_resource(slug="code-review", path="resources/checklist.md")
```

Scripts can be executed with `execute_skill_resource`:

```
execute_skill_resource(slug="deploy", path="scripts/deploy.sh")
```

## Skill Locations

Skills can be stored in two locations:

| Location | Path | Description |
|----------|------|-------------|
| **Global** | `{workspace}/skills/` | Available to all agents |
| **Agent-specific** | `{workspace}/agents/{id}/skills/` | Private to a specific agent |

**Resolution order**: When looking up a skill, agent-specific skills take priority over global skills with the same name.

## Creating Skills

### Via WebUI

1. Navigate to Skills in the sidebar
2. Click "New Skill"
3. Fill in name, description, keywords, activation mode
4. Write instructions in the markdown editor
5. Save

### Via API

```sh
POST /api/v1/skills
```

```json
{
  "name": "my-skill",
  "description": "My custom skill",
  "content": "# My Skill\n\nInstructions here...",
  "keywords": ["custom", "example"],
  "activation": "on_demand"
}
```

### Via Agent Tool

Agents can create skills during conversation using the `create_skill` tool:

```
create_skill(
  name="deployment-checklist",
  description="Pre-deployment verification checklist",
  instruction="# Deployment Checklist\n\n1. Run tests\n2. Check logs\n3. Deploy",
  keywords=["deploy", "release"],
  activation="contextual"
)
```

## Installing Skills

### From Registry

Install a skill from the vizier-lab/vizier repository:

```sh
vizier skill install code-review
```

### From Git Repository

Install from any git repository:

```sh
vizier skill install https://github.com/user/custom-skills.git
```

### From Local Path

Install from a local directory:

```sh
vizier skill install ./my-local-skill
```

### Install for Specific Agent

```sh
vizier skill install code-review --agent my-agent
```

## Managing Skills

### List Installed Skills

```sh
vizier skill list
```

Filter by activation mode:

```sh
vizier skill list --activation contextual
```

### Update a Skill

Update a skill from its registry source:

```sh
vizier skill update code-review
```

> **Note:** Only skills installed from the registry can be updated. Skills created locally or from external git repos cannot be updated via CLI.

### Uninstall a Skill

```sh
vizier skill uninstall code-review
```

Uninstall from a specific agent:

```sh
vizier skill uninstall code-review --agent my-agent
```

## Skill Matching (Contextual)

When a skill has `activation: contextual`, it is automatically loaded based on task content. The matching process:

### 1. Keyword Matching (Fast)

Checks if any of the skill's `keywords` appear in the task text:

```
Task: "review this PR for security issues"
Skill keywords: ["review", "quality", "security"]
→ Match found: "review" and "security"
```

### 2. Description Matching (Fallback)

If no keyword matches, compares task against skill name and description:

```
Task: "check code quality"
Skill name: "code-review"
Skill description: "Guidelines for conducting code reviews"
→ Match: "code" and "review" appear in both
```

### 3. Embedding Similarity (Future)

If no text matches, the system can use embedding similarity to find relevant skills (requires embedding configuration).

## Agent Tools for Skills

| Tool | Description |
|------|-------------|
| `create_skill` | Create a new skill |
| `update_skill` | Update an existing skill's content, description, keywords, or activation |
| `delete_skill` | Delete a skill and all its resources |
| `list_skills` | List available skills, optionally filtered by keyword |
| `read_skill_resource` | Read a resource file from a skill folder |
| `execute_skill_resource` | Execute a script from a skill folder |

## Example Skills

### Calculator

```yaml
---
name: calculator
author: vizier
description: Perform mathematical calculations and computations
keywords: [math, calculate, compute, arithmetic]
activation: on_demand
version: 1
---

# Calculator Skill

Perform mathematical calculations when the user asks for math operations.

## Capabilities

- Basic arithmetic (add, subtract, multiply, divide)
- Unit conversions
- Statistical calculations
```

### Code Review

```yaml
---
name: code-review
author: vizier
description: Guidelines for conducting thorough code reviews
keywords: [review, quality, security, code, pr]
activation: contextual
version: 1
---

# Code Review Skill

## Review Checklist

### Code Quality
- [ ] Readable and well-organized
- [ ] Functions are appropriately sized
- [ ] No code duplication

### Security
- [ ] Input validation present
- [ ] No injection vulnerabilities
- [ ] Authentication properly implemented
```

## Version Tracking

Skills installed via CLI have a `.meta.json` file tracking their source:

```json
{
  "source": "registry",
  "registry_url": "https://github.com/vizier-lab/vizier.git",
  "slug": "code-review",
  "installed_version": 1,
  "installed_at": "2025-01-15T10:00:00Z"
}
```

This enables `vizier skill update` to pull the latest version from the registry.
