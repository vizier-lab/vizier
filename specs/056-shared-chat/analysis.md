# Analysis: Shared Chat Snapshots

Consistency analysis of `spec.md` against `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`, `tasks.md` and the code on branch `056-shared-chat`. Round 2, 2026-11-12, done by hand (the earlier `/speckit-analyze` output was never saved; several files pointed at this file before it existed).

**Source of truth**: `spec.md` (FR-001 to FR-054, SC-001 to SC-016). Earlier drafts of the other documents used a shorter numbering of their own; those references were rewritten (A22).

**Method**: every FR and SC was looked up in the plan, the tasks and the code (Rust constants and tests read, not run: no `cargo` in this round; WebUI tests run). Severity: **High** = a requirement is violated or can be bypassed; **Medium** = a requirement is unmet or a document contradicts the spec; **Low** = wording, numbering, stale text.

## Findings

| ID | Sev | Area | Finding | Resolution / status |
|---|---|---|---|---|
| A1 | High | FR-016, FR-017, FR-051 vs research R1 | Removal of secrets, paths, recall and reasoning happens in the browser. A direct `POST /shares` can publish uncleaned text, so "every rule holds for direct requests" (FR-051) cannot be true for FR-016. Only the author's own data is at risk | **Accepted for v1**, now written in R1 and in the plan's risk table. A server-side backstop that rejects (never rewrites) credential-shaped text can be added without changing the contract. Product owner to confirm |
| A2 | Medium | Audience table, FR-014 vs research R8 | An early draft returned 404 to an anonymous visitor on a restricted link, so a signed-out named person was told "not found". The spec's table and FR-014 require "may sign in" | **Consistent now**: R8 and the contract return 401 `sign_in_required` to anonymous callers for anything that is not an active public share, and the WebUI offers sign-in with `returnTo` (T054, T059, T102) |
| A3 | Medium | FR-007 vs the contract and R7 | The author's name is optional and per share, but the read response is cached publicly and carries no viewer-specific field. The name cannot be added at read time | Decision: the name goes **inside the snapshot**, chosen at create time (defaults: on for the instance audiences, off for public and file). Not implemented: **DEFERRED T084** |
| A4 | Low | FR-037 vs research R13 | R13 cited FR-035 and described the Link tab ambiguously | R13 rewritten: file only, one line explaining why, also when web access is on |
| A5 | Medium | FR-049, SC-014 vs R12, T016, tasks | Miss rate was 30 per minute in R12, T016 and the plan; the spec and the Rust constant (`MISSES_PER_MINUTE`) say 60. The WebUI `limits.ts` twin said 30 | All set to 60 (`limits.ts` and a test, R12, T016, plan) |
| A6 | Medium | FR-012 vs contract and T055 | The spec says "display name and username"; accounts have only a username. T055 said "≤ 20 results"; the spec and the Rust constant say 10 | Results are 10 everywhere now (R12, T055, WebUI slices to 10, test). Username only: **spec wording to be changed** to "username (and display name where an account has one)" by the product owner; no code change |
| A7 | Medium | FR-039 vs `data-model.md` | The data model said a revoked row keeps its content until a purge; the spec, the contract and the code (`snapshot_json = ''`) delete the body and keep a tombstone | `data-model.md` rewritten: body deleted on revoke, tombstone kept |
| A8 | Low | FR-040 vs research R7 | R7 wrote `stale-while-revalidate=0`; the contract and the code send `public, max-age=300, s-maxage=300` with no stale directives | R7 rewritten: the header is absent, so no cache may serve past 300 s |
| A9 | Low | Quickstart, T014 | Both named `VIZIER_TEST_POSTGRES_URL`; the test support code reads `VIZIER_TEST_DATABASE_URL` | Both corrected |
| A10 | High | FR-030, SC-013 vs the read response | `ShareView` (the `GET /shares/{id}` body) includes `audience`, so a named person or any signed-in reader can observe whether the share is for named people, the instance or public. FR-030 says a viewer learns nothing about the audience; only the author sees it. The WebUI viewer does not display it | **Open (backend)**. Remove the field, or return it only to the author. **T104**, needs `cargo` |
| A11 | Medium | FR-044 | Nothing tells the author when an administrator revokes their share (no notification in `service/shares.rs`) | **DEFERRED T093** (backend) |
| A12 | Medium | FR-052 | No record of create, revoke and people changes, and no route to read it | **DEFERRED T099** (backend) |
| A13 | Medium | FR-047 | The 250 MB per-person total is not enforced; the 10 per hour and 200 live caps are | **DEFERRED T096** (backend) |
| A14 | Low | FR-046 | An offline file may be 20 MB; the preview refuses above 5 MiB for both carriers. Stricter than the spec, so safe | **DEFERRED T095** |
| A15 | Medium | FR-041, US5 scenario 4 | The author must be able to remove named people from a live share. No route exists | **DEFERRED T091**, recorded in `spec.md` ("Implementation Status") and `tasks.md`. Not invented client-side. No widening is possible (no update route, T053) |
| A16 | Medium | FR-007 | Optional author name: no field, no switch, no viewer line | **DEFERRED T084** (see A3) |
| A17 | Medium | FR-023 | No way to leave out all pictures and audio to reduce size | **DEFERRED T087** (WebUI only, no backend) |
| A18 | Medium | FR-002, FR-003 | The server checks the permission and the agent, but cannot tell whether `topic_id` is the caller's own conversation, or a channel or task conversation: it never reads the content (R1). The UI only offers Share on the caller's own agent and project chats | Accepted consequence of R1, recorded; `agent_id` and `topic_id` are informational, never dereferenced when serving |
| A19 | High | FR-011 | The dialog opened with "Anyone with the link" selected, and `createLink` defaulted to `public` | **Fixed**: no audience preselected, Create disabled until one is chosen, a missing audience is refused (T085, tests) |
| A20 | Medium | FR-043 | `ShareListItem.title` was typed as always present, so an administrator's list of shares outside their audience showed "undefined" and offered Open and Copy | **Fixed**: title optional, "Title not shown", only Revoke offered (T092, tests) |
| A21 | Low | Research R3 vs the snapshot contract | R3 described a `media` map keyed by name with a `size` field; the contract and code key by `m1…` with `bytes` | R3 rewritten |
| A22 | Medium | All documents | `tasks.md` (traceability table), `research.md`, `plan.md`, `data-model.md`, the contract and the quickstart cited FR and SC numbers from an older, shorter list (for example "FR-033 revocation window", "FR-016/FR-028 neutral answers", "SC-007 1,000 viewers") | **Fixed**: every reference rewritten to the `spec.md` numbering; the traceability table lists all of FR-001 to FR-054 and SC-001 to SC-016 with status |
| A23 | Medium | Tasks T054, T070 | T054 tested only pure functions and claimed a redirect it did not exercise; T070 never imported `main.tsx` | **Fixed**: `share.unauthorized.test.tsx` runs a 401 through the real transport and session handler; `main.test.tsx` imports the real entry against the hostile fixtures |
| A24 | Low | WebUI files | Four files said `specs/055-shared-chat` | Changed to 056 (unrelated colour constants left alone) |
| A25 | Low | `mockups/share-ui.md` | The mockup preselected the public audience and its review notes listed UI that has since been built | Mockup and note updated; still not reviewed by the maintainers (T007 open) |
| A26 | Medium | FR-020, FR-035, FR-050 | No review warning in the dialog; no warning that a file cannot be revoked, and no "unverified copy" line in the file; a fast double confirm could fire twice | **Fixed** (T086, T088, T098) with tests |
| A27 | Medium | FR-038 | No "this chat is shared" mark | **Fixed** with the existing list call, paged up to 500 newest shares (T090); a person with more may not see it |
| A28 | Medium | FR-036 | The author had no way to download the offline file of a link share | **Fixed** in My Shares for the author's own shares (T089) |
| A29 | Medium | FR-016 | The redaction table lacked npm, Hugging Face, GitLab, SendGrid, Twilio, Google OAuth, webhook URLs, Azure keys, cookies, database URLs with `@` in the password; paths lacked `/proc /sys /dev /run /data /app /Library`, `~user`, `$HOME`, `%USERPROFILE%`, `\\?\`; `/home/alice` alone left the user name; `max_token=2048` was removed | **Fixed** with cases in `redact.test.ts` and `paths.test.ts` (T100). Still best effort for unknown shapes (FR-020 says so) |
| A30 | Medium | FR-021, FR-027 | Media cache keyed by file name (two pasted `image.png` collapsed); metadata only stripped from PNG and JPEG; the claimed MIME was not checked against the bytes; the offline entry imported the API client through `routes/share` and accepted the first of several snapshot tags | **Fixed** (T101, T072) with tests |
| A31 | Medium | FR-014, FR-028 | The global 401 handler redirected to a bare `/login`, losing `/share/<id>` | **Fixed** (T102) |
| A32 | High | `export.ts` | `String.replace` with a string replacement made `$&`, `$'`, `` $` `` and `$$` in a title or message corrupt the offline file | **Fixed** with function replacers and a test per pattern (T103) |
| A33 | Medium | SC-001, SC-002, SC-006, SC-010, SC-011, SC-012, SC-015 | Verified only by unit tests or not at all: Playwright scenarios T039, T050, T060, T067, T071 and the quickstart run T081 have not been done | **Open**: no new work in this round; listed as "pending" in the traceability table |
| A34 | Low | SC-007 | The spec asks for at least 100 hostile conversations; the fixture set has 12 | **Open** (T071 and a generated set) |
| A35 | Low | `checklists/requirements.md` | Said 18 open decisions; the spec lists 22 | Corrected |
| A36 | Low | Plan | Referred to `analysis.md` before it existed; said "Notable departures" without the open items | `analysis.md` written; the plan's risk table lists the open requirements |
| A37 | Low | Research R15 | The import design was two sentences | Rewritten as the design handoff for the deferred import spec (context layout, what the snapshot already provides, rules) |

## Coverage

| | Count | Notes |
|---|---|---|
| FR in the spec | 54 | |
| Done (code and a test) | 44 | see the traceability table in `tasks.md`; FR-033 and FR-050 still lack their Playwright run |
| Done in part | 7 | FR-007 (no author name), FR-030 (`audience` in the read response), FR-035 (no "who saved it" name), FR-041 (no removal of named people), FR-046 (no 20 MB file limit), FR-047 (no 250 MB total), FR-051 (client-side removal) |
| Deferred whole | 3 | FR-023, FR-044, FR-052 |
| SC in the spec | 16 | verified by tests: SC-003, SC-004, SC-005, SC-008, SC-009, SC-016 (and SC-013 apart from A10); partly: SC-002, SC-007, SC-010, SC-014, SC-015; pending a run: SC-001, SC-006, SC-011, SC-012 |
| Tasks without a requirement | 0 | the Phase 1 setup tasks serve the plan's constraints |
| Requirements without a task before this round | 15 | FR-007, FR-023, FR-035, FR-036, FR-038, FR-041, FR-043, FR-044, FR-045, FR-046 to FR-048, FR-050, FR-052: now T084 to T099 (T100 to T104 are the round-2 fixes) |

## Next steps, in order

1. **Backend (needs `cargo`)**: T104 (drop or limit `audience` in the read response), T093 (notify the author), T096 (250 MB), T099 (action record), then the FR-041 route with its audit entry (T091).
2. **Format**: T084 (author name inside the snapshot) before the first release, because it changes the stored format.
3. **WebUI**: T087 (leave out pictures), T095 (20 MB file limit).
4. **Verification**: Playwright scenarios (T039, T050, T060, T067, T071), the quickstart run (T081), the 100-conversation hostile set (A34).
5. **Product owner**: A1 (server backstop or not), A6 (display name wording), the maintainers' review of the mockups (T007).
