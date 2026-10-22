import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { Switch } from "./Switch";

const meta = {
  title: "Components/Actions & inputs/Switch",
  component: Switch,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D18", replaces: null } },
  args: { checked: true, "aria-label": "Enabled" },
} satisfies Meta<typeof Switch>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [on, setOn] = useState(args.checked);
    return <Switch {...args} checked={on} onCheckedChange={setOn} />;
  },
};

export const States: Story = {
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[true, false].map((checked) => ({
        label: checked ? "on" : "off",
        cells: interactionCells(() => <Switch checked={checked} aria-label="Enabled" />, [
          { label: "Disabled", node: <Switch checked={checked} aria-label="Enabled" disabled /> },
        ]),
      }))}
    />
  ),
};
