// lint-tokens-exempt: a colour field has to be given colours to show.
import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { ColorField, type ColorFieldProps } from "./ColorField";

function Live(props: Partial<ColorFieldProps>) {
  const [value, setValue] = useState("#bd93f9");
  return (
    <div className="w-180">
      <ColorField label="Accent" {...props} value={value} onChange={setValue} />
    </div>
  );
}

const meta = {
  title: "Components/Actions & inputs/ColorField",
  component: ColorField,
  tags: ["approved"],
  parameters: { vizier: { id: "D65", replaces: null } },
  args: { label: "Accent", value: "#bd93f9", onChange: () => {} },
  render: (args) => <Live label={args.label} overridden={args.overridden} onReset={args.onReset} />,
} satisfies Meta<typeof ColorField>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Set by hand: marked, and it can go back to the value it would otherwise have. */
export const Overridden: Story = { args: { overridden: true, onReset: () => {} } };
