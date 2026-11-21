import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import { CallRow, type CallState } from "./CallRow";
import { sampleLevels } from "../AudioPlayer/Waveform";

const meta = {
  title: "Components/Chat/CallRow",
  component: CallRow,
  tags: ["approved"],
  parameters: { vizier: { id: "D66", replaces: "CallPill in the composer deck's lip" } },
  args: { state: "listening", elapsedSeconds: 42, levels: sampleLevels(60, 5), onEnd: () => {} },
  argTypes: {
    state: {
      control: "select",
      options: ["connecting", "listening", "speaking", "interrupted", "reconnecting", "ended", "failed"],
    },
  },
  // Drawn where it lives: inside the composer box, above the text.
  decorators: [(Story) => <div className="w-560 max-w-full rounded-xl border border-border-strong bg-surface px-14 py-14">{Story()}</div>],
} satisfies Meta<typeof CallRow>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Listening: the waveform is the microphone, newest at the right. */
export const Listening: Story = {};

/** Every state. Connecting waits with dots; reconnecting is amber. The agent working is not a call state: the activity trail shows it. */
export const States: Story = {
  render: () => {
    const live: CallState[] = ["connecting", "listening", "speaking", "interrupted", "reconnecting"];
    return (
      <div className="flex flex-col gap-14">
        {live.map((state) => (
          <CallRow key={state} state={state} elapsedSeconds={68} levels={sampleLevels(80, 3)} onEnd={() => {}} />
        ))}
        <CallRow state="ended" reason="you ended the call" onRestart={() => {}} onDismiss={() => {}} />
        <CallRow state="failed" reason="microphone blocked" onRestart={() => {}} onDismiss={() => {}} />
      </div>
    );
  },
};

/** A call as it sounds: a level every 60ms, scrolling left, flat in the pauses. */
export const Moving: Story = {
  render: () => {
    const [levels, setLevels] = useState<number[]>([]);
    const [seconds, setSeconds] = useState(0);
    useEffect(() => {
      let t = 0;
      const id = setInterval(() => {
        t += 1;
        setLevels((all) => [...all.slice(-399), t % 50 > 40 ? 0.02 : 0.2 + 0.7 * Math.random() * Math.abs(Math.sin(t / 3))]);
        setSeconds((s) => s + 0.06);
      }, 60);
      return () => clearInterval(id);
    }, []);
    return <CallRow state="speaking" levels={levels} elapsedSeconds={seconds} onEnd={() => {}} />;
  },
};
