import type { Meta, StoryObj } from "@storybook/react-vite";
import { MentionChip } from "./MentionChip";

const meta = {
  title: "Components/Chat/MentionChip",
  component: MentionChip,
  tags: ["approved"],
  parameters: { vizier: { id: "X60", replaces: null } },
  args: { handle: "archivist", tone: "resolved" },
  argTypes: { tone: { control: "inline-radio", options: ["resolved", "you"] } },
} satisfies Meta<typeof MentionChip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** In a sentence, with one for the signed-in person. */
export const InText: Story = {
  render: () => (
    <p className="type-prose text-fg">
      <MentionChip handle="archivist" /> have we seen this lock before? Ask <MentionChip handle="maya" tone="you" /> if it
      repeats.
    </p>
  ),
};
