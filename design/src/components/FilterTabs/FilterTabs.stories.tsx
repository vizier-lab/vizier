import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { FilterTabs } from "./FilterTabs";

const meta = {
  title: "Components/Actions & inputs/FilterTabs",
  component: FilterTabs,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D17", replaces: null } },
} satisfies Meta<typeof FilterTabs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A filter with counts, as on Home. */
export const Default: Story = {
  args: { options: [], value: "", onChange: () => {}, "aria-label": "Filter agents" },
  render: function Render() {
    const [v, setV] = useState("all");
    return (
      <FilterTabs
        aria-label="Filter agents"
        value={v}
        onChange={setV}
        options={[
          { value: "all", label: "All", count: 48 },
          { value: "mine", label: "Mine", count: 12 },
          { value: "live", label: "Live", count: 2 },
          { value: "failed", label: "Failed", count: 1 },
        ]}
      />
    );
  },
};

/** Mode switches without counts, as drawn across the app. */
export const ModeSwitches: Story = {
  args: Default.args,
  render: function Render() {
    const sets: [string, string[]][] = [
      ["Editor mode", ["Rich", "Markdown", "Split"]],
      ["Shell", ["Local", "Docker"]],
      ["Server", ["Remote URL", "Local command"]],
      ["Range", ["7 days", "30 days", "Custom"]],
      ["View", ["Table", "Raw"]],
      ["Bundles", ["Largest", "Recently changed", "Most linked"]],
    ];
    const [state, setState] = useState<Record<string, string>>(Object.fromEntries(sets.map(([k, o]) => [k, o[0]])));
    return (
      <div className="flex flex-col items-start gap-12">
        {sets.map(([label, opts]) => (
          <FilterTabs
            key={label}
            aria-label={label}
            value={state[label]}
            onChange={(v) => setState((s) => ({ ...s, [label]: v }))}
            options={opts.map((o) => ({ value: o, label: o }))}
          />
        ))}
      </div>
    );
  },
};
