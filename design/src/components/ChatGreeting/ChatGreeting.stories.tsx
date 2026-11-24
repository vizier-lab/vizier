import type { Meta, StoryObj } from "@storybook/react-vite";
import { ChatGreeting } from "./ChatGreeting";

const meta = {
  title: "Components/Chat/ChatGreeting",
  component: ChatGreeting,
  tags: ["approved"],
  parameters: { vizier: { id: "X34", replaces: null } },
  args: { agentId: "archivist", name: "Archivist", meta: "Topic 4 of 5 · nothing sent yet" },
} satisfies Meta<typeof ChatGreeting>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
