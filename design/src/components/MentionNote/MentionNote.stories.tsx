import type { Meta, StoryObj } from "@storybook/react-vite";
import { MentionNote } from "./MentionNote";

const meta = {
  title: "Components/Chat/MentionNote",
  component: MentionNote,
  tags: ["approved"],
  parameters: { vizier: { id: "X61", replaces: null } },
  args: { children: "scribe is restarting, so it was not woken" },
} satisfies Meta<typeof MentionNote>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
