import type { Meta, StoryObj } from "@storybook/react-vite";
import { CallGlyphIcon, CallIcons, type CallIconItem } from "../CallIcons/CallIcons";
import { ScriptCall } from "../ScriptCall/ScriptCall";
import { TrailCall } from "./TrailCall";

const meta = {
  title: "Components/Chat/TrailCall",
  component: TrailCall,
  tags: ["approved"],
  parameters: { vizier: { id: "D51", replaces: null } },
  args: { name: "execute_python", intent: "compare benchmark notes, render table", duration: "12.6s", state: "done" },
  argTypes: { state: { control: "inline-radio", options: ["done", "running", "failed", "limit", "lookup"] } },
  decorators: [(Story) => <ul className="w-480 rounded-lg bg-surface p-12">{Story()}</ul>],
} satisfies Meta<typeof TrailCall>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Done: Story = {};

export const Running: Story = { args: { state: "running" } };

export const Failed: Story = { args: { state: "failed", duration: "0.6s" } };

export const Limit: Story = { args: { state: "limit", intent: "re-run p99 on all 2.1M passages", duration: "30.0s" } };

/** A lookup: quieter, no time. */
export const Lookup: Story = { args: { state: "lookup", name: "list_tool_functions", intent: "memory · 8 functions" } };

const memory: CallIconItem = { key: "memory", label: "Memory", glyph: { icon: "memory" }, count: 3 };
const web: CallIconItem = { key: "web", label: "Web", glyph: { icon: "globe" }, count: 4 };
const github: CallIconItem = { key: "mcp:github", label: "GitHub (MCP)", glyph: { brand: "github" }, count: 1 };

/** In plain words: the run's intent as the row, and what it used beside the time. */
export const Plain: Story = {
  args: {
    name: undefined,
    intent: undefined,
    label: "Find the latest release notes online",
    icons: <CallIcons items={[web, memory]} total={7} duration="2.4s" />,
    duration: "2.4s",
  },
};

/** No intent was stated: a sentence made from what it used, in a softer tone. */
export const PlainNoIntent: Story = {
  args: { ...Plain.args, label: "Searched the web and memory", labelTone: "soft" },
};

/** Failed: the category of the call that failed is red. */
export const PlainFailed: Story = {
  args: {
    ...Plain.args,
    label: "Look up open issues for the 0.9 milestone",
    state: "failed",
    duration: "1.6s",
    icons: <CallIcons items={[{ ...github, failed: true }, memory]} total={4} />,
  },
};

/** A trail of plain rows: the times line up whatever each row used. */
export const PlainTrail: Story = {
  render: () => (
    <>
      <TrailCall label="Check what we discussed last week" icons={<CallIcons items={[memory]} total={3} />} duration="0.8s">
        <ScriptCall name="memory_search" icon={<CallGlyphIcon glyph={{ icon: "memory" }} />} args="last week" duration="0.3s" />
      </TrailCall>
      <TrailCall label="Find the latest release notes online" icons={<CallIcons items={[web, memory]} total={7} />} duration="2.4s" />
      <TrailCall label="Work out how long until the launch" icons={<CallIcons items={[]} />} duration="0.1s" />
      <TrailCall label="Look up open issues for the 0.9 milestone" icons={<CallIcons items={[github]} total={1} />} duration="1.6s" />
      <TrailCall label="Draft the weekly summary" state="running" icons={<CallIcons items={[memory]} total={1} />} />
    </>
  ),
};

/** A phone row: one glyph and a count. */
export const PlainCompact: Story = {
  args: { ...Plain.args, icons: <CallIcons items={[web, memory, github]} compact total={8} /> },
  decorators: [(Story) => <div className="w-343">{Story()}</div>],
};
