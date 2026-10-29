# Quickstart: AWS Bedrock Provider

**Feature**: `034-aws-bedrock-provider` | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

Validation runs in two parts: **offline** checks that need no AWS account (§1–§4), and **live AWS** checks that exercise real Bedrock (§5–§6). Live steps are marked **[live]** and are run by a **volunteer verifier on the special release** — the maintainers have no Bedrock account, so §5–§6 are a release checklist, not a CI job. Never commit the keys used for them.

## Verifier report

The volunteer fills this in when the special release is tested; attach it to the release notes or the tracking issue.

| Item | Result / notes |
|---|---|
| Build/commit tested | |
| AWS region | |
| Chat model(s) enabled | |
| `bedrock:InvokeModel` + `bedrock:ListFoundationModels` policy in place | |
| §5.2 Test returned a model count | |
| §5.3–§5.4 chat and multi-turn | |
| §5.5 tool call | |
| §5.6 image input (vision and text-only) | |
| §5.7 usage/cost | |
| §6.1–§6.2 embeddings and memory search | |
| §6.3 wrong embedding model error | |
| §6.4–§6.5 image generation and size handling | |
| §6.6 cross-region model id | |
| §6.7 negative credential/region/model-access checks | |
| Anything that failed, with the error text | |

## Prerequisites

- Rust toolchain and the repository built (`just install` once, then `cargo build`).
- Node.js 22+ for the WebUI checks.
- **[live]** An AWS account with Bedrock available in a region where the chosen models are offered.
- **[live]** An IAM user with the policy below and an access key. The policy is the one the connect form shows:

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

- **[live]** Model access enabled in Bedrock → Model access for at least one chat model, `amazon.titan-embed-text-v2:0` and `amazon.nova-canvas-v1:0`. Anthropic chat models may require the use-case form.

## 1. Offline: build and unit tests

```sh
cargo test
cargo clippy
cd webui && npm test && npm run typecheck
```

Expected: all pass. The SigV4 tests include the AWS test vectors and the model-id encoding cases (versioned ids, cross-region profiles, ARNs); the dialect tests pin the Converse request/response mappings; the media tests pin Titan/Cohere and Nova/Titan bodies; the indexer test pins the dimension-mismatch error.

## 2. Offline: provider listing, form guide and credential API

Start Vizier with a temporary workspace (no AWS configuration):

```sh
cargo run -- run --config dev.vizier.yaml
```

In the WebUI, open **Settings › Providers** and search for **Amazon Bedrock**.

Expected:

- Amazon Bedrock is listed as connectable (not hidden/unsupported) with the Bedrock logo and the catalogue's model count.
- The connect form shows **How to connect** with five steps, the copyable policy (`bedrock:InvokeModel` + `bedrock:ListFoundationModels`), and the four links (IAM, model access, supported models, Vizier docs).
- The fields are **Access key ID**, **Secret access key**, **Session token (optional)** and **Region**; the first and last are plain text, the two secrets are masked.
- Clicking **Copy** copies the policy exactly; the guide remains visible after a failed Test and while editing.
- No browser network request to AWS is made by opening the form.

Using the API with an authenticated session (the WebUI's token or an API key):

```sh
curl -s http://localhost:9999/api/v1/catalogue/providers | jq '.data[] | select(.id=="amazon-bedrock") | {fields, guide}'
curl -s -X PUT http://localhost:9999/api/v1/providers/amazon-bedrock \
  -H 'content-type: application/json' \
  -d '{"access_key_id":"AKIAEXAMPLE","secret_access_key":"secret","region":"us-east-1"}'
curl -s http://localhost:9999/api/v1/providers/amazon-bedrock | jq '{has_secret_access_key, access_key_id, region}'
```

Expected: the listing has the four fields and the guide; the PUT saves; the GET shows `access_key_id` and `region` but no secret (`has_secret_access_key: true`). Restart Vizier and confirm the secret still exists (it is encrypted and decrypts with the workspace key).

## 3. Offline: credential and error checks

- PUT with `{"access_key_id": "AKIA…"}` only: refused with a message naming the missing secret access key; no row is left half-connected.
- PUT with a complete pair and `{"region": "not-a-region"}`: refused with "invalid AWS region" before any request.
- Remove the region and unset `AWS_REGION`/`AWS_DEFAULT_REGION`, then start: the build names the region setting and both variables.
- Set only `AWS_BEARER_TOKEN_BEDROCK` and start: the build says Bedrock API keys are not supported and names `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`.
- PUT `{"api_key": "x"}`: refused because Bedrock does not use `api_key`.
- Confirm none of the above sends an AWS request (check the process logs; no `bedrock-runtime` host appears).

## 4. Offline: regression and no startup traffic

- With a `dummyplug` agent (see `specs/008-dummyplug-provider/quickstart.md`), send `tools` and a normal message; both still work. Bedrock support changed no existing provider.
- Start Vizier with Bedrock credentials saved but without using an agent for 60 s; confirm no `bedrock`/`bedrock-runtime` request is made by startup, the scheduler or a dream (the catalogue refresh rule is unchanged).
- Confirm `vizier agent ps`/health shows the failed build with the actionable Bedrock error for a bad region, and no failed state for agents on other providers.

## 5. [live] Connect and chat

1. Create the IAM user and access key as the form describes; save the key pair and `us-east-1` (or the region you enabled) in **Settings › Providers › Amazon Bedrock**.
2. Click **Test**. Expected: success with a model count (`bedrock:ListFoundationModels`); no agent starts.
3. Create an agent with provider **Amazon Bedrock** and a catalogue model you enabled, e.g. `amazon.nova-lite-v1:0` or `anthropic.claude-haiku-4-5-20251001-v1:0`. Send "Hello".
   Expected: a normal reply, stored in history; `vizier agent ps` shows the effective context window and source from the catalogue.
4. Send several turns; confirm the system prompt, CORE and earlier history reach the model (the conversation stays coherent).
5. Ask a question that needs a tool (for example "search your memory for X" or "run a Python script that prints 2+2"). Expected: the tool runs exactly once and the final reply uses its result.
6. Send an image to a vision-capable model (e.g. Claude Haiku 4.5 or Nova Lite if enabled). Expected: the reply refers to the image. Switch to a text-only catalogue model and repeat; expected: the image is replaced by the note and the agent still answers.
7. Open the usage view. Expected: the chat turn appears with input/output tokens and, when the catalogue prices the model, an estimated cost at the catalogue rate. Embeddings/images are not counted as chat tokens.

## 6. [live] Embeddings, images and cross-region ids

1. Edit the agent's embedding settings: provider **Bedrock**, model empty. Save and rebuild. Save a memory containing a unique phrase, then ask `memory_search` for it. Expected: the passage is returned; the build used Titan Text Embeddings V2.
2. Change the embedding model to `cohere.embed-english-v3`, rebuild, reindex if the index dimension check asks for it. Save and search again. Expected: recall works.
3. Set a deliberately wrong model (e.g. `anthropic.claude-haiku-4-5-20251001-v1:0`) as the embedding model. Expected: the build fails naming the model and the supported families.
4. Enable image generation: provider **Bedrock**, model empty, no size. Ask for an image. Expected: a valid PNG appears in the conversation files and is delivered as an attachment; the filename ends in `.png`.
5. Set size `1024x1024` and an explicit model `amazon.titan-image-generator-v2:0`; generate again. Expected: a valid image. Set size `100x100`; expected: the provider's validation error, no file.
6. Reconnect the agent with a cross-region inference profile id from the catalogue (e.g. `us.anthropic.claude-haiku-4-5-20251001-v1:0` or a `global.` entry available in your region). Send a message. Expected: the id is sent unchanged and the reply arrives; an invalid id returns the provider's error for that id.
7. Negative checks: remove the secret access key and start the agent (actionable missing-field error); use a region where the model is not offered (provider error naming the model/region); disable model access for the chosen model (AWS `AccessDeniedException`/model-access message). In every case the agent stays available for other work.

## 7. [live] Docs anchor and cleanup

- Open `https://www.vizier.rs/docs/configuration/providers#amazon-bedrock`; it contains the same five steps, policy, environment variables, supported embedding/image models and defaults, the single-region limitation, and the note that Bedrock has no TTS/STT.
- Remove the Bedrock connection. Expected: agents using it report "not connected" and point to Settings › Providers; agents on other providers are unaffected.
- Delete the IAM access key used for testing, and revoke model access if it was enabled only for this check.

## Expected outcomes (spec mapping)

| Check | Criterion |
|---|---|
| §1 | SC-002, SC-012 (regression), unit coverage for SC-005/SC-008/SC-009 |
| §2–§3 | FR-011a/b, FR-010, SC-003, SC-004, SC-016 |
| §4 | FR-046/SC-013 (no startup AWS traffic), FR-047/SC-012 (no regression) |
| §5 | SC-001, SC-002, SC-003, SC-005, SC-006, SC-010, SC-011, SC-015 |
| §6 | SC-007, SC-008, SC-009, SC-010, SC-015 |
| §7 | FR-024, FR-025, SC-014 |
