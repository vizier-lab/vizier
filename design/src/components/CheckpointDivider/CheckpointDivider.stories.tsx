import type { Meta, StoryObj } from "@storybook/react-vite";
import { CheckpointDivider } from "./CheckpointDivider";

const meta = {
  title: "Components/Chat/CheckpointDivider",
  component: CheckpointDivider,
  tags: ["approved"],
  parameters: { vizier: { id: "D48", replaces: "webui/app/components/CheckpointDivider.tsx" } },
  args: { detail: "handed over at 81% of 200k" },
  decorators: [(Story) => <div className="w-640">{Story()}</div>],
} satisfies Meta<typeof CheckpointDivider>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const NoHandover: Story = { args: { detail: undefined } };
