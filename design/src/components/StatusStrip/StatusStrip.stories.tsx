import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { EmptyState } from "../EmptyState/EmptyState";
import { StatusStrip } from "./StatusStrip";
import { StatusPanel } from "./StatusPanel";

const items = [
  { id: "memory" as const, icon: "memory" as const, label: "recalled passages", count: 3 },
  { id: "jobs" as const, icon: "layers" as const, label: "background jobs running", count: 2, busy: true },
  { id: "files" as const, icon: "file" as const, label: "session files", count: 4 },
  { id: "changes" as const, icon: "git-branch" as const, label: "changed files", count: 5 },
];

const meta = {
  title: "Components/Chat/StatusStrip",
  component: StatusStrip,
  tags: ["approved"],
  parameters: { vizier: { id: "D62", replaces: "webui/app/components/BackgroundJobTray.tsx" } },
  args: { items },
} satisfies Meta<typeof StatusStrip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** A project chat: the strip carries the changed files too. */
export const WithChanges: Story = {};

/** A chat that has done nothing yet: every section still shows, at zero. */
export const Idle: Story = { args: { items: items.map((i) => ({ ...i, count: 0, busy: false })) } };

/**
 * The strip and its panel, as the chat draws them: click a count to open the panel,
 * and its x to close it again.
 */
export const Interactive: Story = {
  render: () => (
    <div className="flex h-600 items-end justify-center bg-panel p-24">
      <div className="flex w-1000 flex-col gap-8">
        <div className="h-84 rounded-xl border border-border bg-surface" />
        <StatusStrip items={items} panelGap={100}>
          {(item, close) => (
            <StatusPanel
              title={item.id === "memory" ? "Recalled memory" : item.id === "jobs" ? "Background jobs" : "Session files"}
              meta={item.count}
              onClose={close}
            >
              <ul className="flex flex-col">
                {["ops/on-call-rotation", "incidents/2026-10-db-failover"].map((p) => (
                  <li key={p} className="truncate px-8 py-6 type-mono-sm text-fg">
                    {p}
                  </li>
                ))}
              </ul>
            </StatusPanel>
          )}
        </StatusStrip>
      </div>
    </div>
  ),
};

/** The panel an item opens, hovering above the composer. */
export const WithPanel: Story = {
  render: () => (
    <div className="flex h-600 items-end justify-center bg-panel p-24">
      <div className="flex w-1000 flex-col gap-8">
        <div className="h-84 rounded-xl border border-border bg-surface" />
        <StatusStrip items={items} open="memory" panelGap={100}>
          {(item, close) => (
            <StatusPanel
              title={item.id === "memory" ? "Recalled memory" : item.id === "jobs" ? "Background jobs" : "Session files"}
              meta={item.id === "jobs" ? "2 running" : item.count}
              onClose={close}
              search={<div className="h-34 rounded-md border border-border-strong bg-surface" />}
              footer={<span className="type-small text-faint">Ranked by relevance</span>}
            >
              <ul className="flex flex-col">
                {["ops/on-call-rotation", "incidents/2026-10-db-failover", "people/mira"].map((p) => (
                  <li key={p} className="truncate px-8 py-6 type-mono-sm text-fg">
                    {p}
                  </li>
                ))}
              </ul>
            </StatusPanel>
          )}
        </StatusStrip>
      </div>
    </div>
  ),
};

/** The panel with nothing in it: the empty state keeps its margin from the border. */
export const EmptyPanel: Story = {
  render: () => (
    <div className="flex h-600 items-end justify-center bg-panel p-24">
      <div className="flex w-1000 flex-col gap-8">
        <div className="h-84 rounded-xl border border-border bg-surface" />
        <StatusStrip items={items} open="memory" panelGap={100}>
          {(_item, close) => (
            <StatusPanel
              title="Recalled memory"
              meta="0 passages"
              onClose={close}
              search={<div className="h-34 rounded-md border border-border-strong bg-surface" />}
            >
              <EmptyState
                kind="empty"
                icon="memory"
                title="Nothing recalled"
                message="Passages the agent reads from memory appear here."
              />
            </StatusPanel>
          )}
        </StatusStrip>
      </div>
    </div>
  ),
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid rows={[{ cells: interactionCells(() => <StatusStrip items={items} />).filter((c) => c.label !== "Pressed") }]} />
  ),
};
