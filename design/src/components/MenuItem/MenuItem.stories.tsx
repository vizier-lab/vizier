import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { MenuItem } from "./MenuItem";

const meta = {
  title: "Components/Actions & inputs/MenuItem",
  component: MenuItem,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D22", replaces: null } },
  args: { icon: "pencil", label: "Rename", meta: "F2", tone: "default" },
  argTypes: { tone: { control: "inline-radio", options: ["default", "danger"] } },
  decorators: [(Story, ctx) => (ctx.name === "States" ? Story() : <div role="menu" aria-label="Actions" className="w-240">{Story()}</div>)],
} satisfies Meta<typeof MenuItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Danger: Story = { args: { icon: "trash", label: "Delete", meta: "⌫", tone: "danger" } };

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[
        {
          label: "default",
          cells: interactionCells(() => <div role="menu" aria-label="Actions" className="w-240"><MenuItem icon="pencil" label="Rename" meta="F2" /></div>, [
            { label: "Selected", node: <div role="menu" aria-label="Actions" className="w-240"><MenuItem role="menuitemradio" icon="pencil" label="Rename" selected /></div> },
            { label: "Disabled", node: <div role="menu" aria-label="Actions" className="w-240"><MenuItem icon="pencil" label="Rename" meta="F2" disabled /></div> },
          ]),
        },
        {
          label: "danger",
          cells: interactionCells(() => <div role="menu" aria-label="Actions" className="w-240"><MenuItem tone="danger" icon="trash" label="Delete" meta="⌫" /></div>, [
            { label: "Selected", node: <span /> },
            { label: "Disabled", node: <div role="menu" aria-label="Actions" className="w-240"><MenuItem tone="danger" icon="trash" label="Delete" disabled /></div> },
          ]),
        },
      ]}
    />
  ),
};
