import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { OutlineNav } from "./OutlineNav";

const ITEMS = [
  { id: "code-review", label: "Code review", level: 1 as const },
  { id: "when", label: "When to use", level: 2 as const },
  { id: "steps", label: "Steps", level: 2 as const },
  { id: "severity", label: "Severity", level: 3 as const },
  { id: "example", label: "Example", level: 2 as const },
  { id: "output", label: "Output format", level: 2 as const },
  { id: "not", label: "What not to flag", level: 2 as const },
];

const meta = {
  title: "Components/Rich text/OutlineNav",
  component: OutlineNav,
  tags: ["approved"],
  parameters: { vizier: { id: "X51", replaces: null } },
  args: { items: ITEMS, activeId: "steps", onSelect: () => {} },
  decorators: [(Story) => <div className="w-162">{Story()}</div>],
} satisfies Meta<typeof OutlineNav>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [a, setA] = useState(args.activeId);
    return <OutlineNav {...args} activeId={a} onSelect={setA} />;
  },
};
