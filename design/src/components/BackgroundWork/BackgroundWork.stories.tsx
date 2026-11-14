import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { BackgroundWork, type BackgroundWorkState } from "./BackgroundWork";
import { RunDetail } from "../RunDetail/RunDetail";
import { ScriptCall } from "../ScriptCall/ScriptCall";

const meta = {
  title: "Components/Chat/BackgroundWork",
  component: BackgroundWork,
  tags: ["approved"],
  parameters: { vizier: { id: "058-B", replaces: "src/components/BackgroundJob" } },
  args: {
    intent: "check vendor B's pricing page",
    handle: "d_02",
    state: "running",
    elapsed: "0:41",
    callCount: 2,
  },
  argTypes: {
    state: { control: "select", options: ["running", "succeeded", "failed", "timed_out", "cancelled", "interrupted"] },
  },
  decorators: [(Story) => <div className="w-[560px]">{Story()}</div>],
} satisfies Meta<typeof BackgroundWork>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Running: Story = {};

const code = `page = web_fetch(url="https://vendor-a.example/pricing")
rows = re.findall(r"\\$(\\d+)/mo", page)
{"plans": rows, "count": len(rows)}`;

/** Expanded: the body is the activity trail's own run detail (frame "058 · B"). */
export const Expanded: Story = {
  args: { intent: "check vendor A's pricing page", handle: "d_01", state: "succeeded", elapsed: "0:06", delivered: true },
  render: (args) => {
    const [open, setOpen] = useState(true);
    return (
      <BackgroundWork {...args} expanded={open} onExpandedChange={setOpen}>
        <RunDetail
          code={code}
          callCount={1}
          calls={<ScriptCall name="web_fetch" args="vendor-a.example/pricing" duration="1.2s" />}
          result={`{
  "count": 3,
  "plans": ["12", "49", "199"]
}`}
        />
      </BackgroundWork>
    );
  },
};

const states: { state: BackgroundWorkState; intent: string; elapsed: string; calls?: number; delivered?: boolean }[] = [
  { state: "running", intent: "run the tests and summarise failures", elapsed: "0:41", calls: 1 },
  { state: "succeeded", intent: "check vendor A's pricing page", elapsed: "0:06", delivered: true },
  { state: "succeeded", intent: "fetch the release notes", elapsed: "0:12", delivered: false },
  { state: "failed", intent: "parse the export", elapsed: "0:02", delivered: true },
  { state: "timed_out", intent: "crawl the docs site", elapsed: "10:00", delivered: true },
  { state: "interrupted", intent: "rebuild the index", elapsed: "3:10", delivered: false },
  { state: "cancelled", intent: "run the slow benchmark", elapsed: "1:02" },
];

/** Every state, collapsed, as in the panel. */
export const States: Story = {
  render: () => (
    <div className="flex flex-col gap-8">
      {states.map((s, i) => (
        <BackgroundWork
          key={i}
          intent={s.intent}
          handle={`d_0${i + 1}`}
          state={s.state}
          elapsed={s.elapsed}
          callCount={s.calls}
          delivered={s.delivered}
        />
      ))}
    </div>
  ),
};
