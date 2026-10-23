import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../Button/Button";
import { Segment } from "../Segment/Segment";
import { TextLink } from "../TextLink/TextLink";
import { ActivityGrid, type ActivityRow } from "./ActivityGrid";

function hours(seed: number, quiet = 0.3): number[] {
  let x = seed;
  return Array.from({ length: 24 }, (_, h) => {
    x = (x * 9301 + 49297) % 233280;
    const r = x / 233280;
    const day = h >= 8 && h <= 22 ? 1 : 0.3;
    return r < quiet ? 0 : Math.round(r * 10 * day);
  });
}

const FOUR: ActivityRow[] = [
  { label: "Archivist", hours: hours(1), live: true },
  { label: "Scout", hours: hours(2), live: true },
  { label: "Herald", hours: hours(3) },
  { label: "Treasurer", hours: hours(4, 0.5) },
];

const meta = {
  title: "Components/Visualisation/ActivityGrid",
  component: ActivityGrid,
  tags: ["needs-review"],
  parameters: {
    vizier: { id: "X68", replaces: "webui/app/routes/home.tsx" },
    docs: { description: { component: "Never more than seven rows, so Home keeps its height whether the court has one agent or a thousand. Dot size is turns in that hour; a green dot is an agent replying right now." } },
  },
  args: { rows: FOUR, meta: "4 agents · dot size = turns per hour" },
  decorators: [(Story) => <div className="w-660">{Story()}</div>],
} satisfies Meta<typeof ActivityGrid>;

export default meta;
type Story = StoryObj<typeof meta>;

/** 2 to 6 agents: one row each, most active first. */
export const Several: Story = {};

export const NoAgents: Story = {
  args: {
    rows: [],
    meta: undefined,
    empty: {
      title: "No activity yet",
      message: "This fills in once an agent takes its first turn: a chat, a task, or a dream.",
      action: <Button variant="primary" size="sm" icon="plus">New agent</Button>,
    },
  },
};

/** One agent: the row spans the width, and a sentence says what the day was. */
export const OneAgent: Story = {
  args: {
    rows: [{ label: "Archivist", hours: hours(1, 0.2), live: true }],
    meta: "101 turns · busiest 18:00",
    summary: "Archivist is replying now. 94 chat turns, 5 scheduled tasks and 2 dream stages today.",
  },
};

/** 1,000 agents: an "All" row, the five most active, and a switch. */
export const Thousand: Story = {
  render: function Render(args) {
    const [f, setF] = useState("active");
    return (
      <ActivityGrid
        {...args}
        meta="all agents + 5 most active · 37 live now"
        rows={[
          { label: "All 1,000", hours: hours(9, 0.05).map((v) => v * 40), live: true, total: true },
          { label: "Archivist", hours: hours(1), live: true },
          { label: "Scout", hours: hours(2), live: true },
          { label: "Herald", hours: hours(3) },
          { label: "Treasurer", hours: hours(4) },
          { label: "Scribe", hours: hours(5) },
        ]}
        footer={
          <>
            <div role="group" aria-label="Show" className="inline-flex rounded-md border border-border-strong bg-surface p-2">
              {[
                ["active", "Most active"],
                ["mine", "Mine"],
                ["live", "Live"],
                ["failed", "Failed"],
              ].map(([k, l]) => (
                <Segment key={k} selected={f === k} onClick={() => setF(k)}>
                  {l}
                </Segment>
              ))}
            </div>
            <TextLink size="ui" href="#" arrow="forward">995 more in Usage</TextLink>
          </>
        }
      />
    );
  },
};

/** Mobile. */
export const Compact: Story = { args: { compact: true, meta: undefined }, decorators: [(Story) => <div className="w-343">{Story()}</div>] };
