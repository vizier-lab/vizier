# Feature Specification: Encrypted Storage of Secrets at Rest

**Feature Branch**: `031-encrypt-agent-secrets`

**Created**: 2026-10-26

**Status**: Draft

**Input**: User description: "currently we save secrets, api keys and such as a plain text, i think we need to change this"

## Context

Vizier persists credentials and secret values that it later reads back: agent secret-store values, saved provider credentials, channel bot tokens, addon API keys, MCP server environment and header values, and credential-like values in shell configuration. All of them are currently written to the embedded data store in clear text. The agent secrets documentation says so explicitly, and the same is true of provider credentials and integration settings. A copy of the data directory — a backup, a synced folder, a support bundle, a stolen disk image, or a database file read by another local account — therefore exposes every credential in full.

This feature makes those stored values unreadable without a key that is not stored alongside the values themselves. It is a protection-at-rest feature, not a change to who may use a secret while Vizier is running: agents keep using their own secrets, people keep managing them through the WebUI and API with the same permissions and masking, and provider, channel and integration behaviour does not change.

The key is managed for the operator by default so a fresh install still works with zero setup and no startup prompt. Operators who need stronger separation (containers, secret managers, separate backup handling) can supply the key themselves. Losing or replacing the key must fail safely and visibly, never silently discard or overwrite stored secrets.

This feature supersedes the explicit "no application-level encryption" decision recorded in `specs/027-agent-secrets/` and closes the corresponding line in `docs/src/configuration/secrets.md`.

The plaintext-to-protected conversion is one-time migration code in the sense of the project's pre-1.0 removal tracker (#82): it exists only to carry pre-encryption data forward. This feature therefore adds an inventory entry for the conversion and its plaintext-compatibility branches, and the plan must name the symbols and what depends on them. The encryption itself, key handling and the versioned value reader are permanent and are not removal candidates. After the last pre-1.0 release, the conversion and its plaintext branches are removed, so an installation must pass through that release before upgrading to 1.0.0.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A copy of the data is no longer a copy of the credentials (Priority: P1)

An operator backs up, syncs or shares Vizier's data directory. Someone who obtains only those data files — without the key — searches them with ordinary text and database tools and finds none of the secrets, API keys, bot tokens or provider credentials that Vizier stored. The same files plus the key behave exactly as before.

**Why this priority**: This is the entire point of the feature. If a data-file copy still reveals values, nothing else in the spec matters.

**Independent Test**: Create an agent secret, save a provider API key, connect a channel, and configure an addon and an MCP server with secret values. Stop Vizier. Copy the data directory without the key, and search the copy for every supplied value; confirm none appears. Restore the copy together with the key and confirm every value is usable again.

**Acceptance Scenarios**:

1. **Given** an agent secret, a saved provider key, a channel token, an addon key and an MCP environment value, **When** the data files are inspected without the key, **Then** none of the supplied values appears in readable form.
2. **Given** the same data files plus the correct key, **When** Vizier starts, **Then** every stored value is returned to its normal consumer exactly as supplied, including whitespace, punctuation and multi-line text.
3. **Given** two secrets with the same value stored for different agents, **When** the stored representations are compared, **Then** they differ, so equal secrets cannot be identified by comparing stored data.
4. **Given** stored data that was altered or truncated outside Vizier, **When** the affected value is read, **Then** the operation fails with a clear integrity error and no partial or incorrect value is returned or used.

---

### User Story 2 - Upgrading an existing installation is automatic and lossless (Priority: P1)

An operator upgrades a running installation that already contains plaintext secrets. They change nothing. On the next start, Vizier converts the existing values in place, and every agent, channel, provider and integration continues to work with the same values. The upgrade leaves no recoverable plaintext copy behind in the data directory.

**Why this priority**: The project has existing deployments. A security fix that requires manual migration, loses values or leaves plaintext in free space only partially solves the problem.

**Independent Test**: Start from a data set created by the previous release with plaintext secrets and no key. Upgrade and start. Confirm every value still resolves and every consumer still works. Stop Vizier, scan the whole data directory (including database journal and free space) for the original values, and find none. Start again and confirm nothing changes.

**Acceptance Scenarios**:

1. **Given** an existing installation with plaintext secrets and no key, **When** the upgraded Vizier starts for the first time, **Then** it creates a key, protects every in-scope value, and starts normally with no operator action.
2. **Given** the upgrade is interrupted partway, **When** Vizier starts again, **Then** it finishes safely: each value is either fully plaintext or fully protected, never corrupted or half-converted.
3. **Given** the upgrade completed, **When** the data directory is searched for the values that existed before it, **Then** no recoverable copy remains, including in database free space and journal files.
4. **Given** an installation upgraded twice or restarted repeatedly, **When** it starts again, **Then** no value is re-encrypted repeatedly, no value changes, and startup shows no growing delay.
5. **Given** a value was written through any surface after the upgrade — agent tool, WebUI, HTTP API, provider save, integration save — **When** it is read back, **Then** it behaves identically to a value written before the upgrade.

---

### User Story 3 - Key handling is automatic, controllable and fail-safe (Priority: P1)

On a fresh install, everything works without the operator creating or entering a key, and nothing prompts at startup so headless and container deployments keep working. An operator who wants to control the key can provide it through the environment or a separate file. If the key is missing, unreadable or does not match the stored data, Vizier refuses to start with a message that says what is wrong and what to restore, and it changes nothing.

**Why this priority**: Encryption with unsafe key handling is either unusable (prompts break unattended startup) or dangerous (a regenerated key silently destroys data). Automatic generation plus explicit failure is the minimum safe behaviour.

**Independent Test**: On a fresh data directory with no key configured, start Vizier and complete a secret round trip. Delete the key and start again: confirm startup is refused with a clear message and the data files are untouched. Restore the key and confirm startup succeeds. Repeat with an operator-supplied environment key, with a wrong key, and with a malformed key.

**Acceptance Scenarios**:

1. **Given** a fresh data directory and no key, **When** Vizier starts, **Then** it generates a key automatically, restricts access to it, and starts with no prompt or extra step.
2. **Given** an operator supplies a valid key through the environment or a key file, **When** Vizier starts, **Then** that key is used instead of the generated one and existing data protected by it is readable.
3. **Given** protected data exists but the key is missing, unreadable, the wrong one, or malformed, **When** Vizier starts, **Then** startup stops with an actionable message, no value is changed, and no replacement key is generated.
4. **Given** a startup failure caused by a missing or wrong key, **When** the operator restores the correct key, **Then** startup succeeds and every stored value is available again.
5. **Given** the key file, **When** it is inspected, **Then** it is not part of the database and its access is restricted to the account running Vizier where the platform supports it.
6. **Given** Vizier logs, diagnostics, telemetry, error messages and management responses, **When** any key-related operation fails or succeeds, **Then** the key itself never appears in any of them.

---

### User Story 4 - Backups, restores and moves between machines are predictable (Priority: P2)

An operator copies an installation to a new host. When the data and key travel together — as the backup procedure says — everything works. When only the data is copied, the values stay protected and Vizier refuses to start until the key is supplied. The documentation states plainly what is and is not protected, so nobody relies on the wrong guarantee.

**Why this priority**: Encryption changes backup and restore obligations. Without explicit guidance, operators will either lose data or believe they are protected when they are not.

**Independent Test**: Copy a data directory and its key to a second machine and start there; confirm all values resolve. Then copy only the data directory and confirm startup is refused rather than starting empty or regenerating a key. Verify the documentation's backup and restore steps exactly as written.

**Acceptance Scenarios**:

1. **Given** a data directory and its key, **When** both are restored on another machine with a compatible installation, **Then** all protected values are usable.
2. **Given** only the data directory, **When** it is restored on another machine, **Then** startup is refused with a clear message and no value is lost or replaced.
3. **Given** a container deployment, **When** the workspace is persisted as documented, **Then** the key persists with it and restarts work; when the key is not persisted, the failure is reported instead of silently regenerating.
4. **Given** the documentation, **When** an operator reads the security section, **Then** it states what the protection covers, what it does not cover (for example a compromised host or running process), and how to back up and restore the key.

---

### User Story 5 - Stored secrets remain hidden everywhere except their intended consumer (Priority: P2)

Protecting values at rest does not weaken any existing guarantee. Values are still masked in the WebUI, never returned by management endpoints, never written to logs, telemetry or audit records, and never included in support output. A failure to decrypt is reported without echoing the ciphertext or the value.

**Why this priority**: A decryption path is a new place where values can leak into errors or diagnostics. It must not undo the non-disclosure guarantees that already exist.

**Independent Test**: Perform every secret operation, including a deliberately corrupted value and a wrong-key startup, and search logs, telemetry, audit records, API responses, WebUI responses and diagnostic output for the stored values and the key; confirm none appears.

**Acceptance Scenarios**:

1. **Given** a stored secret, **When** it is created, replaced, read by its agent, listed or deleted, **Then** no human-facing response, log line, telemetry record or audit row contains the value or its protected representation.
2. **Given** a value that cannot be decrypted, **When** the error is surfaced, **Then** the message names the affected item generically and contains neither the ciphertext nor any plaintext.
3. **Given** an agent retrieves a secret, **When** its turn report, stdout, traceback and nested tool arguments are examined, **Then** the existing redaction behaviour is unchanged.

---

### Edge Cases

- **Fresh install**: a key is generated before any secret is written, so no value is ever stored in plaintext, even briefly.
- **Existing install with no key**: migration generates a key and protects existing values on first start; no value is readable from the data files afterwards.
- **Interrupted migration**: a later start resumes and completes it; no value is left corrupted or half-converted, and repeated starts are harmless.
- **Missing key**: startup is refused; existing data is untouched; no new key is generated over it.
- **Wrong key**: detected before any write; startup is refused with a message distinguishing "key does not match the stored data" from "no key found".
- **Malformed or empty operator key**: refused at startup with a message describing the expected form; no data is modified.
- **Key file permissions**: a generated key uses owner-only access where supported; on a platform that cannot express this, Vizier warns once rather than failing.
- **Read-only or full data directory**: key creation or migration fails with a clear error and does not leave a partially converted store.
- **Read-only database**: reads (agent retrieval, provider calls, channel startup) work with the key; writes report the existing storage error.
- **Concurrent writes during migration**: none occur, because migration finishes before agents, channels and scheduler start.
- **Very long, empty-padding, Unicode and multi-line values**: any value the existing limits accept is protected and returned byte-for-byte; the existing validation rules are unchanged.
- **Values shared across agents or settings**: identical plaintext produces unrelated stored representations, so a stored-data comparison reveals nothing.
- **Deleted values**: deletion removes the protected representation; no key or backup is needed to make an agent's next retrieval report "not found".
- **Legacy configuration fields**: credentials left in older agent or tool configuration records are covered by the same protection or removed, not left readable.
- **Key changed by hand**: the data becomes unreadable and startup reports a key mismatch; the old key must be restored. Automatic re-encryption is not offered in this release.
- **Multiple workspaces**: each workspace has its own key and protected data; a key from one must not be silently used for another.
- **Backup taken while Vizier is running**: the backup contains protected values; the key must be backed up separately, and the documentation says so.
- **Downgrade**: an older release cannot read protected values and must fail rather than overwrite them; the upgrade notes state this.
- **Upgrade path to 1.0.0**: once the conversion is removed after 1.0.0, an installation coming from a pre-encryption release must upgrade through the last pre-1.0 release first; skipping it leaves plaintext that the later release will not convert.
- **No secrets at all**: an installation that never stored a secret still starts normally and does not require key handling until it needs to protect something.

## Requirements *(mandatory)*

### Functional Requirements

**Protection**

- **FR-001**: Every credential or secret value Vizier persists and later reads back MUST be stored in a form that cannot be recovered without the key. This includes agent secret-store values, saved provider credentials (API keys, access tokens and custom-provider authentication headers), channel bot tokens, addon API keys, MCP server environment and header values, and credential-shaped values in shell and tool configuration — including values left behind in records saved by older releases.
- **FR-002**: Non-sensitive metadata MUST remain readable without the key: secret names, timestamps, actors, masks, provider identifiers, integration kinds, names and enabled state, and agent identifiers. Listing, filtering and permission checks MUST work without decrypting values.
- **FR-003**: Stored representations MUST be tamper-evident. A value that was truncated, altered or replaced outside Vizier MUST produce a clear integrity error when read, and MUST NOT be returned, partially returned or used as if valid.
- **FR-004**: Storing the same plaintext twice MUST produce different stored representations, so that comparing data files reveals nothing about which stored values are equal.
- **FR-005**: The stored representation MUST carry enough version information for a future release to recognise and convert it without data loss.
- **FR-006**: Protection and recovery MUST be transparent to every existing feature: agent set/get tools, WebUI and HTTP API management, masking, provider resolution and test, channel connection and reconnection, addon and MCP tool use, scheduled and background turns, agent update/restart and full restarts.

**Key handling**

- **FR-007**: On a fresh data directory with no key configured, Vizier MUST generate a strong random key automatically, persist it outside the database, restrict access to the account running Vizier where the platform supports it, and start with no prompt or manual step.
- **FR-008**: An operator MUST be able to supply the key through the environment or a designated key file. A supplied key MUST take precedence over the generated key and MUST be validated at startup.
- **FR-009**: Vizier MUST NOT prompt for a passphrase, fetch a key over the network, or depend on an interactive desktop service at startup. Headless, container and service-manager startup MUST remain supported.
- **FR-010**: The key MUST NOT be stored inside the database or data records, returned by any API or UI, written to logs, telemetry, audit records, error messages or support output.
- **FR-011**: When protected data exists and the key is missing, unreadable, malformed or does not match the data, Vizier MUST refuse to start with an actionable message that identifies the cause and how to recover. It MUST NOT generate a replacement key, overwrite data, or continue with partial functionality.
- **FR-012**: Vizier MUST distinguish "key not found", "key unreadable or malformed" and "key does not match the stored data" in its operator-facing error, and MUST NOT reveal key material in any of them.
- **FR-013**: A key supplied by the operator MUST survive restarts and upgrades unchanged. Changing the key without re-encrypting the data is a documented unsupported operation that fails closed.

**Upgrade and migration**

- **FR-014**: On first start after upgrade, Vizier MUST automatically convert every existing plaintext in-scope value in place, with no operator action, and MUST record that the conversion completed.
- **FR-015**: Migration MUST be idempotent and resumable. An interrupted or repeated run MUST leave each value either fully plaintext or fully protected, never corrupted, and MUST eventually complete.
- **FR-016**: After a successful migration, the data directory MUST NOT contain a recoverable copy of any migrated plaintext, including in database free space and transient database journal files. The old plaintext MUST be reclaimed before normal operation begins.
- **FR-017**: Migration MUST complete before agents, channels, the scheduler or background work start, so no consumer reads a value mid-conversion and no concurrent write races the conversion.
- **FR-018**: A migration or key failure MUST abort startup with a clear error rather than leave the installation silently unreadable. A subsequent start with the correct key MUST be able to complete recovery.
- **FR-019**: Migration MUST log progress and counts without logging any value or key material.
- **FR-028**: The plaintext conversion and every compatibility path it introduces — plaintext detection, the legacy-credential scrub, the migration marker and any helper that exists only for it — MUST be recorded in the project's pre-1.0 migration tracker (#82) as one removable unit, naming the code and what depends on it. The protected-value format version reader is permanent and MUST NOT be listed as removable.
- **FR-029**: Upgrade and release notes MUST identify this release as carrying a one-time conversion that an installation must run before upgrading to 1.0.0, and MUST state that the conversion and its plaintext branches are removed afterwards.

**Compatibility and operations**

- **FR-020**: All existing secret and credential workflows MUST continue unchanged: creating, replacing, retrieving, listing and deleting agent secrets; masking; provider save, test and deletion; channel connect, token replacement and disconnect; addon and MCP configuration; restart and agent update; and deletion of an agent and its data.
- **FR-021**: The existing non-disclosure guarantees MUST remain: no stored value in management responses, logs, telemetry, audit records, change records, execution reports, stdout, tracebacks or nested tool arguments, beyond the already-documented visibility of a value supplied in a set operation.
- **FR-022**: Values MUST round-trip byte-for-byte, including Unicode, whitespace, line breaks and the full existing size range. Existing name, value, permission and validation rules MUST NOT change.
- **FR-023**: A backup or restore that includes both the data and the key MUST work on another machine with the same installation. Restoring data without the key MUST result in the fail-safe startup behaviour, not silent data loss or regeneration.
- **FR-024**: Documentation MUST state what the protection covers, what it does not cover (a compromised host or running process, configuration files and environment variables managed by the operator, conversation history, memory documents and full-disk encryption), and the correct backup, restore and key-loss procedures.
- **FR-025**: The feature MUST NOT require any external service, network access, cloud key store or operating-system keyring, and MUST preserve offline, single-binary, config-less operation.
- **FR-026**: Support/diagnostic output MUST NOT include the key or any protected value, and MUST be safe to share.
- **FR-027**: An older release MUST NOT silently overwrite protected data it cannot read; the upgrade notes MUST state that downgrade is not supported without restoring a pre-upgrade backup.

### Key Entities

- **Protected value**: a credential or secret Vizier stores and reads back. It has a clear-text value only in memory while in use, plus its non-sensitive metadata (name, owner, timestamps, actor, mask). The stored representation is meaningless without the key.
- **Encryption key**: the installation's single secret for reading and writing protected values. It lives outside the data records, is generated automatically when absent, may be supplied by the operator, and is required for every protected read.
- **Key source configuration**: where the key comes from — operator-supplied environment value, operator-supplied key file, or the generated key file — with a defined precedence and validation rules.
- **Key identifier**: a non-secret marker stored with protected data and derived from the key, used to detect a wrong key before any write and to report the failure clearly.
- **Migration record**: the durable marker that the one-time plaintext-to-protected conversion completed, so restarts do not repeat it and an interrupted run can resume. It is removed together with the conversion after the last pre-1.0 release.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With access to the data directory but not the key, an operator searching every file and database page finds zero in-scope secret values in readable form.
- **SC-002**: An existing installation upgrades with zero manual steps, and 100% of previously stored secrets and credentials still resolve to their exact original values.
- **SC-003**: After upgrade, zero migrated plaintext values remain anywhere in the data directory, including database free space and journal files.
- **SC-004**: A missing, wrong or malformed key causes startup to stop with a clear, actionable message in every case, and produces zero data modifications or key regenerations.
- **SC-005**: A fresh install reaches a working agent with zero key setup and zero interactive prompts.
- **SC-006**: 100% of existing secret and credential workflows pass unchanged, including agent retrieval, WebUI/API management, provider calls, channel connection, addon and MCP use, and agent deletion.
- **SC-007**: Storing the same plaintext twice yields different stored representations in 100% of trials.
- **SC-008**: Every corrupted or altered stored value produces a clear error and zero incorrect values used.
- **SC-009**: Secret read and write operations remain visually instantaneous to a person (no perceptible delay compared with the current release), and startup adds no more than a few seconds even on an installation with thousands of stored values.
- **SC-010**: An operator reading the documentation can correctly answer, without asking for help, what is protected, what is not, and how to back up, restore and recover from a lost key.

## Assumptions

- **Default key location**: one key per workspace, stored in the workspace's runtime directory outside the database, generated on first use with owner-only permissions where the platform supports it. An environment variable or explicit key-file path overrides it. This is the zero-setup default; operators needing stronger separation supply a key from a secret manager or mount.
- **Threat model**: the protection targets data at rest — copies, backups, synced folders, stolen disks and database files read by another account. It does not defend against a compromised host, a running Vizier process, a malicious operator with access to the key, memory inspection, swap, or full-disk/OS-level exposure. With the default key beside the data, a copy of the *entire* data directory including the key is not protected; only the data without the key is.
- **Key separation is the operator's choice**: the default favours zero-setup over isolation. The documentation recommends the environment/secret-file source when the key must not travel with the data.
- **One key per installation/workspace**: no per-agent, per-provider or per-integration keys in this release, because the user requirement is to stop plaintext storage, not to introduce a key hierarchy.
- **Scope is stored credentials, not all content**: conversation history, agent memory documents and CORE, uploaded session files, model prompts, usage records, and user password or API-key hashes are out of scope. History may contain a secret pasted by a person or written by an agent before it was stored; protecting or scrubbing transcripts is a separate feature.
- **Configuration and environment stay operator-managed**: `.vizier.yaml` values, including the JWT signing secret, and credentials supplied through environment variables are outside the data store and remain the operator's responsibility; the documentation will say so and recommend file permissions. Vizier will not rewrite or encrypt the configuration file.
- **No key rotation in this release**: automatic re-encryption under a new key, key escrow, recovery codes and expiry are out of scope. Changing the key by hand makes existing data unreadable until the old key is restored.
- **No cryptographic design commitment in this specification**: the implementation plan chooses the authenticated-encryption scheme and key-derivation details; the requirements are confidentiality, tamper evidence, randomized output, versioning and fail-safe key handling.
- **Existing permissions and masking are unchanged**: `all_agents:secret`, `owned_agents:secret`, the script-only retrieval rule and the redaction behaviour from `specs/027-agent-secrets/` all remain exactly as they are; this feature only changes how values are stored.
- **Backup procedure changes**: backups must include the key separately from the data; the documented procedure will make this explicit and the upgrade notes will warn that an older release cannot read protected data.
- **Migration happens once per installation**: the existing plaintext database is converted in place on first start; there is no separate migration command, and a failed migration is retried on the next start.
- **No new external dependency or service**: the key is stored and used locally, keeping the single-binary, offline, config-less guarantees intact.
- **Tracked for removal before 1.0.0**: the plaintext conversion is one-time migration code. Its marker, plaintext detection, legacy-credential scrub and tests are added to #82's inventory, with the permanent value-format reader and key handling explicitly excluded. After the last pre-1.0 release, the conversion and its plaintext branches are removed; installations must pass through that release first (FR-028, FR-029).
