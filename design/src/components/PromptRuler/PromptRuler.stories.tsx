import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { PromptRuler } from "./PromptRuler";

const SAMPLE = [
  "buat function buat nyari info cuaca",
  "make it cache results for 10 minutes",
  "why is the hourly table empty for Tokyo?",
  "add Bandung and Tokyo to the defaults",
  "ok, cache it per city",
  "and add an hourly forecast for tomorrow",
  "retry open-meteo on 502",
  "compare open-meteo with BMKG data",
  "can it warn me before it rains?",
];

const preview = (index: number) => ({
  text: SAMPLE[index % SAMPLE.length],
  time: `${String(9 + (index % 12)).padStart(2, "0")}:${String((index * 7) % 60).padStart(2, "0")}`,
});

/** Holds `current` the way the chat does: a jump moves it. */
function Live({ total, start }: { total: number; start: number }) {
  const [current, setCurrent] = useState(start);
  return (
    <div className="flex h-420 items-center bg-panel p-16">
      <PromptRuler total={total} current={current} preview={preview} onJump={setCurrent} />
    </div>
  );
}

const meta = {
  title: "Components/Chat/PromptRuler",
  component: PromptRuler,
  tags: ["approved"],
  parameters: { vizier: { id: "X92", replaces: null } },
  args: { total: 9, current: 7, preview, onJump: () => {} },
} satisfies Meta<typeof PromptRuler>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A few prompts: every one shown, the one being read long. */
export const Few: Story = { render: () => <Live total={9} start={7} /> };

/** 1,240 prompts: a window around the current one, every tenth numbered. Scroll or drag it. */
export const Many: Story = { render: () => <Live total={1240} start={411} /> };

/** At the very start of a long conversation. */
export const AtTheStart: Story = { render: () => <Live total={1240} start={0} /> };
