import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import { VoiceRecorder } from "./VoiceRecorder";

const meta = {
  title: "Components/Chat/VoiceRecorder",
  component: VoiceRecorder,
  tags: ["approved"],
  parameters: { vizier: { id: "X35", replaces: null } },
  args: { mode: "recording", elapsed: 12, onDiscard: () => {} },
  decorators: [(Story) => <div className="w-396 rounded-xl border border-border-strong bg-surface px-14 py-14">{Story()}</div>],
} satisfies Meta<typeof VoiceRecorder>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Recording: Story = {};

/** A voice as the microphone hears it: a level every 60ms, newest at the right, scrolling left. */
export const Speaking: Story = {
  render: (args) => {
    const [levels, setLevels] = useState<number[]>([]);
    useEffect(() => {
      let t = 0;
      const id = setInterval(() => {
        t += 1;
        // Syllables over a breath: loud runs, short dips, a pause every few seconds.
        const pause = t % 60 > 48;
        const level = pause ? 0.02 : Math.min(1, 0.25 + 0.5 * Math.abs(Math.sin(t / 2.3)) * Math.random() + 0.15 * Math.random());
        setLevels((all) => [...all.slice(-399), level]);
      }, 60);
      return () => clearInterval(id);
    }, []);
    return <VoiceRecorder {...args} levels={levels} />;
  },
};

export const Recorded: Story = { args: { mode: "recorded", progress: 0 } };
