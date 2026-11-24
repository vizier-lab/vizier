import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentCard } from "./AgentCard";

const meta = {
  title: "Components/Agents & dreams/AgentCard",
  component: AgentCard,
  tags: ["approved"],
  parameters: { vizier: { id: "X53", replaces: "webui/app/components/AgentCard.tsx" } },
  args: {
    agentId: "archivist",
    name: "Archivist",
    description: "memory & research",
    state: "running",
    detail: "replying",
    model: "anthropic/claude-sonnet-5-5",
    owned: true,
    href: "#",
  },
  decorators: [(Story) => <div className="w-226">{Story()}</div>],
} satisfies Meta<typeof AgentCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Live: Story = {};

/** Home's court: running, live, failed; owned or not. */
export const Court: Story = {
  decorators: [(Story) => <div className="grid w-710 grid-cols-3 gap-12">{Story()}</div>],
  render: () => (
    <>
      <AgentCard agentId="archivist" name="Archivist" description="memory & research" state="running" detail="replying" model="anthropic/claude-sonnet-5-5" owned href="#" />
      <AgentCard agentId="scout" name="Scout" description="web errands" state="running" detail="replying" model="openai/gpt-5-mini" href="#" />
      <AgentCard agentId="herald" name="Herald" description="Discord announcer" state="live" detail="2h ago" model="google/gemini-3-flash" href="#" />
      <AgentCard agentId="treasurer" name="Treasurer" description="usage & cost reports" state="live" detail="2h ago" model="ollama/qwen3:32b" owned href="#" />
      <AgentCard agentId="scribe" name="Scribe" description="meeting notes" state="failed" detail="retry in 40s" model="ollama/llama3.3:70b" href="#" />
      <AgentCard agentId="chamberlain" name="Chamberlain" description="scheduling" state="live" detail="2h ago" model="anthropic/claude-haiku-5-5" href="#" />
    </>
  ),
};
