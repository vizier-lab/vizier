import type { Meta, StoryObj } from "@storybook/react-vite";
import { PlanCard } from "./PlanCard";

const meta = {
  title: "Components/Chat/PlanCard",
  component: PlanCard,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X89", replaces: null } },
  args: {
    title: "Add --json to agent ps",
    summary: "Print the same rows as the table, as JSON, behind a flag.",
    steps: ["Add json: bool to PsArgs in cli/agent.rs", "Serialize AgentHealthStatus rows when it is set", "Run cargo test -p vizier"],
    onBuild: () => {},
  },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof PlanCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The newest plan: one action. */
export const Proposed: Story = {};

/** Build was pressed; the chat is switching mode. */
export const Building: Story = { args: { building: true } };

/** An older plan: a later message exists, so the action is gone. */
export const History: Story = { args: { onBuild: undefined } };

export const LongPlan: Story = {
  args: {
    summary: undefined,
    steps: Array.from({ length: 11 }, (_, i) => `Step ${i + 1}: update the module and keep its tests passing, then move on to the next one`),
  },
};
