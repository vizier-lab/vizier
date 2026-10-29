# Contract: Provider Credentials, Test and Setup Guide API

**Feature**: `034-aws-bedrock-provider` | **Plan**: [plan.md](../plan.md)

This contract covers the existing provider endpoints and the catalogue listing as they change for `amazon-bedrock`. No new route is added.

## 1. Catalogue listing

`GET /api/v1/catalogue/providers` (and the connected list built from it) gains one field on `ProviderSummary`:

```jsonc
{
  "id": "amazon-bedrock",
  "name": "Amazon Bedrock",
  "kind": "cloud",
  "dialect": "bedrock",
  "fields": ["access_key_id", "secret_access_key", "session_token", "region"],
  "env": ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_REGION", "AWS_BEARER_TOKEN_BEDROCK",
          "AWS_SESSION_TOKEN", "AWS_DEFAULT_REGION"],
  "model_count": 200,
  "guide": {
    "intro": "Use an IAM access key pair with permission to invoke Bedrock models.",
    "steps": [
      {"title": "Create an IAM access key", "body": "IAM → Users → Security credentials → Create access key. Choose “Application running outside AWS”. The secret is shown once."},
      {"title": "Attach the minimal policy", "body": "Allow the actions below. bedrock:ListFoundationModels is used by Test; bedrock:InvokeModel runs chat, embeddings and images."},
      {"title": "Enable model access", "body": "Bedrock → Model access: enable the chat, embedding and image models you will use. Anthropic models may need a use-case form."},
      {"title": "Choose a region", "body": "Use a Bedrock region where the model is offered, e.g. us-east-1. Cross-region ids (us., eu., apac., global.) are accepted as model ids."},
      {"title": "Paste the values", "body": "Enter the access key ID, secret access key and region. Add the session token only for temporary credentials."}
    ],
    "policy": {
      "language": "json",
      "code": "{\n  \"Version\": \"2012-10-17\",\n  \"Statement\": [{\n    \"Effect\": \"Allow\",\n    \"Action\": [\"bedrock:InvokeModel\", \"bedrock:ListFoundationModels\"],\n    \"Resource\": \"*\"\n  }]\n}"
    },
    "links": [
      {"label": "AWS IAM users", "url": "https://console.aws.amazon.com/iam/home#/users"},
      {"label": "Bedrock model access", "url": "https://console.aws.amazon.com/bedrock/home#/modelaccess"},
      {"label": "AWS supported models", "url": "https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html"},
      {"label": "Vizier provider docs", "url": "https://www.vizier.rs/docs/configuration/providers#amazon-bedrock"}
    ]
  }
}
```

Rules:

- `guide` is `null` for every provider without a guide. It is static metadata, not user data.
- `fields` is the authoritative list for `PUT` validation and for the generic form.
- `env` is informational; the resolver's actual order is saved row → catalogue env (plus the two Bedrock-only names) → legacy names.
- `model_count`, `logo`, `dialect` and the model picker come from the existing catalogue path once the dialect mapping exists.

## 2. Save credentials

`PUT /api/v1/providers/amazon-bedrock`

```json
{
  "access_key_id": "AKIA…",
  "secret_access_key": "…",
  "session_token": "…",   // optional
  "region": "us-east-1"
}
```

Rules:

- Any omitted field keeps its stored value; `""` clears it (existing `apply` semantics).
- Only the four declared fields are accepted. `api_key`, `base_url`, `endpoint`, `access_token` and `account_id` are refused with the existing `"… does not use …"` / `"does not take a base_url"` errors.
- A save that would leave **exactly one** of `access_key_id`/`secret_access_key` set is refused with the missing field named. Setting only the region (or a session token) before the key pair is allowed; an empty save clears the connection as today, and disconnecting means clearing both key fields (or every field). The resolver applies the pair rule to a stored row and refuses any request with an incomplete pair.
- Secrets are encrypted at rest (R11) before the row is written. `access_key_id` and `region` are stored plainly.
- Saving, changing or deleting restarts the agents that use `amazon-bedrock` through the existing provider-change path; no other agent is touched.

Response: the existing `ProviderResponse` plus:

```jsonc
{
  "id": "amazon-bedrock",
  "has_api_key": false,
  "access_key_id": "AKIA…",
  "has_secret_access_key": true,
  "has_session_token": false,
  "region": "us-east-1",
  "source": "saved" // or "env:AWS_ACCESS_KEY_ID"
}
```

A secret value is never present in any response, log, error or telemetry attribute. `source` follows the existing resolver's origin label.

## 3. Test

`POST /api/v1/providers/amazon-bedrock/test` (body optional, same fields as `PUT`)

- A body overlays the stored values for the test only, as today; a missing body tests the saved connection.
- Bedrock uses the control-plane `ListFoundationModels` with SigV4, a 10–15 s timeout and one attempt (no retries).
- Success: `{"ok": true, "model_count": n}`.
- Failure: `{"ok": false, "status": 403, "error": "<provider body>"}` (the existing `TestResult`); the saved credentials are kept.
- The test verifies the region and credentials without needing a specific chat model. It requires `bedrock:ListFoundationModels` in addition to `bedrock:InvokeModel`; the guide states both.

## 4. Environment variables

| Variable | Field | Order |
|---|---|---|
| `AWS_ACCESS_KEY_ID` | `access_key_id` | 1 |
| `AWS_SECRET_ACCESS_KEY` | `secret_access_key` | 2 |
| `AWS_SESSION_TOKEN` | `session_token` | 3 |
| `AWS_REGION` | `region` | 4 |
| `AWS_DEFAULT_REGION` | `region` | 5 (only when `AWS_REGION` is unset) |
| `AWS_BEARER_TOKEN_BEDROCK` | `api_key` | recognised only to report that Bedrock API keys are unsupported |

The saved row wins field by field; the environment fills what it leaves unset. The first variable used becomes the `source` label.

## 5. Setup-form behaviour

- The guide is rendered above the fields: steps, a `<pre>` policy with a copy action, and the links. It is open for a new connection, collapsed while editing, and remains reachable after a failed Test.
- The form renders the four fields from `fields`; `access_key_id` and `region` are plain text, `secret_access_key` and `session_token` are password inputs with "leave empty to keep the saved one" while editing.
- Saving then tests; a failed test shows the AWS message and a "Keep it anyway" action (existing flow).
- No field is prefilled with a stored secret; the guide never contains a real key.
- The form is the same generic component for every provider; the guide comes from the listing, not a Bedrock branch.
