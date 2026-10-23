import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentStartingCard } from "./AgentStartingCard";

const meta = {
  title: "Components/Agents & dreams/AgentStartingCard",
  component: AgentStartingCard,
  tags: ["approved"],
  parameters: { vizier: { id: "X58", replaces: null } },
  args: { name: "Treasurer", step: "Pulling qwen3:32b from Ollama", percent: 62, detail: "12.4 GB of 20.0 GB", eta: "about 3 min", onDismiss: () => {} },
} satisfies Meta<typeof AgentStartingCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Pulling: Story = {};

export const Building: Story = { args: { step: "Building tools and connecting MCP servers", percent: undefined, detail: undefined, eta: undefined } };
