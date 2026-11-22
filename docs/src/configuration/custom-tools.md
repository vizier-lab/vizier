# Functions (Custom Tools)

An agent can save a Python script that combines its tools as a **function**: a named, versioned tool of its own that it calls like any built-in one. A workflow the agent keeps repeating, such as "pull last week's notes, summarise them, write a digest", becomes one call.

Agents create functions themselves, usually after noticing a repeated sequence, and their [dream cycle](./agents.md#dream-cycle) looks for such sequences too. You do not write them: you read, switch off, edit, roll back and delete them. Functions belong to one agent and are not shared with others.

## Function or skill?

A **function** is for a workflow whose steps are fixed and only the inputs change. A [skill](./skills.md) is for work that needs judgment. A one-off needs neither.

## The Functions page

Open **Functions** in the agent's column (beside Skills). Functions are grouped by category, and each one shows its version, who wrote it, its last change, and **needs attention** when it calls a tool the agent no longer has or its name is now taken by another tool.

You need the **Manage functions** permission to change anything (`all_agents:manage_functions` for every agent, or `owned_agents:manage_functions` for agents you own; **Settings → Roles**, group Functions). Anyone who can view the agent can read its functions. With it you can:

- switch a function off or on (one you switched off stays off until you switch it on);
- edit it, which saves a new version under your name;
- roll back to an earlier version, or delete it (the deletion stays in the history);
- rename, redescribe or move a category.

Every change is a new version, with a diff against the previous one. An edit you make does not run the function.

## Rules worth knowing

- A function's name cannot be the name of one of the agent's other tools. If a tool added later (a new MCP server, say) takes a function's name, the tool keeps it and the function is marked **needs attention** until it is renamed.
- Functions run as part of the turn that calls them, with the same tools, the same time limit and the same person to ask, and an unattended turn can call them too.
- There are no limits yet on the number or size of functions beyond the sandbox's 64 KiB script cap, and no extra security checks beyond those of any script.

## HTTP

`GET|PATCH|DELETE /api/v1/agents/{id}/custom-tools[/{name}]` and `/api/v1/agents/{id}/custom-tool-categories/{name}`. There is no create or run route: agents create functions, people govern them. See the [REST API](../api-integration/rest-api.md).
