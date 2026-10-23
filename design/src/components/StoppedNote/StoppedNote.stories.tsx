import type { Meta, StoryObj } from "@storybook/react-vite";
import { StoppedNote } from "./StoppedNote";

const meta = {
  title: "Components/Chat/StoppedNote",
  component: StoppedNote,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X32", replaces: null } },
  args: { children: "Stopped by you at 14:03 · 1 queued message dropped" },
} satisfies Meta<typeof StoppedNote>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
