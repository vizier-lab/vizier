import type { Meta, StoryObj } from "@storybook/react-vite";
import { JumpToLatest } from "./JumpToLatest";

const meta = {
  title: "Components/Chat/JumpToLatest",
  component: JumpToLatest,
  tags: ["approved"],
  parameters: { vizier: { id: "X43", replaces: null } },
  args: { count: 3 },
} satisfies Meta<typeof JumpToLatest>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const NoCount: Story = { args: { count: 0 } };
