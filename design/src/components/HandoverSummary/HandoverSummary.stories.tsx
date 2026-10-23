import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { CheckpointDivider } from "../CheckpointDivider/CheckpointDivider";
import { HandoverSummary } from "./HandoverSummary";

const meta = {
  title: "Components/Chat/HandoverSummary",
  component: HandoverSummary,
  tags: ["approved"],
  parameters: { vizier: { id: "X31", replaces: "webui/app/components/CheckpointDivider.tsx" } },
  args: { children: "Reviewing PR #418. Found two bugs in settle(); asked for a lock around retries. Next: re-run the race test." },
  decorators: [(Story) => <div className="w-400">{Story()}</div>],
} satisfies Meta<typeof HandoverSummary>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Opened from its divider. */
export const FromDivider: Story = {
  render: function Render(args) {
    const [open, setOpen] = useState(true);
    return (
      <div className="flex flex-col gap-10">
        <CheckpointDivider detail="handed over at 81% of 200k" open={open} onClick={() => setOpen(!open)} />
        {open && <HandoverSummary {...args} />}
      </div>
    );
  },
};
