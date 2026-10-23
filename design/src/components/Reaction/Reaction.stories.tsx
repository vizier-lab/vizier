import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, statePseudo } from "../../stories/StateGrid";
import { AddReaction, Reaction } from "./Reaction";

const meta = {
  title: "Components/Chat/Reaction",
  component: Reaction,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D55", replaces: "webui/app/components/ReactionBadges.tsx" } },
  args: { emoji: "👍", count: 1 },
} satisfies Meta<typeof Reaction>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Under a reply, with the add button. */
export const Row: Story = {
  render: () => (
    <div className="flex items-center gap-6">
      <Reaction emoji="👍" count={2} mine />
      <Reaction emoji="🎉" count={1} />
      <AddReaction />
    </div>
  ),
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[
        {
          cells: [
            { label: "Default", node: <Reaction emoji="👍" count={1} /> },
            { label: "Hover", pseudo: "hover", node: <Reaction emoji="👍" count={1} /> },
            { label: "Mine", node: <Reaction emoji="👍" count={1} mine /> },
            { label: "Focus", pseudo: "focus-visible", node: <Reaction emoji="👍" count={1} /> },
          ],
        },
      ]}
    />
  ),
};
