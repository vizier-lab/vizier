# Contract: WebUI

Reference layout: the mockup at https://claude.ai/artifact/SENVYMyyTnC9UBQbVFbdgN (sample data only). Colours come from the existing `app.css` tokens (`--surface`, `--border`, `--text-*`, `--accent-*`), so the page follows the app's light and dark themes. The mockup's literal hexes are not copied.

## Route `/:agentId/usage` (`routes/usage.tsx`, rewritten)

**URL state**: `?period=7|14|30|90|custom&start=…&end=…&activity=<id>&by=activity|model|place`. Defaults are `period=30`, no `activity` and `by=activity`. A view can be linked to and survives reload. Changing `activity` to a value where `by=activity` is meaningless switches `by` to `place`.

**One fetch per state change**: `GET /agents/{id}/usage?start_date&end_date&activity`. While it loads, the previous data stays on screen, dimmed.

**Sections, top to bottom:**

1. **Header**: breadcrumb "Agents / ‹name›", title "Usage", and a period picker built as a segmented control of real `<button aria-pressed>`. "Custom" opens two date inputs.
2. **Activity filter**: chips for All plus the seven activities, single choice (FR-021). Each chip has a colour dot from the activity palette below. **Other** is shown only when it has usage in range.
3. **Partial-accounting notice** (FR-019), shown when `range.start < accounting_since`:
   - with **no** activity filter;
   - or with an activity filter of `dreams`, `compaction` or `vision`.

   Its text says which things weren't recorded before the date. It is not dismissible.
4. **Headline card**:
   - `summary.cost_usd` (or "—" with "no known price" when it is null);
   - a change pill from `previous` (up = warning colour, down = success colour; hidden when `previous` is null);
   - "‹tokens› tokens · ‹calls› model calls · biggest driver: ‹activity› (‹share›)". When filtered, the last part reads "‹share_of_all›% of all spend".
5. **Two tiles**:
   - **Cache hit rate**, `summary.cache_hit_rate` with a bar ("—" when null);
   - **Spent on failed turns**, `summary.failed.cost_usd`, with its share of spend and the turn count, and a link that scrolls to Costliest.
6. **Unpriced note**, shown when `unpriced.tokens > 0`: "‹tokens› tokens on ‹models› have no known price and aren't in the cost."
7. **Daily cost**: one Recharts stacked `BarChart` of `daily[].by_activity[*].cost_usd`, one series per activity in range. The tooltip shows the date, cost, tokens and calls. Bars for dates before `accounting_since` render at reduced opacity. There are no metric or display toggles (FR-013).
8. **Where it went**: a segmented control (Activity, Model, Channel; Activity is omitted while filtered) over one ranked list. Each row has a colour swatch, label, detail line, share bar with %, cost, tokens and calls. Model rows also put served models, cache rate and failed tokens on the detail line, and unpriced models show "—" and "No known price".
9. **Costliest conversations**: up to 10 rows with rank, label and detail, activity dot, model, tokens, cost, and a **Details** `<button>`. Rows with `failed_share ≥ 0.5` get a "Failed" marker. An empty list shows "No conversations for this activity in the period."

**Empty period**: one message, "No usage in this period", in place of sections 4–9. No zero-filled chart and no "−100%".

## Details drawer (`components/UsageConversationDrawer.tsx`, new)

- Opened by **Details**. It fetches `GET …/usage/conversation?channel&topic&start_date&end_date&activity` and is a `role="dialog" aria-modal="true"` panel on the right, full height and full width on phones. Esc and the close button dismiss it, and focus returns to the Details button.
- **Contents**:
  - activity dot, label, detail;
  - four figures: cost, calls, cached input %, failed cost;
  - the `insight` callout when present;
  - "Model calls, most expensive first": time, activity · round, served model (red "part of failed turn" when `failed`), input, output, cost;
  - "+ ‹more› more calls" when `more > 0`.
- **Footer**:
  - when `link` is present, a primary link to `link.path`, labelled by kind:

    | Kind | Label |
    |---|---|
    | `chat` | Open in chat |
    | `task_run` | Open run in Tasks |
    | `dream` | Open in Dream journal |
    | `topic` | Open in launching topic |

  - otherwise one sentence from `no_link_reason`:
    - `no_web_page`: "‹Discord/Telegram› conversations have no page in the WebUI; this breakdown is the only view."
    - `deleted`: "This conversation was deleted. Its usage still counts."

## Activity palette

| Activity | Swatch |
|---|---|
| Chat | `--accent-dark` |
| Dreams | violet |
| Compaction | amber |
| Background | sky |
| Scheduled | slate |
| Vision | pink |
| Other | `--color-text-muted` |

Neighbouring series in the stack must also differ in lightness, not only in hue. The palette is defined once in `components/usageActivities.ts`, with each activity's label and description. The chart, chips, rows and drawer all read from it.

## Types and services

- `interfaces/types.ts`: remove `AgentUsageStats` and its helpers. Add `UsageActivity`, `UsageReport`, `UsageSummary`, `BreakdownRow`, `ModelRow`, `ConversationRow`, `ConversationUsage`, `CallRow` and `UsageLink`, mirroring contracts/http-api.md.
- `services/vizier.tsx`: `getAgentUsage(agentId, start, end, activity?)` returns `UsageReport`. Add `getConversationUsage(agentId, {channel, topic, start, end, activity, limit})`.
- `components/UsageBarChart.tsx`: replaced by `components/UsageDailyChart.tsx`. `formatCost` moves to `utils/`.

## Accessibility

All controls are real buttons or links with visible focus. Icon-only buttons have an `aria-label`. Text contrast is at least 4.5:1 in both themes. The chart has a visually hidden table summary (date, cost) for screen readers.
