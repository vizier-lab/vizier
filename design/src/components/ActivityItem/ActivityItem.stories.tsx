import type { Meta, StoryObj } from "@storybook/react-vite";
import { Sigil } from "../../brand/Sigil/Sigil";
import { ActivityItem } from "./ActivityItem";

const meta = {
  title: "Components/Lists/ActivityItem",
  component: ActivityItem,
  tags: ["approved"],
  parameters: { vizier: { id: "X26", replaces: null } },
  args: { actor: "ganesha", time: "Sep 14", text: "Installed Brave Search." },
  decorators: [(Story) => <div className="w-340">{Story()}</div>],
} satisfies Meta<typeof ActivityItem>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The integration log. */
export const Plain: Story = {};

/** Home's "Since you were away", with sigils. */
export const WithSigils: Story = {
  render: () => (
    <div className="flex flex-col gap-8">
      <ActivityItem leading={<Sigil agentId="Scribe" size="md" />} actor="Scribe" time="21:04" failed text='Failed to start: model "llama3.3:70b" not found. Retrying in 40s.' />
      <ActivityItem leading={<Sigil agentId="Archivist" size="md" />} actor="Archivist" time="21:02" text="Answering Scout's report in vector-db-review." />
      <ActivityItem leading={<Sigil agentId="Treasurer" size="md" />} actor="Treasurer" time="07:00" text='Task "Morning cost digest" answered: $3.87 yesterday, 412k tokens.' />
    </div>
  ),
};
