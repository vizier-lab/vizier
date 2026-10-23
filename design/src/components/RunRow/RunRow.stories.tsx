import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Eyebrow } from "../Eyebrow/Eyebrow";
import { RunRow } from "./RunRow";

const meta = {
  title: "Components/Lists/RunRow",
  component: RunRow,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X28", replaces: null } },
  args: { time: "Oct 20, 09:00", state: "answered", duration: "41s", onSelect: () => {} },
  decorators: [(Story) => <div className="w-320">{Story()}</div>],
} satisfies Meta<typeof RunRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** A task's runs, as in its detail panel. */
export const Runs: Story = {
  render: function Render() {
    const [sel, setSel] = useState(0);
    const runs: [string, React.ComponentProps<typeof RunRow>["state"], string | undefined][] = [
      ["Oct 20, 09:00", "answered", "41s"],
      ["Oct 13, 09:00", "answered", "1m 02s"],
      ["Oct 6, 09:00", "no-response", undefined],
      ["Sep 29, 09:00", "interrupted", undefined],
    ];
    return (
      <div className="flex flex-col gap-6">
        <Eyebrow>Runs</Eyebrow>
        {runs.map(([t, s, d], i) => (
          <RunRow key={t} time={t} state={s} duration={d} selected={sel === i} onSelect={() => setSel(i)} />
        ))}
      </div>
    );
  },
};
