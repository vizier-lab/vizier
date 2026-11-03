# 2.3 Providers

A provider is an LLM backend. Providers are shared across all agents; each agent picks one `provider` + `model` (and optionally a different `dream_provider`/`dream_model` for its dream cycle, and a vision model for read-image).

## The model catalogue

Vizier's list of providers and models comes from the [models.dev](https://models.dev) catalogue: over 200 providers and thousands of models, each with its context window, output limit, capabilities (tool calling, image input, reasoning) and price. Any catalogue provider that speaks a protocol Vizier supports (OpenAI chat or Responses, Anthropic, Gemini, Ollama) can be used — adding a provider needs no code change.

- **Offline by default**: a snapshot of the catalogue is built into the binary, so a fresh install works without network access.
- **Kept current lazily**: when a person opens the provider pickers or Settings → Providers, Vizier checks models.dev in the background (at most once per `catalogue.ttl`, with a conditional request) and keeps the result in SQLite. Agents, scheduled tasks and dreams never trigger a check, and startup makes no request. Settings → Providers shows the catalogue's date and has a **Refresh now** button.
- **Turn it off** with `catalogue.refresh: false` (or `VIZIER_CATALOGUE_REFRESH=false`); the built-in snapshot is then used until the next upgrade.

```yaml
vizier:
  catalogue:
    refresh: true   # default
    ttl: 24h        # default; how often a person's visit may trigger a check
```

## Connecting a provider

- **WebUI**: Settings → Providers (requires the `settings:providers` permission). Search for a provider, enter its key, **Test**, **Save**. Agent editors can also connect a provider inline from the agent form's model picker.
- **Environment variables**: the catalogue's variable for the provider (e.g. `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GROQ_API_KEY`) is picked up automatically. Settings → Providers lists these as *From env NAME*.
- **API**: `GET /api/v1/providers` (connected providers, with where their key comes from and which agents use them), `PUT|DELETE /api/v1/providers/{id}`, `POST /api/v1/providers/{id}/test`. The `PUT` body accepts only the fields the provider uses (`api_key`, and for some `endpoint`, `base_url`, `access_token`, `account_id`); responses never return a secret.

Credentials are resolved in this order: the saved entry, then the catalogue's environment variable, then the environment variable Vizier read before the catalogue existed (see the table below). Saving or removing a provider's credentials restarts the agents that use it, so the change takes effect immediately. Saved API keys and access tokens are encrypted at rest; the workspace key is separate from the database and must be backed up — see [Encryption at Rest](./encryption.md).

In the agent model picker, local servers such as Ollama and llama.cpp are marked connected only when a base URL is saved or supplied by their environment variable. A built-in default address alone does not mean a server is running; use **Test** to check reachability.

A cloud provider's address always comes from the catalogue; to point at another address (a proxy, a self-hosted server) add a **custom provider**.

> **YAML `providers` is gone.** Earlier versions seeded providers from a `providers:` section in `.vizier.yaml`. It is now ignored; Vizier logs a warning naming the environment variable to set instead. Credentials saved by earlier versions were migrated automatically on upgrade.

## Custom providers

For a self-hosted model (vLLM, LiteLLM, LM Studio, a llama.cpp server) or a gateway, add a custom provider in Settings → Providers → **+ Custom provider**:

1. **Connection**: a name, an id (agents refer to the provider by it; it can't change once saved), the protocol (**OpenAI-compatible** or **Anthropic**), the base URL, an optional API key and optional extra headers. **Test connection** checks it before saving.
2. **Models**: **Fetch from server** lists the server's models; edit each row's context window, output limit and capabilities. Anything left blank is filled from the catalogue when a model with the same id is known there (marked ⓒ). Rows you edit are kept when you fetch again.

API: `GET|POST /api/v1/providers/custom`, `PUT|DELETE /api/v1/providers/custom/{id}`, `POST /api/v1/providers/custom/{id}/fetch-models`, `POST /api/v1/providers/custom/test`. Keys and header values are write-only.

## Built-in providers

| Id | Credentials | Env vars | Notes |
|----|-------------|----------|-------|
| `ollama` | `base_url` | `OLLAMA_BASE_URL` | Local [Ollama](https://ollama.com), default `http://localhost:11434`; its models are listed live |
| `llama-cpp` | `base_url` | `LLAMA_CPP_BASE_URL` | Local [llama.cpp](https://github.com/ggml-org/llama.cpp) server, default `http://localhost:8080` |
| `chatgpt` | `access_token`, `account_id` | `CHATGPT_ACCESS_TOKEN`, `CHATGPT_ACCOUNT_ID`, `CHATGPT_API_BASE` | ChatGPT subscription session (Codex-style), not an API key |
| `elevenlabs` | `api_key` | `ELEVENLABS_API_KEY` | TTS/STT only |
| `voyageai` | `api_key` | `VOYAGE_API_KEY` | Embeddings only |
| `hyperbolic` | `api_key` | `HYPERBOLIC_API_KEY` | TTS and image generation |
| `dummyplug` | `base_url` | `DUMMYPLUG_URL` | Local test server; default `http://127.0.0.1:9998/v1` |

`dummyplug` is a testing provider, not a connected provider by default. It appears in Settings → Providers only when an agent uses it, a base URL has been saved, or `DUMMYPLUG_URL` is set. Deleting a saved base URL removes its row unless an agent still uses it or the environment variable still supplies a connection. The sample Compose setup does not set `DUMMYPLUG_URL`; enable its `dummyplug` profile and set the URL explicitly when testing.

Every other provider comes from the catalogue.

## Amazon Bedrock

Amazon Bedrock is a catalogue provider (`amazon-bedrock`, **Amazon Bedrock**) for chat, memory embeddings and image generation. One connection supplies all three capabilities; changing it (keys or region) affects every Bedrock agent and no agent on another provider. Speech is not offered: Bedrock has no batch text-to-speech or speech-to-text API, so use Amazon Polly and Amazon Transcribe directly, or the existing TTS/STT providers.

### Connect

1. **Create an IAM access key.** IAM → Users → Security credentials → Create access key. Choose *Application running outside AWS*. The secret is shown once.
2. **Attach the minimal policy.** Allow the actions below. `bedrock:ListFoundationModels` is used by **Test**; `bedrock:InvokeModel` runs chat, embeddings and images.

   ```json
   {
     "Version": "2012-10-17",
     "Statement": [{
       "Effect": "Allow",
       "Action": ["bedrock:InvokeModel", "bedrock:ListFoundationModels"],
       "Resource": "*"
     }]
   }
   ```

3. **Enable model access.** Bedrock → Model access: enable the chat, embedding and image models you will use. Anthropic models may need a use-case form.
4. **Choose a region.** Use a Bedrock region where the model is offered, e.g. `us-east-1`. Cross-region ids (`us.`, `eu.`, `apac.`, `global.`) are accepted as model ids.
5. **Paste the values.** Enter the access key ID, secret access key and region. Add the session token only for temporary credentials.

The same steps appear in the connect form, with a copy button for the policy ([AWS IAM users](https://console.aws.amazon.com/iam/home#/users), [Bedrock model access](https://console.aws.amazon.com/bedrock/home#/modelaccess), [AWS supported models](https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html)).

### Credentials and region

| Field | Secret | Environment variables |
|-------|--------|-----------------------|
| `access_key_id` | no (an identifier) | `AWS_ACCESS_KEY_ID` |
| `secret_access_key` | yes, encrypted at rest | `AWS_SECRET_ACCESS_KEY` |
| `session_token` | yes, encrypted at rest | `AWS_SESSION_TOKEN` (temporary credentials only) |
| `region` | no | `AWS_REGION`, then `AWS_DEFAULT_REGION` |

The saved connection wins field by field; the environment fills what it leaves unset, and the provider list shows the source (`saved` or `env:AWS_ACCESS_KEY_ID`). Both halves of the key pair are required: an incomplete pair is refused when saving and at build time, naming the missing field. A Bedrock **API key** (bearer token, `AWS_BEARER_TOKEN_BEDROCK`) is not supported; the error names the access-key variables instead. `base_url` is refused — a Bedrock connection always uses the regional endpoint derived from the region, and an operator who needs another address uses a custom provider.

There is **one region per saved connection**; switching regions is a settings change and restarts only the Bedrock agents. If no region is set anywhere, the build fails naming the setting and both environment variables instead of guessing. There is no AWS credential-chain support (profiles, roles, instance metadata): only a static access key pair.

Requests are signed with AWS Signature Version 4 (service `bedrock`); the access key pair never travels in a header or query parameter.

### Models

Chat models come from the catalogue — their ids, names, context windows, output limits, tool/vision capabilities and prices are used unchanged. The picker lists all 200 catalogue entries, cross-region profiles included; a typed id the picker does not list is stored and sent exactly as entered, and the provider decides whether it is valid.

Embeddings (`embedding.provider: bedrock`) and image generation (`image_gen_settings.provider: bedrock`) use the same saved connection and region. See [Tools & Embedding](./tools-embedding.md#amazon-bedrock) for their model families, defaults and dimensions.

### Errors and recovery

The provider's own error is kept and shown:

| What happened | What it looks like | What to do |
|---|---|---|
| Missing region | Build error naming the region setting, `AWS_REGION` and `AWS_DEFAULT_REGION` | Save a region in Settings → Providers or set the variable |
| Incomplete key pair | Error naming the missing field | Save both halves together |
| Bearer-only configuration | "Bedrock API keys are not supported" | Set `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` |
| Wrong region / DNS | Transport error naming the `bedrock-runtime.<region>` host | Check the region value |
| Model not enabled or not offered in the region | 403 `AccessDeniedException` or 404 `ResourceNotFoundException` body | Enable model access, or choose a model/region that offers it |
| Missing IAM permission | 403 body naming the action | Attach `bedrock:InvokeModel` (and `bedrock:ListFoundationModels` for Test) |
| Expired session token | 403 body naming the expiry | Save a new session token or a permanent key pair |
| Bad model or parameters | 400 `ValidationException` body | The model states what it accepts |
| Throttling / unavailable | 429/503, retried with backoff | Wait; the existing retry policy applies |

Rotating keys, changing the region or removing the connection restarts exactly the agents that use Bedrock; their next turn uses the new values without a Vizier restart. Invalid credentials make the agent's health report the failure, and saving working credentials recovers it. Startup, catalogue refresh, the scheduler and dream cycles never make an AWS request.

## Amazon Bedrock (SSO)

`amazon-bedrock-sso` (**Amazon Bedrock (SSO)**) is a second Bedrock provider that signs in through your organisation's IAM Identity Center instead of holding an access key pair. Vizier runs the SSO device flow, asks the portal for a role credential for the account and role you pick, and renews that credential itself: your password never reaches Vizier and no long-lived AWS key is stored. Chat, embeddings and image generation all use this one connection, exactly as they do on `amazon-bedrock`.

### Connect

1. **Find the Identity Center start URL** — the portal URL your organisation gave you, `https://<subdomain>.awsapps.com/start`.
2. **Choose the SSO region** — where the Identity Center instance lives. It is often *not* the region your models run in.
3. **Sign in and pick account + role** — press **Sign in**, confirm the code in a browser on any machine, then choose the account and the role you were given.
4. **Attach the minimal policy** — `bedrock:InvokeModel` on the role is all a signed-in connection needs. Listing permissions are optional: the model list comes from the catalogue, not from your account.
5. **Add your inference profiles** — paste each ARN, name it, and pick the catalogue model it runs so its usage is costed (see below).
6. **Test** — one small billable call to the model or profile the test names, so a bad policy or a missing model shows up here, not mid-conversation.

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["bedrock:InvokeModel"],
    "Resource": "*"
  }]
}
```

The same steps and policy appear in the connect form, with a copy button ([IAM Identity Center](https://console.aws.amazon.com/singlesignon/home), [Bedrock model access](https://console.aws.amazon.com/bedrock/home#/modelaccess), [Application inference profiles](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-profiles.html)).

### The connection's fields

| Field | What it is |
|-------|------------|
| `start_url` | The Identity Center portal URL |
| `sso_region` | The region of the Identity Center instance |
| `account_id` | The account picked at sign-in |
| `role_name` | The role picked at sign-in |
| `region` | The **Bedrock** region, where the models are invoked |
| `sso_profile` | The session's readable name: the non-secret handle that identifies this connection in lists, errors and logs |

The last is not a credential. It identifies one stored session among several to the same account and role, and it is the only part of the session a log line may name. The session itself (access token, refresh token, expiry) lives only in Vizier's encrypted storage, keyed to this connection and readable only through the workspace key; it is never returned by the API, written to a log or drawn in the UI.

### Two regions

`sso_region` and `region` are different settings and are usually different values. The **SSO region** is where Identity Center authenticates you; the **Bedrock region** is where the model runs and where the regional `bedrock-runtime.<region>` endpoint is derived from. Using `us-east-1` for one and `eu-central-1` for the other is normal. A mismatch shows either as a sign-in failure (wrong SSO region) or as a 403/404 from the model (wrong Bedrock region) — see the table below.

### Session lifecycle

- **Renewal**: role credentials are short-lived. Vizier renews them from the stored session five minutes before they expire, so a healthy connection keeps working with nobody present.
- **Expiry**: when the Identity Center session itself lapses, the connection reports *expired* and the form asks for a code and nothing else — the saved fields are kept, so signing back in is one click and one code.
- **Sign out**: **Sign out** drops the stored session and its role credentials but keeps the connection fields. Deleting the connection removes both.

### Inference profiles and pricing

A model you use over Bedrock SSO is usually an **application inference profile**: an ARN that belongs to your account, so it cannot come from the catalogue. Add it in the provider's **Models** drawer — paste the ARN, name it, and optionally set its context window, output limit, tool-calling and image-input capabilities. The ARN is stored and sent exactly as entered; Vizier does not validate it and does not take its limits from it.

An ARN's cost is not in the catalogue either, so the entry carries an optional **priced-by pairing**: one model chosen from the catalogue's Bedrock chat list when the entry is added. Its **price** is re-resolved on every display and every use, and it is what costs the entry's usage. Its **two limits are filled once, at the moment you pick it**: a context window or output limit you left empty takes the catalogue row's number for that model, so picking `us.xai.grok-4.7` fills both at 500 000 rather than asking you to type it twice. A number you typed yourself is never overwritten — not by a later re-pick, not by a catalogue refresh, not by saving the form again — and clearing a field returns it to *unknown* and leaves it there. If the catalogue later drops the model you picked, the limits stay as they were (a window that was resolved does not stop being true) and only the price falls to *pricing not resolved*. The pairing never supplies the tool-calling or image-input flags, and it does not validate the ARN. An entry with nothing picked still chats and is shown as *pricing not set* with the reason beside the label rather than a bare flag, because the reason is derived when it is read and so stays accurate. Removing an entry names the agents chatting through it first: they stop working until they are given another model.

### When the session ends

An ended session is an ordinary failure, not a crash. The turn fails with a message naming the connection and telling you to sign in again (`SESSION_ENDED`), the agent reports that connection as its health cause, and results already produced in the turn are kept — no half-written assistant message is stored. Background work (scheduled tasks, dreams, memory embeddings) that reaches an ended session reports the same cause instead of retrying blindly. Signing in again restores every agent on the connection, without a Vizier restart.

Each failure reads differently enough to act on:

| What happened | What it looks like | What to do |
|---|---|---|
| Sign-in denied, or code expired | The panel says so and offers **Sign in** again | Start again; the code is short-lived |
| Session expired | *Session ended — sign in again*, naming the profile | **Sign in** again |
| Role not assigned | The portal lists no such account or role | Ask for the role, or pick another |
| Missing IAM permission | 403 body naming the action | Attach `bedrock:InvokeModel` to the role |
| Model access not enabled | 403 `AccessDeniedException` | Enable the model in Bedrock → Model access |
| Profile not found, or wrong region | 404 `ResourceNotFoundException` naming the ARN | Check the ARN's region against the connection's Bedrock region |
| Throttling or unavailable | 429/503, retried with backoff | Wait; the existing retry policy applies |

## Ids from earlier versions

Provider ids now follow the catalogue. The old names are still accepted everywhere (API, agent configs) and are normalised on read, and the environment variables Vizier used before are still honoured:

| Old name | Id now | Old env vars still read |
|----------|--------|-------------------------|
| `gemini` | `google` | `GEMINI_API_KEY` |
| `mimo` | `xiaomi` | `XIAOMI_MIMO_API_KEY` |
| `moonshot` | `moonshotai` (or `moonshotai-cn` for `api.moonshot.cn`) | `MOONSHOT_API_KEY` |
| `zai` | `zai` (or `zhipuai` for `open.bigmodel.cn`) | `ZAI_API_KEY` |
| `minimax` | `minimax` (or `minimax-cn`) | `MINIMAX_API_KEY` |
| `together` | `togetherai` | `TOGETHER_API_KEY` |
| `copilot` | `github-copilot` | `COPILOT_API_KEY` |
| `opencode_zen` | `opencode` | `OPENCODE_ZEN_API_KEY` |
| `opencode_go` | `opencode-go` | `OPENCODE_GO_API_KEY` |
| `llama_cpp` | `llama-cpp` | `LLAMA_CPP_BASE_URL` |
| `azure` | `azure` | `AZURE_API_KEY`, `AZURE_ENDPOINT` |
| `custom` | the custom provider `custom` | `CUSTOM_BASE_URL`, `CUSTOM_API_KEY` |
| `galadriel`, `mira` | **removed** | — |

On upgrade, saved credentials and agents are rewritten to the new ids once. An agent on a removed provider shows `provider "galadriel" is not available; pick another provider` until it's moved.

## Dummyplug (testing)

`dummyplug` is a local Node.js OpenAI-compatible chat server for exercising agents, channels, sessions and tools without a live model. Start it with `just dummyplug` (requires Node.js 22), then set an agent's provider to `dummyplug` (shown as **dummyplug (testing)**) and any model name. Its default URL is `http://127.0.0.1:9998/v1`; override it with `DUMMYPLUG_URL` or save a `base_url` through Settings → Providers. It is optional: Vizier and other providers work when the server is stopped, while dummyplug turns report a connection error.

The separate server reads the latest user message and replies by these rules, first match wins:

| Send | Reply |
|------|-------|
| `tools` (any case) | A list of every tool available to the agent, including MCP tools (named `<server>_<tool>`) |
| An exact tool name, e.g. `memory_write` | The tool's description, its required/optional arguments, and a sample request in a `json` code block |
| `{"tool": "<name>", "arguments": { ... }}` (optionally fenced) | Runs the tool through the normal tool path (hooks, history, UI), then replies with a **Tool result** (`<name>`) block holding its output. Malformed JSON or an unknown tool gets an error reply and runs nothing. |
| Anything else | 1–3 paragraphs of random lorem ipsum |

Example:

````text
> memory_read
**`memory_read`** — read one whole memory by `(bundle, path)`

```json
{
  "tool": "memory_read",
  "arguments": { "bundle": "core", "path": "soul" }
}
```

> {"tool": "memory_read", "arguments": {"bundle": "core", "path": "soul"}}
**Tool result** (`memory_read`):
"# Core\n\n## Operating Framework ..."
````

Scheduled tasks, dream cycles and other non-interactive prompts fall into the last rule, so they complete normally. Token usage is always reported as zero.

## Model limits and capabilities

An agent's context window and output limit come from the catalogue entry for its model; Settings shows them on the model card, and `vizier agent ps` shows the effective context window and where it came from. The agent's `context_window` and `max_tokens` fields (under **Advanced** in the WebUI) override them. For a model the catalogue doesn't list, Vizier asks the provider's model listing (OpenRouter, Ollama) and then falls back to detection by model name. The context window drives automatic [checkpoints](./agents.md#checkpoints).

Capabilities are used too: images are sent straight to a model that can see them (read-image needs no separate vision model), and are replaced by a short note for one that can't. Saving an agent on a model without tool calling, or with read-image on a model that can't see images, returns a warning (never an error).

## Related per-agent providers

Embedding, TTS, STT, and image generation each have their own provider choice configured **per agent**; they reuse the credentials of the matching provider here (e.g. Gemini embeddings use `google`'s key). See [Tools & Embedding](./tools-embedding.md).
