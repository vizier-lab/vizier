# Feature Specification: Model Override for Chat Requests

**Feature Branch**: `049-model-override`

**Created**: 2026-11-04

**Status**: Draft

**Input**: User description: "Add model override to chat requests. A user/client can change the model used for a prompt. Introduce the single `model_override` permission, without `all_agents:` or `owned_agents:` scope prefixes. In the WebUI, users with permission can choose from provider/model options in a dropdown on the right side of the status-strip row, below the send button. Do not add Discord or Telegram support for now."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Choose a model for chat prompts (Priority: P1)

A user who has the model-override permission chooses an available provider/model in the WebUI and sends a prompt using that model, while continuing the same conversation.

**Why this priority**: This is the core user value: select which model answers a chat prompt without leaving the conversation.

**Independent Test**: Give a user the permission, select a listed model, send a prompt, and verify that the response comes from the selected model in the existing conversation.

**Acceptance Scenarios**:

1. **Given** a user has the model-override permission and available models are listed, **When** they choose a provider/model and send a prompt, **Then** that prompt is handled by the selected model and the conversation remains the same.
2. **Given** a user has selected a model, **When** they send another prompt in that conversation without changing the selection, **Then** that prompt also uses the selected model.
3. **Given** a user has selected a model, **When** they select a different available model and send a prompt, **Then** the new prompt uses the newly selected model.
4. **Given** the model selector is shown, **When** the user views its choices, **Then** each choice displays the provider name, a slash, and the complete model identifier, preserving any slashes in the model identifier. For example, provider `opencode` with model `deepseek/deepseek-flash` is displayed as `opencode/deepseek/deepseek-flash`, and provider `openai` with model `gpt-luna` is displayed as `openai/gpt-luna`.

---

### User Story 2 - Respect model-override permission (Priority: P1)

A user without model-override permission continues chatting with the normal model and is not offered model override controls; a permitted user can use the selector.

**Why this priority**: The feature is explicitly permission-gated; unauthorized users must not be able to change model selection.

**Independent Test**: Compare WebUI sessions for users with and without the permission and attempt to submit an override in each.

**Acceptance Scenarios**:

1. **Given** a user does not have the model-override permission, **When** they open a conversation, **Then** no model-override selector is shown and their prompts use the normal configured model.
2. **Given** a user lacks the permission, **When** they attempt to submit a model override by any available request path, **Then** the override is rejected and no different model is used.
3. **Given** a user has the `model_override` permission and available model choices, **When** they open a conversation, **Then** one model dropdown appears in the empty space immediately to the right of the status-strip controls, on their same row below the send button, while the existing status strip retains its appearance.

---

### User Story 3 - Handle unavailable model choices (Priority: P2)

A user gets clear feedback if a selected model is no longer available and their prompt is not silently sent using a different model.

**Why this priority**: Clear failure behavior avoids silently changing the model choice and gives the user a reliable way to recover.

**Independent Test**: Select a model, make it unavailable before sending, and verify the user is informed and the prompt is not processed by a fallback model.

**Acceptance Scenarios**:

1. **Given** a selected model is unavailable when the user sends a prompt, **When** the request is handled, **Then** the user is informed that the selected model cannot be used and the prompt is not silently handled by another model.
2. **Given** no eligible models are available, **When** a permitted user opens a conversation, **Then** the selector communicates that there are no available choices and does not present unusable choices as selectable.

### Edge Cases

- The available model list is empty, cannot be loaded, or changes while a conversation is open.
- The selected provider or model becomes unavailable between selection and prompt submission.
- A user's permission is removed after the selector was displayed or a model was selected.
- A conversation is reopened after a model was selected; its selected model remains associated with that conversation, unless the selection is no longer valid.
- The user changes models while a response is already in progress; the change applies to prompts sent afterward and does not change an in-progress response.
- The user is using Discord or Telegram; model override is not available there in this release.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST define a single permission named `model_override` that controls whether a user may select a model override. This permission MUST NOT be split into `all_agents:model_override` and `owned_agents:model_override` variants.
- **FR-002**: The WebUI MUST show the model selector only to users who have the `model_override` permission.
- **FR-003**: The WebUI MUST show exactly one model dropdown in the empty space immediately to the right of the existing status-strip controls, on their same row below the send button. The status strip and dropdown MUST be separated by flexible space, and adding the dropdown MUST NOT change the existing status-strip style.
- **FR-004**: The single dropdown MUST offer available provider/model choices, displaying each as `<provider>/<complete-model-id>`. The provider name is followed by `/` and the complete model identifier; any `/` characters within the model identifier MUST be preserved. For example, `opencode` with model `deepseek/deepseek-flash` displays as `opencode/deepseek/deepseek-flash`, and `openai` with model `gpt-luna` displays as `openai/gpt-luna`.
- **FR-005**: A permitted user MUST be able to select a model for prompts in the current conversation; the selected model MUST apply to prompts sent afterward until the user changes the selection or the selection becomes unavailable.
- **FR-006**: Changing the selected model MUST NOT create a new conversation or change a response that is already in progress.
- **FR-007**: The system MUST enforce the permission when processing a model override using the single `model_override` permission; hiding the selector alone MUST NOT grant or deny authorization.
- **FR-008**: If a requested model is unavailable or the user is not authorized, the system MUST reject the override and MUST NOT silently use a different model.
- **FR-009**: The WebUI MUST communicate when no eligible models are available or a selected model cannot be used.
- **FR-010**: Model override MUST be supported in the WebUI only for this release. Discord and Telegram MUST continue to use their existing model behavior without model-override controls.

### Key Entities *(include if data involved)*

- **Model choice**: An available model identified by its provider and complete model identifier; displayed as `<provider>/<complete-model-id>`, preserving slashes within the model identifier.
- **Conversation model selection**: The model selected by a permitted user for subsequent prompts in a conversation, until changed or no longer available.
- **`model_override` permission**: The single authorization that allows a user to select a model for chat prompts, independent of `all_agents` or `owned_agents` scope prefixes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At least 95% of permitted users who have available model choices can identify and select a desired model on their first attempt.
- **SC-002**: A permitted user can change the model for their next prompt in an existing conversation in 15 seconds or less.
- **SC-003**: 100% of model-override attempts from users without the permission are prevented from using a different model.
- **SC-004**: 100% of prompts submitted with an unavailable selected model avoid silent fallback to another model.
- **SC-005**: Users without the permission can continue sending prompts through the existing conversation flow without model-override controls or additional model-selection steps.

## Assumptions

- Model choices are the models currently available for use by the running application; provider/model names are sufficient for users to distinguish choices.
- A selection is scoped to the current conversation, is used for prompts sent after selection, and does not retroactively affect earlier or in-progress prompts.
- If a saved selection is no longer available, it is not silently replaced; the user receives feedback and can choose another available model.
- Existing conversation access controls remain in effect. The new permission only governs model override.
- Discord and Telegram support are intentionally out of scope for this release.
