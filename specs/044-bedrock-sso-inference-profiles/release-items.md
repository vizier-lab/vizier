# Release items: AWS Bedrock over SSO

**Feature**: `044-bedrock-sso-inference-profiles` | **Quickstart**: [quickstart.md](./quickstart.md) | **Plan**: [plan.md](./plan.md) risk 2

This file exists so the one part of the verification that is **not** CI is not mistaken for
verified work. Nothing in it has been run by CI, by a maintainer, or by the implementing agent:
the live Identity Center path needs an Identity Center instance, an assigned account and role, and
Bedrock model access, which no offline run has.

**Status: open release item. Not verified.** Do not tick, quote or ship it as evidence until a
volunteer has filled in the verifier report table in [quickstart.md](./quickstart.md) and attached
it to the release notes.

## Why it cannot be CI

[plan.md](./plan.md) risk 2: the portal requests are implemented against the documented shapes and
are exercised offline against a local stub (`crates/vizier-core/src/llm/sso/tests_support.rs`). Only
a real Identity Center instance can confirm the registration, the device-code round trip, the
account/role listing and the invoke against a real inference profile. A stub proves the client
follows the contract, never that AWS accepts it.

## Items a release must carry

Every row of quickstart §5, unchanged in meaning, with its current state:

| # | Release item | State |
|---|---|---|
| 5.1 | Two distinct Bedrock rows in Settings > Providers, the existing one unchanged | unverified - live |
| 5.2 | Device-code sign-in completes against a real Identity Center | unverified - live |
| 5.3 | Accounts and roles listed, none typed | unverified - live |
| 5.4 | A chat turn on a catalogue model | unverified - live |
| 5.5 | An ARN entry chats through its own profile and is costed from its paired model | unverified - live |
| 5.5b | An unpaired ARN entry chats and shows "pricing not set", never `$0.00` | logic proven offline (T075, §2b.1/§2b.5b); the live UI surface unverified |
| 5.6 | Two entries, two agents, two profiles in console metrics | unverified - live |
| 5.7 | **Test** on an ARN invokes once (billable) and a role without `bedrock:InvokeModel` is refused verbatim | unverified - live |
| 5.8 | Memory embedding and `memory_search` on an SSO-only setup | unverified - live |
| 5.9 | One image generated, no second sign-in | unverified - live |
| 5.10 | `~/.aws` unchanged (`diff -r`) | unverified - live |
| 5.11 | Re-sign-in after expiry; agents resume with no restart | unverified - live |
| 5.12 | Session ended on the AWS side reads as a session error, not a key/model error | message proven offline (unit tests); the live trigger unverified |
| 5.13 | A background task with no session fails fast, no hang | unverified - live |

Also release-blocking, from quickstart §4 (`clean`, `encrypted`, `untouched`): the secret-leak
audit is offline and is tracked as T077, not here. §6.2-§6.3 (existing static-credentials
connection untouched) are offline and are tracked as T073/T074.

## What closes this item

1. A volunteer runs §5 on the release candidate against a real Identity Center instance.
2. They fill in the verifier report table in [quickstart.md](./quickstart.md), including the
   build/commit under test.
3. The filled table is attached to the release notes. Any failed row is a release blocker, not a
   known issue.

Until then this feature ships with the live path **unverified by design**, and the release note
must say so in those words.

