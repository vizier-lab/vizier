import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentHeader } from "./AgentHeader";

const meta = {
  title: "Components/Chat/AgentHeader",
  component: AgentHeader,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D44", replaces: "webui/app/components/MessageItem.tsx" } },
  args: { agentId: "archivist", name: "Archivist", time: "14:02" },
} satisfies Meta<typeof AgentHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
