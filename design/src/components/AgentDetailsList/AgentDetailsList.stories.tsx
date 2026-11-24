import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentDetailsList } from "./AgentDetailsList";

const meta = {
  title: "Components/Mobile/AgentDetailsList",
  component: AgentDetailsList,
  tags: ["approved"],
  parameters: { vizier: { id: "X91", replaces: null } },
  args: { hrefFor: (p) => `#${p}`, counts: { memory: "5", tasks: "2 open", channels: "2", addons: "1", mcp: "3" } },
  decorators: [(Story) => <div className="w-370">{Story()}</div>],
} satisfies Meta<typeof AgentDetailsList>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** A person who may not see Secrets or Artifacts, and may not edit Config. */
export const Restricted: Story = {
  args: { hiddenPages: ["secrets", "artifacts"], disabledPages: ["config"] },
};
