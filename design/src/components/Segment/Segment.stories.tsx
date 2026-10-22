import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { Segment } from "./Segment";

const meta = {
  title: "Components/Actions & inputs/Segment",
  component: Segment,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D16", replaces: null } },
  args: { children: "All", count: 48, selected: true },
} satisfies Meta<typeof Segment>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const States: Story = {
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[true, false].map((selected) => ({
        label: selected ? "selected" : "not selected",
        cells: interactionCells(() => <Segment selected={selected} count={48}>All</Segment>, [
          { label: "Disabled", node: <Segment selected={selected} count={48} disabled>All</Segment> },
        ]),
      }))}
    />
  ),
};
