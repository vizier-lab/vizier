import type { Meta, StoryObj } from "@storybook/react-vite";
import { DropZone } from "./DropZone";

const meta = {
  title: "Components/Actions & inputs/DropZone",
  component: DropZone,
  tags: ["approved"],
  parameters: { vizier: { id: "X19", replaces: null } },
  args: { onFiles: () => {} },
  decorators: [(Story) => <div className="w-271">{Story()}</div>],
} satisfies Meta<typeof DropZone>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Skill editor resources. */
export const Inline: Story = {};

/** Chat: covers the column while files are dragged over it. */
export const Overlay: Story = {
  decorators: [(Story) => <div className="relative h-320 w-560 rounded-xl border border-border bg-surface">{Story()}</div>],
  args: { look: "overlay", active: true, hint: "Files, images and audio" },
};
