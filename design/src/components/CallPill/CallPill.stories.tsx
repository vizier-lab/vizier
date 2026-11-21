import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import { CallPill, type CallState } from "./CallPill";

const meta = {
  title: "Components/Chat/CallPill",
  component: CallPill,
  tags: ["approved"],
  parameters: { vizier: { id: "D66", replaces: "LiveVoiceRow in the composer's meta line" } },
  args: { state: "listening", elapsedSeconds: 42, level: 0.5, onEnd: () => {} },
  argTypes: {
    state: {
      control: "select",
      options: ["connecting", "listening", "speaking", "interrupted", "reconnecting", "ended", "failed"],
    },
    level: { control: { type: "range", min: 0, max: 1, step: 0.05 } },
  },
} satisfies Meta<typeof CallPill>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Listening: the meter follows the microphone. */
export const Listening: Story = {};

/** Every state, as the lip shows it. Connecting waits with dots; reconnecting is amber. The agent working is not a call state: the activity trail shows it. */
export const States: Story = {
  render: () => {
    const live: CallState[] = ["connecting", "listening", "speaking", "interrupted", "reconnecting"];
    return (
      <div className="flex flex-col items-start gap-12">
        {live.map((state) => (
          <CallPill key={state} state={state} elapsedSeconds={68} level={0.6} onEnd={() => {}} />
        ))}
        <CallPill state="ended" reason="you ended the call" onRestart={() => {}} onDismiss={() => {}} />
        <CallPill state="failed" reason="microphone blocked" onRestart={() => {}} onDismiss={() => {}} />
      </div>
    );
  },
};

/** A level that moves, as a call's does. */
export const Moving: Story = {
  render: () => {
    const [level, setLevel] = useState(0.3);
    const [seconds, setSeconds] = useState(0);
    useEffect(() => {
      const t = setInterval(() => {
        setLevel(Math.random());
        setSeconds((s) => s + 0.15);
      }, 150);
      return () => clearInterval(t);
    }, []);
    return <CallPill state="speaking" level={level} elapsedSeconds={seconds} onEnd={() => {}} />;
  },
};
