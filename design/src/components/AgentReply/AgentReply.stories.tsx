import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentHeader } from "../AgentHeader/AgentHeader";
import { ActivityTrail } from "../ActivityTrail/ActivityTrail";
import { Prose } from "../Prose/Prose";
import { AddReaction, Reaction } from "../Reaction/Reaction";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { AgentReply } from "./AgentReply";

const TEXT = "All three handled our 2.1M-passage corpus; the difference is tail latency. sqlite-vec keeps p99 under 40 ms on one node, which is why I would start there.";

const meta = {
  title: "Components/Chat/AgentReply",
  component: AgentReply,
  tags: ["approved"],
  parameters: { vizier: { id: "D45", replaces: "webui/app/components/MessageItem.tsx" } },
  args: {
    children: TEXT,
    reactions: (
      <>
        <Reaction emoji="👍" count={1} />
        <AddReaction />
      </>
    ),
    stats: "anthropic/claude-sonnet-5-5 · 6.1k in · 712 out · ~$0.03",
    onCopy: () => {},
    onMore: () => {},
  },
  decorators: [(Story) => <div className="w-640">{Story()}</div>],
} satisfies Meta<typeof AgentReply>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Streaming: Story = { args: { state: "streaming" } };

export const Failed: Story = {
  args: { state: "failed", error: "Stopped: the provider didn't answer in 120 s. What arrived is kept; send again to retry." },
};

/** Hover shows Copy and ⋯. */
export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: (args) => (
    <StateGrid
      rows={[
        { cells: [{ label: "Default", node: <div className="w-560"><AgentReply {...args} /></div> }] },
        { cells: [{ label: "Hover", pseudo: "hover", node: <div className="group w-560"><AgentReply {...args} className="[&_.opacity-0]:opacity-100" /></div> }] },
      ]}
    />
  ),
};

/** A whole turn: header, folded trail, the reply with a table. */
export const Turn: Story = {
  render: (args) => (
    <div className="flex flex-col gap-12">
      <AgentHeader agentId="archivist" name="Archivist" time="14:02" />
      <ActivityTrail state="done" title="Worked 18.4s" counts="3 python runs · 4 passages recalled" />
      <AgentReply {...args}>
        <Prose>
          <p>All three handled our 2.1M-passage corpus; the difference is in tail latency and how they grow.</p>
          <table>
            <thead>
              <tr>
                <th>Database</th>
                <th>p50</th>
                <th>p99</th>
              </tr>
            </thead>
            <tbody>
              <tr><td>Qdrant</td><td>4.1 ms</td><td>19 ms</td></tr>
              <tr><td>sqlite-vec</td><td>6.9 ms</td><td>31 ms</td></tr>
              <tr><td>pgvector</td><td>5.3 ms</td><td>48 ms</td></tr>
            </tbody>
          </table>
        </Prose>
      </AgentReply>
    </div>
  ),
};
