import type { Meta, StoryObj } from "@storybook/react-vite";
import { ScriptCall } from "./ScriptCall";

const meta = {
  title: "Components/Chat/ScriptCall",
  component: ScriptCall,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D52", replaces: null } },
  args: { name: "memory_search", args: '"vector db benchmark"', duration: "0.4s", state: "ok" },
  argTypes: { state: { control: "inline-radio", options: ["ok", "failed"] } },
  decorators: [(Story) => <div className="w-480 rounded-lg bg-surface p-12">{Story()}</div>],
} satisfies Meta<typeof ScriptCall>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ok: Story = {};

export const Failed: Story = { args: { name: "memory_write", args: "infra/vector-dbs/summary.md", duration: "0.1s", state: "failed" } };

/** Four identical calls, grouped. */
export const Repeated: Story = { args: { name: "memory_read", repeat: 4, args: "infra/qdrant.md, +3", duration: "0.9s" } };

/** With its arguments, opened in a run's detail. */
export const Expandable: Story = {
  args: {
    name: "memory_write",
    args: "infra/vector-dbs/summary.md",
    duration: "0.2s",
    details: (
      <pre className="whitespace-pre-wrap">{'bundle  "infra"\npath    "vector-dbs/summary.md"\nbody    "| Database | p50 | p99 | RAM |…"'}</pre>
    ),
  },
};
