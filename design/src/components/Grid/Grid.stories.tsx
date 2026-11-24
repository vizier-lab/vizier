import type { Meta, StoryObj } from "@storybook/react-vite";
import { Card } from "../Card/Card";
import { Grid } from "./Grid";

const meta = {
  title: "Components/Layout/Grid",
  component: Grid,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X85", replaces: null } },
  args: { cols: 3, gap: 12 },
  argTypes: { cols: { control: { type: "range", min: 1, max: 12, step: 1 } } },
} satisfies Meta<typeof Grid>;

export default meta;
type Story = StoryObj<typeof meta>;

const cells = Array.from({ length: 6 }, (_, i) => <Card key={i}>Cell {i + 1}</Card>);

export const Default: Story = { render: (args) => <Grid {...args}>{cells}</Grid> };

/** Fewer columns as the container narrows; resize the canvas to see it. */
export const Responsive: Story = {
  args: { cols: 4, minCol: 160 },
  render: (args) => <Grid {...args}>{cells}</Grid>,
};
