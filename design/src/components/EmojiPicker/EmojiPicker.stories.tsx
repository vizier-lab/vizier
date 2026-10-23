import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { AddReaction, Reaction } from "../Reaction/Reaction";
import { EmojiPicker } from "./EmojiPicker";

const meta = {
  title: "Components/Actions & inputs/EmojiPicker",
  component: EmojiPicker,
  tags: ["approved"],
  parameters: { vizier: { id: "X78", replaces: null } },
  args: { onSelect: () => {}, trigger: <AddReaction /> },
  decorators: [(Story) => <div className="h-360 w-360 p-24">{Story()}</div>],
} satisfies Meta<typeof EmojiPicker>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Open from the add-reaction button; the pick lands as a Reaction. */
export const Default: Story = {
  render: function Render(args) {
    const [picked, setPicked] = useState<string[]>(["👍"]);
    return (
      <div className="flex items-center gap-6">
        {picked.map((e) => (
          <Reaction key={e} emoji={e} count={1} mine />
        ))}
        <EmojiPicker {...args} onSelect={(e) => setPicked((p) => (p.includes(e) ? p : [...p, e]))} />
      </div>
    );
  },
};

export const Open: Story = { args: { open: true } };
export const NoRecent: Story = { args: { open: true, recent: [] } };
