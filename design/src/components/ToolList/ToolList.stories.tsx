import type { Meta, StoryObj } from "@storybook/react-vite";
import { Eyebrow } from "../Eyebrow/Eyebrow";
import { ToolList } from "./ToolList";

const meta = {
  title: "Components/Integrations & settings/ToolList",
  component: ToolList,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X61", replaces: null } },
  args: {
    tools: [
      { name: "brave_search_web", description: "Search the web; returns titles, links and snippets" },
      { name: "brave_search_news", description: "Search recent news articles" },
    ],
  },
} satisfies Meta<typeof ToolList>;

export default meta;
type Story = StoryObj<typeof meta>;

/** On an addon's card. */
export const Default: Story = {
  render: (args) => (
    <div className="flex flex-col gap-8">
      <Eyebrow>Tools it adds</Eyebrow>
      <ToolList {...args} />
    </div>
  ),
};
