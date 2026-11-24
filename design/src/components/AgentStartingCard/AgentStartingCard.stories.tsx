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

/** Per-artifact detail, as the notification panel shows it: one line per download. */
export const WithDownloads: Story = {
  args: {
    step: "Pulling qwen3:32b",
    downloads: (
      <>
        <span className="type-mono-xs text-muted">model · qwen3:32b · 42% · pulling manifest</span>
        <span className="type-mono-xs text-muted">embedding · bge-small · done</span>
        <span className="type-mono-xs text-failed-text">stt · whisper-small · failed · connection reset</span>
      </>
    ),
  },
};

/** The card fills the notification panel's content width, up to its own size. */
export const InAPanel: Story = {
  decorators: [(Story) => <div className="w-400 max-w-full bg-surface p-20">{Story()}</div>],
};
