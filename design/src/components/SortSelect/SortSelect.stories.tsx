import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SortSelect } from "./SortSelect";

const meta = {
  title: "Components/Actions & inputs/SortSelect",
  component: SortSelect,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X12", replaces: null } },
  args: {
    label: "Sort",
    value: "next",
    options: [
      { value: "next", label: "Next run" },
      { value: "last", label: "Last run" },
      { value: "name", label: "Name" },
    ],
    onChange: () => {},
  },
  decorators: [(Story) => <div className="flex h-180 justify-end">{Story()}</div>],
} satisfies Meta<typeof SortSelect>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Toolbars: Tasks, Skills, MCP, Users. */
export const Bordered: Story = {
  render: function Render(args) {
    const [v, setV] = useState(args.value);
    return <SortSelect {...args} value={v} onChange={setV} />;
  },
};

/** Usage: break down by. */
export const BreakDown: Story = {
  render: function Render() {
    const [v, setV] = useState("activity");
    return (
      <SortSelect
        label="Break down by"
        value={v}
        onChange={setV}
        options={[
          { value: "activity", label: "Activity" },
          { value: "agent", label: "Agent" },
          { value: "model", label: "Model" },
          { value: "channel", label: "Channel" },
        ]}
      />
    );
  },
};

/** Inside panels: Memory bundles, Roles. */
export const Text: Story = {
  render: function Render() {
    const [v, setV] = useState("people");
    return (
      <SortSelect
        look="text"
        label="Sorted by"
        value={v}
        onChange={setV}
        options={[
          { value: "people", label: "people" },
          { value: "name", label: "name" },
          { value: "permissions", label: "permissions" },
        ]}
      />
    );
  },
};
