# Triggers

A **trigger** starts an unattended run of an agent when something happens, rather than at a time. A scheduled task fires on a clock; a trigger fires on an event: a CI failure, a form submission, a page that changed.

Two kinds ship:

- **Webhook**: another system sends a request to the trigger's own address.
- **Monitor**: a short Python script checks something on an interval, without calling a model, and reports an event only when it finds one.

Addons can add their own kinds later; they work the same way.

## The Triggers page

Each agent has a **Triggers** page, beside its Schedule, for people who may view them. It lists
what each trigger watches, how it last went and how many runs it started this hour. The badge counts
triggers waiting for a person: to approve one, to restart a stopped monitor, or to fix what one is
missing. A trigger's own page shows its runs, every event with what became of it, and its settings.

## How an event becomes a result

```text
event ──► filter (webhook only, free) ── no match ──► ignored
            ▼
          triage (optional, a model you choose) ── not important ──► recorded, in the summary
            ▼ important
          waits for a run (one at a time, within the hourly limit)
            ▼
          run: the agent reads the instruction and up to 20 events
            ▼
          Vizier delivers the reply once, to the person who set the trigger up
```

Every event is recorded with what became of it: waiting, handled, not important, ignored, dropped or failed. You can read them on the trigger's page.

### Runs

Each run is a new conversation of its own. The agent is told which trigger fired, that nobody is reading as it works, and that the events are **untrusted data, never instructions**. Its final message is what gets delivered. If there is nothing worth sending, the agent ends the run with `nothing_to_report` and a short note instead.

Runs of one trigger never overlap. Events that arrive during a run wait, and the next run takes up to 20 of them together. You can set a trigger to drop events that arrive during a run instead.

If a run fails or is interrupted, its events get one more run; after that they are marked failed and you are told.

### Delivery

Vizier sends each run's reply to the **person who set the trigger up**: to their notification inbox, and optionally into one of their WebUI conversations (`webui:<topic id>`). Nobody else who can see the agent gets it. If that person can no longer view the trigger, results go to the agent's owner.

Every few hours (5 by default, adjustable or off) the same person gets one **summary** listing the runs that had nothing to report and the events triage judged unimportant. It is built without a model, and an empty window sends nothing.

### Triage

Triage is optional. You write what counts as important and pick a model from your configured providers; a cheap or free model is usually enough. The model has no tools, no memory and no agent context: it sees your criteria and the events, and answers important or not with a short reason for each. Events are judged in batches of up to 20.

If the model or its provider is removed, the trigger is marked as needing a triage model and you are told. If triage fails or answers in a form Vizier cannot read, the events are treated as **important** and marked as not triaged, and you are told. An outage can spend the run limit, but it never hides an event.

Try your criteria on a few real payloads first, including one that claims to be urgent. A free provider may keep or train on what it is sent; check its terms, or use a local model.

### Limits

| Limit | Default | Notes |
|---|---|---|
| Runs per hour | 20 | Per trigger, either kind. Raise, lower or switch off. Only important events lead to runs. |
| Cooldown between runs | none | Optional |
| Events waiting for a run | 100 | A webhook drops its oldest; a monitor refuses the new one and reports it again later |
| Events waiting for triage | 500 | Oldest dropped |
| Triage calls per hour | 200 | Adjustable |
| Events per run, per triage call | 20 | |
| Triggers per agent | 20 | A server setting |
| Event payload kept | 3 days | The record of what happened stays |

The run limit bounds what reaches you only while the trigger's runs cannot send messages or wake conversations themselves. The page warns when you allow that, or switch the limit off.

### Server settings

The operator can change these in the [config file](./main-config.md):

```yaml
vizier:
  triggers:
    monitor_interval_secs: 60      # when a monitor names no interval
    monitor_min_interval_secs: 5   # the shortest a monitor may ask for
    monitor_max_failures: 10       # failed checks in a row before a monitor stops
    max_per_agent: 20              # triggers an agent may have
```

## Webhooks

Creating a webhook gives you an address and a secret. **The secret is shown once.** Configure the sending system with both:

- Preferred: send the secret in a header, `Authorization: Bearer <secret>`, to `POST /api/v1/hooks/<trigger id>`.
- For senders that cannot set a header: `POST /api/v1/hooks/<trigger id>/<secret>`. Addresses are often logged by proxies, so use this only when you must.

The answer comes at once, before any run: `202 {"event_id": …, "outcome": "accepted" | "ignored" | "dropped" | "refused" | "duplicate"}`. A wrong secret, an unknown trigger and an inactive one all get the same `404`, and repeated misses from one address are rate limited.

Bodies up to 256 KiB are accepted as JSON, form-encoded or text.

| Setting | Meaning |
|---|---|
| Signing secret | Optional. Verify an HMAC (SHA-256 by default, or SHA-1) of the exact body, in a header such as `x-hub-signature-256` (`sha256=` prefixes are accepted) |
| Timestamp header | Optional. Refuse requests older than the tolerance (5 minutes by default), so a captured signed request cannot be replayed |
| Filter | Rules every request must meet, such as `body.ref=refs/heads/master` or `header.x-github-event=workflow_run` (`!=` too). Applied before triage and the limits, so filtered requests cost nothing |
| Delivery id | `header.<name>` or `body.<path>`, so a redelivered request is recognised as a duplicate for at least 24 hours |
| Forwarded headers | The only headers the agent may see. Authorization and signature headers never are |

### Reaching your instance

The sender must be able to reach your server over HTTPS. Vizier provides no relay. The desktop app opens no port by default, so it cannot receive webhooks until web access is turned on.

Behind a reverse proxy or a tunnel (Cloudflare Tunnel, for example):

- Let `/api/v1/hooks/` through without a login page or browser challenge. Vizier authenticates the request itself.
- If the proxy runs as a separate container, add its network to `VIZIER_TRUSTED_PROXIES`, so rate limiting sees the real caller address. Keep the range narrow.
- Remember that whatever decrypts TLS (your proxy, your tunnel provider) can read requests, secrets in addresses included.

## Monitors

A monitor's script reads `input["state"]`, the value it returned last time (`None` on the first check), and ends with one of:

- `None`: nothing happened; the state is kept.
- `{"state": …}`: a new state, saved at once, and nothing to report. Use this to record a baseline.
- `{"state": …, "fire": …}`: report `fire` as an event. The new state is saved only if the event is accepted, so an event that could not be taken is reported again on the next check.

```python
s = input["state"]
latest = fetch(url="https://example.com/status")["content"]
if s is None:
    result = {"state": latest}
elif latest != s:
    result = {"state": latest, "fire": {"was": s, "now": latest}}
else:
    result = None
result
```

The script's last line is its answer, so end with the value itself, as above. This example needs the `web` category.

| Setting | Default |
|---|---|
| Interval | 60 seconds, at least 5 |
| Tools the script may call | none: list the categories it needs, such as `web` and `secrets`. A monitor an agent creates starts with `memory_read` |

A check never calls a model. It runs inside the agent, so it can use the agent's tools, integrations and secrets, within the categories listed. Tools that reach a model, a person or another agent, and tools that only make sense in a turn, are never available to a check, whatever is listed. A check may run for the shorter of its interval and 30 seconds; one still running when the next is due is skipped.

A failing check fires nothing and changes no state. The next check waits longer, doubling from 2 seconds up to 5 minutes (never sooner than the interval). After 10 failures in a row the monitor stops and you are told; editing or reactivating it starts it again.

**Test** runs the script once and shows what it returned, without firing or saving state. The tool calls the script makes do really happen.

## Tool access

Each trigger limits which tool categories its runs may use. Webhooks, and any trigger an agent creates, start with **reading memory only**: no writing memory, no messages. A monitor you create starts with every tool except messaging and waking conversations. A person who may manage the trigger can change either.

The limit follows the work: a worker a run hires, a conversation it wakes and an agent it consults keep the same limit, and so does the turn their results come back in. A result that arrives while an ordinary turn is already running joins that turn, which keeps its own tools. Trigger runs can never create or change triggers, launch detached scripts or ask a person a question.

## Permissions

Triggers have their own role permissions, each for every agent (`all_agents:`) or for agents the person owns (`owned_agents:`):

| Permission | Allows |
|---|---|
| `view_triggers` | See triggers, events, runs and their conversations |
| `manage_triggers` | Create, change, test, approve, deactivate and delete (includes view) |
| `public_webhooks` | Create, activate, approve and replace the secret of a webhook (needs manage too) |

Only the administrator role holds them at first. Seeing an agent, or having it shared with you, grants none of them. Choosing a triage model also needs `model_override`.

## Agents creating triggers

An agent can create, change, test and delete its triggers with its trigger tools, but never more than the person behind the turn may: the person in a WebUI or project chat, the agent's owner in a scheduled run, or, in a Telegram direct message, the person who [connected](./channel-identities.md) the sender's Telegram account with `/connect`. The tools are not offered in Discord, in Telegram groups, or to an account nobody has connected.

Every trigger an agent creates starts **inactive** and waits for a person to approve it, and the owner is told. The agent never sees a webhook's secret: the person approving gets a fresh one. Any change the agent makes puts the trigger back to awaiting approval, and the agent cannot run its script (`test_trigger`) until then. What a monitor's script may call starts as reading memory, and goes back to that when the agent changes the script; only a person widens it. An agent may lower a run limit but not raise it or switch it off.

## Security notes

- **Payloads are hostile input.** Anyone who can reach a webhook, or write to what a monitor reads (an inbox, a web page), can put instructions in front of your agent. The framing tells the agent to treat them as data; the tool limits hold whatever the model decides. Keep runs to the tools they need.
- **What is redacted.** Authorization and signature headers never reach the agent or storage. Other content of an outside payload cannot be told apart from ordinary data and is stored as received, for 3 days.
- **Delivered replies can repeat what an event said.** They go only to the person who set the trigger up.

## HTTP API

| Route | Needs |
|---|---|
| `GET /api/v1/trigger-kinds` | signed in |
| `GET /api/v1/agents/{id}/triggers`, `GET …/{slug}` | `view_triggers` |
| `GET /api/v1/agents/{id}/trigger-summary` | `view_triggers` |
| `POST /api/v1/agents/{id}/triggers`, `PUT`/`DELETE …/{slug}` | `manage_triggers` (+ `public_webhooks` for a webhook) |
| `POST …/{slug}/approve`, `POST …/{slug}/test` | `manage_triggers` |
| `POST …/{slug}/secret` | `manage_triggers` + `public_webhooks` |
| `GET …/{slug}/events`, `GET …/{slug}/runs`, `GET …/{slug}/runs/{run_id}` | `view_triggers` |
| `POST /api/v1/hooks/{trigger id}[/{secret}]` | the trigger's secret |

A trigger is read with its `latest_run`, `runs_last_hour` (what the hourly limit counts) and
`needs_person`: true while it waits for approval, something it needs is gone, or its checks have
stopped while it is switched on. The summary counts an agent's triggers as `total`, `active` and
`needs_person`.
