import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { IconButton, IconButtonGroup } from "./IconButton";

const meta = {
  title: "Components/Actions & inputs/IconButton",
  component: IconButton,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D11", replaces: null } },
  args: { icon: "history", "aria-label": "Version history", variant: "secondary", size: "md" },
  argTypes: {
    variant: { control: "inline-radio", options: ["secondary", "ghost"] },
    size: { control: "inline-radio", options: ["sm", "md"] },
  },
} satisfies Meta<typeof IconButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={(["secondary", "ghost"] as const).map((variant) => ({
        label: variant,
        cells: interactionCells(
          () => <IconButton variant={variant} icon="history" aria-label="Version history" />,
          [
            { label: "Disabled", node: <IconButton variant={variant} icon="history" aria-label="Version history" disabled /> },
            { label: "Selected", node: <IconButton variant={variant} icon="history" aria-label="Version history" selected /> },
          ],
        ),
      }))}
    />
  ),
};

/** Sm, for the close × in panels. */
export const Small: Story = { args: { icon: "close", "aria-label": "Close panel", variant: "ghost", size: "sm" } };

/** The vertical group on the memory graph. */
export const Group: Story = {
  render: () => (
    <IconButtonGroup aria-label="Graph zoom">
      <IconButton variant="ghost" icon="zoom-in" aria-label="Zoom in" />
      <IconButton variant="ghost" icon="zoom-out" aria-label="Zoom out" />
      <IconButton variant="ghost" icon="maximize" aria-label="Fit to view" />
    </IconButtonGroup>
  ),
};
