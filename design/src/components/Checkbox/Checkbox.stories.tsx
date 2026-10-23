import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { Checkbox } from "./Checkbox";

const meta = {
  title: "Components/Actions & inputs/Checkbox",
  component: Checkbox,
  tags: ["approved"],
  parameters: { vizier: { id: "D19", replaces: null } },
  args: { label: "Read memory", defaultChecked: true },
} satisfies Meta<typeof Checkbox>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[true, false].map((checked) => ({
        label: checked ? "checked" : "unchecked",
        cells: interactionCells(() => <Checkbox aria-label="Read memory" defaultChecked={checked} />, [
          { label: "Disabled", node: <Checkbox aria-label="Read memory" defaultChecked={checked} disabled /> },
        ]),
      }))}
    />
  ),
};
