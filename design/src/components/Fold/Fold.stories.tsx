import type { Meta, StoryObj } from "@storybook/react-vite";
import { Fold } from "./Fold";

const meta = {
  title: "Components/Chat/Fold",
  component: Fold,
  tags: ["approved"],
  parameters: { vizier: { id: "D54", replaces: null } },
  args: { label: "9 more · 1 thought, 8 python runs" },
  decorators: [(Story) => <ul className="w-480 rounded-lg bg-surface p-12">{Story()}</ul>],
} satisfies Meta<typeof Fold>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
