import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Stack } from "./Stack";

const meta = {
  title: "Components/Layout/Stack",
  component: Stack,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X83", replaces: null } },
  args: { gap: 12, align: "stretch" },
  argTypes: {
    gap: { control: "select", options: [0, 2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 32, 48] },
    align: { control: "inline-radio", options: ["start", "center", "end", "stretch"] },
  },
  decorators: [(Story) => <div className="w-360">{Story()}</div>],
} satisfies Meta<typeof Stack>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: (args) => (
    <Stack {...args}>
      <Button variant="primary">First</Button>
      <Button variant="secondary">Second</Button>
      <Button variant="secondary">Third</Button>
    </Stack>
  ),
};

export const Aligned: Story = { args: { align: "start", gap: 8 }, render: Default.render };
