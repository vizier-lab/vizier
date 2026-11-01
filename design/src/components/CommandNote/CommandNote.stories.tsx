import type { Meta, StoryObj } from "@storybook/react-vite";
import { CommandNote } from "./CommandNote";

const meta = {
  title: "Components/Chat/CommandNote",
  component: CommandNote,
  tags: ["approved"],
  parameters: { vizier: { id: "X33a" } },
  args: { command: "checkpoint", meta: "14:03" },
  decorators: [(Story) => <div className="w-640">{Story()}</div>],
} satisfies Meta<typeof CommandNote>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A checkpoint: the conversation carries on from a handover. */
export const Checkpoint: Story = {};

/** A lobotomy: the context was cleared, nothing carried over. */
export const Lobotomy: Story = { args: { command: "lobotomy" } };

/** An abort, which also stops whatever was running. */
export const Abort: Story = { args: { command: "abort" } };
