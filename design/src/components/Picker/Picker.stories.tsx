import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo, useState } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { Badge } from "../Badge/Badge";
import { ConversationRow } from "../ConversationRow/ConversationRow";
import { Kbd } from "../Kbd/Kbd";
import { ListItem } from "../ListItem/ListItem";
import { Segment } from "../Segment/Segment";
import { Status, agentStatusLabel } from "../Status/Status";
import { bundles, conversations, manyAgents, recentAgents, type AgentItem } from "./fixtures";
import { HighlightMatch, Picker, PickerAction } from "./Picker";

const meta = {
  title: "Components/Actions & inputs/Picker",
  component: Picker,
  tags: ["approved"],
  parameters: {
    vizier: { id: "X41", replaces: "webui/app/components/ProviderModelPicker.tsx" },
    docs: {
      description: {
        component:
          "One searchable picker behind the agent, conversation, topic and bundle pickers. ↑ ↓ move, Enter selects, Esc closes, typing filters. Rows are windowed, so 1,000 items scroll like ten. Shown inline here; pass a `trigger` to make it a popover (see AgentSwitcher).",
      },
    },
  },
} satisfies Meta<typeof Picker>;

export default meta;
type Story = StoryObj<typeof meta>;

const agentRow = (a: AgentItem, { query }: { query: string }, current?: string) => (
  <ListItem
    shape="bare"
    className="w-full"
    leading={<Sigil agentId={a.id} live={a.state === "live" || a.state === "running"} size="sm" />}
    title={<HighlightMatch text={a.name} query={query} />}
    subtitle={<Status state={a.state} label={agentStatusLabel(a.state)} detail={a.detail} font="mono-xs" dot={false} />}
    trailing={
      <>
        {a.owner && <Badge tone="rank">Owner</Badge>}
        {current === a.id && <Icon name="check" size={14} title="Current agent" />}
      </>
    }
  />
);

/** The agent picker: Recent, All A–Z, what's left, Create agent. */
export const Agent: Story = {
  args: {} as never,
  render: function Render() {
    const [q, setQ] = useState("");
    const all = manyAgents.filter((a) => a.name.toLowerCase().includes(q.toLowerCase()));
    const shown = q ? all : all.slice(0, 3);
    return (
      <Picker
        label="Agents"
        search={{ placeholder: "Search agents", total: q ? `${all.length} of 1,003` : "1,003", value: q, onChange: setQ }}
        sections={q ? [{ label: "Matches", items: shown }] : [{ label: "Recent", items: recentAgents }, { label: "All · A–Z", items: shown }]}
        getKey={(a) => a.id}
        renderItem={(a, s) => agentRow(a, s, "archivist")}
        onSelect={() => {}}
        remainder={q ? undefined : `${all.length - shown.length} more · type to filter`}
        footer={<PickerAction icon="plus">Create agent</PickerAction>}
      />
    );
  },
};

/** The conversation picker: filter tabs with counts, day groups, channel icons, a live dot. */
export const Conversation: Story = {
  args: {} as never,
  render: function Render() {
    const [q, setQ] = useState("");
    const [f, setF] = useState<"all" | "webui" | "discord" | "telegram">("all");
    const items = conversations.filter((c) => (f === "all" || c.channel === f) && c.title.toLowerCase().includes(q.toLowerCase()));
    return (
      <Picker
        label="Conversations"
        width={380}
        search={{ placeholder: "Search conversations", total: 600, value: q, onChange: setQ }}
        filters={
          <>
            {(["all", "webui", "discord", "telegram"] as const).map((k) => (
              <Segment key={k} selected={f === k} onClick={() => setF(k)} count={{ all: 600, webui: 38, discord: 412, telegram: 150 }[k]}>
                {{ all: "All", webui: "WebUI", discord: "Discord", telegram: "Telegram" }[k]}
              </Segment>
            ))}
          </>
        }
        sections={[
          { label: "Today", items: items.filter((c) => c.day === "Today") },
          { label: "Yesterday", items: items.filter((c) => c.day === "Yesterday") },
          { label: "Older", items: items.filter((c) => c.day === "Older") },
        ]}
        getKey={(c) => c.id}
        renderItem={(c, { query }) => (
          <ConversationRow kind={c.channel} title={c.title} query={query} place={c.where} live={c.live} time={c.time} />
        )}
        onSelect={() => {}}
        remainder="Older conversations load as you scroll"
        footer={<PickerAction icon="plus">New conversation</PickerAction>}
      />
    );
  },
};

/** The topic picker: compact rows, and a new-topic row with Enter. */
export const Topic: Story = {
  args: {} as never,
  render: function Render() {
    const [q, setQ] = useState("perf-regression");
    const topics = [
      { name: "vector-db-review", when: "now" },
      { name: "release-0.13", when: "2 h" },
      { name: "onboarding-copy", when: "yesterday" },
    ];
    return (
      <Picker
        label="Topics"
        width={396}
        itemHeight={33}
        search={{ placeholder: "Search 214 topics", value: q, onChange: setQ }}
        sections={[{ items: topics }]}
        getKey={(t) => t.name}
        renderItem={(t) => <ListItem shape="bare" className="w-full" title={t.name} titleMono meta={t.when} />}
        onSelect={() => {}}
        footer={
          <span className="flex w-full items-center justify-between">
            <span className="type-code text-fg">+ {q || "new topic"}</span>
            <Kbd>Enter</Kbd>
          </span>
        }
      />
    );
  },
};

/** The bundle picker, with the query highlighted in each match. */
export const Bundle: Story = {
  args: {} as never,
  render: function Render() {
    const [q, setQ] = useState("proj");
    const matches = bundles.filter((b) => b.name.includes(q));
    return (
      <Picker
        label="Bundles"
        width={360}
        itemHeight={33}
        search={{ placeholder: "Search bundles", total: `${matches.length} of 1,024`, value: q, onChange: setQ }}
        sections={[
          { label: "Recent", items: matches.filter((b) => ["project-atlas", "projects/2026-q4"].includes(b.name)) },
          { label: "All matches · A–Z", items: matches },
        ]}
        getKey={(b) => b.name}
        renderItem={(b, { query }) => (
          <ListItem
            shape="bare"
            className="w-full"
            leading={<Icon name="folder" size={15} tone="muted" />}
            title={<span className="type-code text-muted"><HighlightMatch text={b.name} query={query} /></span>}
            meta={b.docs.toLocaleString("en-US")}
          />
        )}
        onSelect={() => {}}
        remainder="↑↓ to move · Enter to open"
        footer={
          <>
            <PickerAction icon="plus">New bundle</PickerAction>
            <PickerAction>All bundles →</PickerAction>
          </>
        }
      />
    );
  },
};

/** 1,000 agents, all listed: only the visible rows are in the DOM. */
export const Thousand: Story = {
  args: {} as never,
  render: function Render() {
    const [q, setQ] = useState("");
    const items = useMemo(() => manyAgents.filter((a) => a.name.toLowerCase().includes(q.toLowerCase())), [q]);
    return (
      <Picker
        label="Agents"
        search={{ placeholder: "Search agents", total: items.length.toLocaleString("en-US"), value: q, onChange: setQ }}
        sections={[{ label: "All · A–Z", items }]}
        getKey={(a) => a.id}
        renderItem={(a, s) => agentRow(a, s)}
        onSelect={() => {}}
      />
    );
  },
};

/** Server mode: each keystroke fetches, and scrolling to the end fetches the next page. */
export const ServerPaged: Story = {
  args: { label: "Agents", search: { placeholder: "Search", value: "", onChange: () => {} }, sections: [], getKey: () => "", renderItem: () => null, onSelect: () => {} },
  render: function Render(args) {
    const [q, setQ] = useState("");
    const [count, setCount] = useState(25);
    const [loading, setLoading] = useState(false);
    const all = useMemo(() => manyAgents.filter((a) => a.name.toLowerCase().includes(q.toLowerCase())), [q]);
    const shown = all.slice(0, count);
    return (
      <Picker<AgentItem>
        {...args}
        label="Agents"
        search={{ placeholder: "Search agents", value: q, onChange: (v) => { setQ(v); setCount(25); }, mode: "server", total: `${all.length} agents` }}
        sections={[{ items: shown }]}
        getKey={(a) => a.id}
        renderItem={(a, { query }) => <HighlightMatch text={a.name} query={query} />}
        onSelect={() => {}}
        loading={loading}
        onEndReached={() => {
          if (count >= all.length) return;
          setLoading(true);
          setTimeout(() => { setCount((c) => c + 25); setLoading(false); }, 400);
        }}
      />
    );
  },
};

const emptyArgs = { label: "Conversations", search: { placeholder: "Search conversations", value: "", onChange: () => {} }, sections: [], getKey: () => "", renderItem: () => null, onSelect: () => {} };

/** The first page is on its way: placeholder rows, not an empty list. */
export const FirstLoad: Story = {
  args: emptyArgs,
  render: (args) => (
    <Picker {...args} width={380} loading footer={<PickerAction icon="plus">New conversation</PickerAction>} />
  ),
};

/** Nothing to show: `empty` says why, and the footer still offers the way forward. */
export const Empty: Story = {
  args: emptyArgs,
  render: (args) => (
    <Picker
      {...args}
      width={380}
      empty={
        <>
          <span className="type-body-strong text-fg">No conversations yet</span>
          <span>Start one below.</span>
        </>
      }
      footer={<PickerAction icon="plus">New conversation</PickerAction>}
    />
  ),
};
