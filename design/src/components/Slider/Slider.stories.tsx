import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Slider } from "./Slider";

const meta = {
  title: "Components/Actions & inputs/Slider",
  component: Slider,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X86", replaces: null } },
  args: { label: "Roughness", value: 40, min: 0, max: 100, step: 1, onChange: () => {} },
  decorators: [(Story) => <div className="w-360">{Story()}</div>],
} satisfies Meta<typeof Slider>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [value, setValue] = useState(args.value);
    return <Slider {...args} value={value} onChange={setValue} />;
  },
};

export const Stepped: Story = { args: { min: 0, max: 1, step: 0.1, value: 0.5, label: "Opacity" }, render: Default.render };

export const Disabled: Story = { args: { disabled: true }, render: Default.render };
