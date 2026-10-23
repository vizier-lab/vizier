import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentPreviewCard } from "./AgentPreviewCard";

const meta = {
  title: "Components/Agents & dreams/AgentPreviewCard",
  component: AgentPreviewCard,
  tags: ["approved"],
  parameters: { vizier: { id: "X55", replaces: null } },
  args: {
    agentId: "scribe",
    name: "Scribe",
    description: "meeting notes",
    facts: [
      { label: "Model", value: "ollama/llama3.3:70b" },
      { label: "Dreams", value: "nightly 03:00" },
      { label: "Voice", value: "in and out" },
      { label: "Shell", value: "Docker" },
      { label: "Tools", value: "14 categories" },
    ],
    note: "Its sigil is drawn from the agent ID, so it stays the same everywhere: here, in Discord and in Telegram.",
  },
  decorators: [(Story) => <div className="w-340">{Story()}</div>],
} satisfies Meta<typeof AgentPreviewCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
