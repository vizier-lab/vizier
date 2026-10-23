import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentRow } from "./AgentRow";

const meta = {
  title: "Components/Lists/AgentRow",
  component: AgentRow,
  tags: ["approved"],
  parameters: { vizier: { id: "X30", replaces: null } },
  args: { agentId: "Archivist", name: "Archivist", state: "running", detail: "replying", href: "#" },
  decorators: [(Story) => <div className="w-375 divide-y divide-border">{Story()}</div>],
} satisfies Meta<typeof AgentRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Mobile Home: running, live, failed. */
export const List: Story = {
  render: () => (
    <>
      <AgentRow agentId="Archivist" name="Archivist" state="running" detail="replying" href="#" />
      <AgentRow agentId="Herald" name="Herald" state="live" detail="2h ago" href="#" />
      <AgentRow agentId="Scribe" name="Scribe" state="failed" detail="retry in 40s" href="#" />
    </>
  ),
};
