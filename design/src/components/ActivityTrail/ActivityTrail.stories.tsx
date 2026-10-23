import type { Meta, StoryObj } from "@storybook/react-vite";
import { CodeBlock } from "../CodeBlock/CodeBlock";
import { Fold } from "../Fold/Fold";
import { Notice } from "../Notice/Notice";
import { ScriptCall } from "../ScriptCall/ScriptCall";
import { Thought } from "../Thought/Thought";
import { TrailCall } from "../TrailCall/TrailCall";
import { ActivityTrail, TrailItem } from "./ActivityTrail";

const meta = {
  title: "Components/Chat/ActivityTrail",
  component: ActivityTrail,
  tags: ["approved"],
  parameters: { vizier: { id: "D49", replaces: "webui/app/components/ActivityTrail.tsx" } },
  args: { state: "done", title: "Worked 18.4s", counts: "3 python runs · 4 passages recalled", defaultExpanded: true },
  decorators: [(Story) => <div className="w-720">{Story()}</div>],
} satisfies Meta<typeof ActivityTrail>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A finished turn, opened: three runs and the last script. */
export const Done: Story = {
  render: (args) => (
    <ActivityTrail {...args}>
      <TrailCall name="execute_python" intent="search infra for benchmark notes" duration="1.1s" />
      <TrailCall name="execute_python" intent="parse 4 passages, render table" duration="12.6s" />
      <TrailCall name="execute_python" intent="file the summary in infra" duration="0.4s" />
      <TrailItem>
        <CodeBlock language="execute_python" code={'hits = memory_search(query="vector db benchmark", limit=8)\nmemory_write(bundle="infra", path="vector-dbs/summary.md", body=render_table(rows))'} />
      </TrailItem>
    </ActivityTrail>
  ),
};

/** Working: a thought, then the running call. */
export const Live: Story = {
  args: { state: "live", title: "Working 3.2s", counts: "1 thought" },
  render: (args) => (
    <ActivityTrail {...args}>
      <TrailItem>
        <Thought>The benchmark notes live in the infra bundle. Search there first rather than the whole memory, then read the three newest.</Thought>
      </TrailItem>
      <TrailCall name="execute_python" intent="search infra for benchmark notes" state="running" />
    </ActivityTrail>
  ),
};

/** A call inside failed: marked where it happened, with its notice. */
export const CallFailed: Story = {
  args: { title: "Worked 4.1s", counts: "2 python runs · 1 failed" },
  render: (args) => (
    <ActivityTrail {...args}>
      <TrailCall name="execute_python" intent="file the summary" duration="0.6s" state="failed">
        <ScriptCall name="memory_search" args='"vector db benchmark"' duration="0.3s" />
        <ScriptCall name="memory_write" args="infra/vector-dbs/summary.md" duration="0.1s" state="failed" />
        <Notice tone="failed" title="memory_write failed">
          <span className="type-mono-sm">bundle 'infra' is read-only for Archivist</span>
        </Notice>
      </TrailCall>
      <TrailCall name="execute_python" intent="write to research bundle instead" duration="0.5s">
        <ScriptCall name="memory_write" args="research/vector-dbs.md" duration="0.2s" />
      </TrailCall>
    </ActivityTrail>
  ),
};

/** Lookups read quieter. */
export const Lookups: Story = {
  args: { title: "Worked 1.9s", counts: "2 lookups · 1 python run" },
  render: (args) => (
    <ActivityTrail {...args}>
      <TrailCall name="list_tool_functions" intent="memory · 8 functions" state="lookup" />
      <TrailCall name="describe_tool_function" intent="memory_write" state="lookup" />
      <TrailCall name="execute_python" intent="file the summary" duration="1.8s" />
    </ActivityTrail>
  ),
};

/** A long trail: the middle folds so the newest stays in view. */
export const Long: Story = {
  args: { state: "live", title: "Working 48s", counts: "2 thoughts · 12 python runs" },
  render: (args) => (
    <ActivityTrail {...args}>
      <TrailCall name="execute_python" intent="search infra for benchmark notes" duration="1.1s" />
      <TrailCall name="execute_python" intent="read qdrant notes" duration="0.8s" />
      <Fold label="9 more · 1 thought, 8 python runs" />
      <TrailCall name="execute_python" intent="render comparison table" duration="2.0s" />
      <TrailCall name="execute_python" intent="write the summary" state="running" />
    </ActivityTrail>
  ),
};

/** Folded after the reply: one line with ›. */
export const Collapsed: Story = { args: { defaultExpanded: false } };

/** Mobile: a mini timeline. */
export const Mini: Story = { args: { title: "Worked 18.4s", counts: "3 runs", mini: { dots: ["done", "done", "done"] } } };
