import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { DateTimeField } from "./DateTimeField";

const meta = {
  title: "Components/Actions & inputs/DateTimeField",
  component: DateTimeField,
  tags: ["approved"],
  parameters: { vizier: { id: "X77", replaces: null } },
  args: { label: "Run at", value: null, onChange: () => {} },
  decorators: [(Story) => <div className="h-440 w-320">{Story()}</div>],
} satisfies Meta<typeof DateTimeField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {
  render: function Render(args) {
    const [value, setValue] = useState<Date | null>(null);
    return <DateTimeField {...args} value={value} onChange={setValue} />;
  },
};

export const Filled: Story = {
  render: function Render(args) {
    const [value, setValue] = useState<Date | null>(new Date(2026, 9, 24, 14, 30));
    return <DateTimeField {...args} value={value} onChange={setValue} helper="In your time zone" />;
  },
};

/** Days before `min` can't be picked, and an earlier time on the first allowed day is lifted to it. */
export const WithMin: Story = {
  render: function Render(args) {
    const [value, setValue] = useState<Date | null>(null);
    return <DateTimeField {...args} value={value} onChange={setValue} min={new Date()} helper="Must be in the future" />;
  },
};

export const Invalid: Story = { args: { invalid: true, helper: "Pick a time in the future" } };
export const Disabled: Story = { args: { disabled: true } };
export const Open: Story = {
  render: function Render(args) {
    const [value, setValue] = useState<Date | null>(new Date(2026, 9, 24, 14, 30));
    return <DateTimeField {...args} value={value} onChange={setValue} />;
  },
  play: async ({ canvasElement }) => {
    canvasElement.querySelector<HTMLButtonElement>("button")?.click();
  },
};
