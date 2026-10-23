import type { Meta, StoryObj } from "@storybook/react-vite";
import { VoiceRecorder } from "./VoiceRecorder";

const meta = {
  title: "Components/Chat/VoiceRecorder",
  component: VoiceRecorder,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X35", replaces: null } },
  args: { mode: "recording", elapsed: 12, onStop: () => {}, onDiscard: () => {} },
  decorators: [(Story) => <div className="w-396 rounded-xl border border-border-strong bg-surface px-14 py-14">{Story()}</div>],
} satisfies Meta<typeof VoiceRecorder>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Recording: Story = {};

export const Recorded: Story = { args: { mode: "recorded", progress: 0 } };
