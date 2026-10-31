# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Two primary audiences, weighted equally:

- **Solo self-hoster.** A technical person running Vizier for themselves. They open the WebUI to chat with their agents, shape an agent's identity (CORE.md), memory, skills and scheduled tasks, and check usage and cost. Usually one operator, sometimes with a few invited people.
- **Small-team or community operator.** An admin running agents for other people, often reached through Discord or Telegram. They manage users, roles and permissions, agent ownership and sharing, integrations (channel bots, addons, MCP servers), providers, and spend.

Both are comfortable with self-hosting, API keys and model/provider concepts. Neither should have to drop to YAML or the CLI for anything the WebUI covers.

## Product Purpose

Vizier is a self-hosted AI agent framework: one Rust binary that runs many concurrent agents. Each agent has its own provider, tools, memory and identity, and is reachable over Discord, Telegram, REST, WebSocket and the bundled WebUI. The WebUI is where people talk to agents and where every piece of runtime configuration lives: agents, providers, users and roles, integrations, skills, tasks, memory and usage. Agents are never defined in config files.

It succeeds when a person can stand up a capable, persistent agent with no external services, keep it working unattended (scheduled tasks, dream cycle, background jobs), and understand at any point what each agent knows, can do, and costs.

## Positioning

Vizier foregrounds four things together:

1. **Agents with a persistent self.** Every agent has a CORE.md identity, long-term memory stored as open-format markdown bundles with a link graph and passage-level search, and a dream cycle that reflects on recent sessions and folds what it learns back into memory and CORE. Revision history is kept for both. Agents remember and grow, not just respond.
2. **Single binary, no services.** One binary with embedded SQLite. It runs with no config file (config-less mode), in Docker or bare, with nothing else to install.
3. **Many agents, many channels.** Concurrent, isolated agents. Each has its own MCP clients, shell, provider and integrations, and its own presence on Discord, Telegram and the WebUI. Agents can consult and delegate to each other.
4. **Open and inspectable.** Memory is human-readable markdown on disk. Bundles export and import as .zip. The project is MIT-licensed and has no lock-in.

## Operating Context

- Deployed on the operator's own machine or server (local, Docker, systemd), served at `http://localhost:9999` by default. First run is: create the first user, add a provider key under Settings › Providers, create an agent.
- Conversations happen in the WebUI chat (sessions and topics, streamed replies, tool calls, background-job tray, reactions, session files, interactive client-rendered pages) and equally in Discord and Telegram. The WebUI has to show what happened in turns that ran elsewhere or unattended.
- Agent work runs in code mode: the model writes Python scripts in a sandbox that call tools by category. Tool activity is a normal, visible part of a turn.
- Recurring rituals: scheduled tasks and their run reports, dream-cycle journals, memory graph browsing and editing, checkpoints and handovers near the context limit, usage and cost review.
- Slash commands (`/checkpoint`, `/lobotomy`, `/abort`, `/skills`, `/<skill>`) work the same across channels.

## Capabilities and Constraints

- WebUI routes today: home, chat, onboarding, login, agent creation and settings, agent CORE, agent dream, memory, skills, tasks, usage, and integrations (channels, addons, MCP) under settings.
- Stack: React Router v7, React 19, TypeScript, Tailwind v4, Zustand-style stores, Recharts, MDX editor. The built WebUI is embedded in and served by the Rust binary, so the frontend ships with each release and cannot rely on a separate host or CDN at runtime.
- Multi-user: JWT plus API keys, roles with granular permissions, agent ownership and sharing. Integration access depends on permissions (`all_agents` vs `owned_agents`), never on who created the integration.
- Providers come from the models.dev catalogue (29+ including Ollama, llama.cpp, OpenAI, Anthropic, Google, OpenRouter, ChatGPT OAuth, GitHub Copilot), plus operator-defined OpenAI-compatible or Anthropic-protocol custom providers. Cost is shown as an estimate.
- Terminology to keep: agent, CORE.md, memory bundle, passage, dream cycle, skill, task / task run, session / topic, integration (channel, addon, MCP server), grant, checkpoint, background job, code mode.
- Agent failures are visible states (`Failed`, retrying with backoff, channel reader `failed`), not silent ones. The UI should surface health and never imply an agent is working when it isn't.

## Brand Commitments

- **Name:** Vizier.
- **Tagline (binding):** "A digital steward for Your Majesty." Keep it as written (adopted 2026-10-21, replacing "21st Century Digital Steward; Right-hand agent for you majesty").
- **Subtitle:** "AI agents that remember who they are, and who you are. Hosted by you." Sits under the tagline wherever there is room (README, docs, social card, hero).
- **Voice (binding):** the steward persona. Vizier is a loyal, capable right hand serving its "majesty", the person running it. Copy may lean into this court/steward register lightly, but it never gets in the way of clarity in operational UI: errors, settings and destructive actions stay plain and precise.
- **Assets:** `webui/public/vizier-logo-light.svg`, `webui/public/vizier-logo-dark.svg`, `webui/public/favicon.ico`; integration icons in `assets/integrations/`; provider logos in `assets/catalogue/`.

## Evidence on Hand

- Product description and feature list: `Readme.md`, `docs/src/introduction.md` (mdBook docs under `docs/`), `DOCKERHUB.md`, `CHANGELOG.md`.
- Feature specs: `specs/` (e.g. 004 memory open format, 007 code mode, 012 background jobs, 016–017 integrations, 018 usage accounting, 019 client tools, 021 user-invokable skills).
- An offline `dummyplug` provider for demos and end-to-end checks without API keys.
- No testimonials, user counts, customer names, benchmarks, pricing or hosted-service claims exist. Future work must not invent any. The project is free and MIT-licensed. The installer is `https://get.vizier.rs`, and Docker images are `blinfoldking/vizier` and `ghcr.io/vizier-lab/vizier`.

## Product Principles

1. **The agent is someone, not a widget.** Identity, memory and growth (CORE, bundles, dreams, revisions) are first-class, and the UI makes them visible and editable.
2. **Everything inspectable.** What an agent knows, what tools it used, what ran unattended, what it cost: a person can always see it and trace it back. Nothing important stays hidden in logs.
3. **Serve one and many equally.** The solo self-hoster gets a fast, direct path, and the team operator gets clear ownership, permissions and per-agent scoping, without either one paying for the other's complexity.
4. **Self-contained and honest.** No external services assumed and no hidden lock-in. Failures and degraded states are stated plainly.
5. **A steward's manner.** Capable, loyal and composed. The persona adds character, never ceremony.
