import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentRow } from "./AgentRow";

const meta = {
  title: "Components/Lists/AgentRow",
  component: AgentRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X30", replaces: null } },
  args: { agentId: "Archivist", name: "Archivist", state: "live", detail: "replying", href: "#" },
  decorators: [(Story) => <div className="w-375 divide-y divide-border">{Story()}</div>],
} satisfies Meta<typeof AgentRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Mobile Home: live, idle, failed. */
export const List: Story = {
  render: () => (
    <>
      <AgentRow agentId="Archivist" name="Archivist" state="live" detail="replying" href="#" />
      <AgentRow agentId="Herald" name="Herald" state="idle" detail="2h ago" href="#" />
      <AgentRow agentId="Scribe" name="Scribe" state="failed" detail="retry in 40s" href="#" />
    </>
  ),
};
