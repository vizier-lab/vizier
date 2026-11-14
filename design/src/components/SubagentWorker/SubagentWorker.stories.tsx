import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SubagentWorker, type SubagentWorkerState } from "./SubagentWorker";
import { ActivityTrail } from "../ActivityTrail/ActivityTrail";
import { TrailCall } from "../TrailCall/TrailCall";
import { Eyebrow } from "../Eyebrow/Eyebrow";

const meta = {
  title: "Components/Chat/SubagentWorker",
  component: SubagentWorker,
  tags: ["approved"],
  parameters: { vizier: { id: "059-A" } },
  args: {
    task: "Compare vendor A and B list prices",
    position: 1,
    handle: "s_01",
    state: "running",
    agentName: "Archivist",
    elapsed: "1:12",
  },
  argTypes: {
    state: { control: "select", options: ["running", "asking", "answered", "failed", "timed_out", "stopped"] },
  },
  decorators: [(Story) => <div className="w-[760px]">{Story()}</div>],
} satisfies Meta<typeof SubagentWorker>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Running: Story = {};

/** Asking: amber, quoting the question it put to the main agent. */
export const Asking: Story = {
  args: {
    task: "Find the EU price sheet for vendor C",
    position: 2,
    handle: "s_02",
    state: "asking",
    elapsed: "0:31",
    question: "Vendor C has two EU sheets, retail and partner. Which one should I use?",
  },
};

/** Expanded: the steps are the chat's own activity trail (the WebUI passes its `Trail`). */
export const Expanded: Story = {
  args: {
    task: "Summarise last quarter's invoices",
    position: 3,
    handle: "s_03",
    state: "answered",
    elapsed: "0:58",
    handedOver: true,
  },
  render: (args) => {
    const [open, setOpen] = useState(true);
    return (
      <SubagentWorker
        {...args}
        expanded={open}
        onExpandedChange={setOpen}
        fullTask="Read the Q3 invoices in the finance bundle and total the spend per vendor. Return a table, and flag anything that looks wrong."
        stepCount={4}
        steps={
          <ActivityTrail state="done" title="Worked 58s" counts="4 python runs" defaultExpanded>
            <TrailCall name="execute_python" intent="search finance for the q3 invoices" duration="0.4s" />
            <TrailCall name="execute_python" intent="read finance/q3-invoices" duration="0.2s" />
            <TrailCall name="execute_python" intent="ask Archivist about INV-2207" duration="0.1s" />
            <TrailCall name="execute_python" intent="total the spend per vendor" duration="0.1s" />
          </ActivityTrail>
        }
        result={`Did: totalled 41 invoices across 6 vendors (table below).
Could not: check vendor E's credit note, it is not in memory.
How sure: high for the totals; vendor B includes both INV-2207 invoices, flagged.`}
      />
    );
  },
};

const rows: { state: SubagentWorkerState; task: string; elapsed: string; extra?: object }[] = [
  { state: "running", task: "Compare vendor A and B list prices", elapsed: "1:12" },
  {
    state: "asking",
    task: "Find the EU price sheet for vendor C",
    elapsed: "0:31",
    extra: { question: "Vendor C has two EU sheets, retail and partner. Which one should I use?" },
  },
  { state: "answered", task: "Summarise last quarter's invoices", elapsed: "0:58", extra: { handedOver: true } },
  { state: "answered", task: "Draft the renewal email", elapsed: "0:40", extra: { handedOver: false } },
  { state: "failed", task: "Parse the vendor export", elapsed: "0:02", extra: { handedOver: true } },
  { state: "timed_out", task: "Crawl the docs site", elapsed: "10:00", extra: { handedOver: true } },
  { state: "stopped", task: "Run the slow benchmark", elapsed: "1:02", extra: { stopReason: "by Archivist" } },
];

/** The panel's body: one spawn group, every state, collapsed. */
export const Group: Story = {
  render: () => (
    <div className="flex flex-col gap-8">
      <Eyebrow>This turn · {rows.length} workers · hired 1:12 ago</Eyebrow>
      {rows.map((r, i) => (
        <SubagentWorker
          key={i}
          position={i + 1}
          handle={`s_0${i + 1}`}
          agentName="Archivist"
          task={r.task}
          state={r.state}
          elapsed={r.elapsed}
          {...r.extra}
        />
      ))}
    </div>
  ),
};
