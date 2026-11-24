import type { Meta, StoryObj } from "@storybook/react-vite";
import { Badge } from "../Badge/Badge";
import { Row } from "./Row";

const meta = {
  title: "Components/Layout/Row",
  component: Row,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X84", replaces: null } },
  args: { gap: 8, align: "center", justify: "start", wrap: false },
  argTypes: {
    align: { control: "inline-radio", options: ["start", "center", "end", "stretch"] },
    justify: { control: "inline-radio", options: ["start", "center", "end", "between"] },
  },
  decorators: [(Story) => <div className="w-360">{Story()}</div>],
} satisfies Meta<typeof Row>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <Row {...args}>
      <Badge>one</Badge>
      <Badge>two</Badge>
      <Badge>three</Badge>
    </Row>
  ),
};

export const Spread: Story = { args: { justify: "between" }, render: Default.render };

export const Wrapping: Story = {
  args: { wrap: true },
  render: (args) => (
    <Row {...args}>
      {Array.from({ length: 14 }, (_, i) => (
        <Badge key={i}>item {i + 1}</Badge>
      ))}
    </Row>
  ),
};
