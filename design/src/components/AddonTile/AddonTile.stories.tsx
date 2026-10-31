import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Switch } from "../Switch/Switch";
import { AddonTile } from "./AddonTile";

const meta = {
  title: "Components/Integrations & settings/AddonTile",
  component: AddonTile,
  tags: ["approved"],
  parameters: { vizier: { id: "X66", replaces: "the old Tools tab" } },
  args: {
    kind: "brave_search",
    name: "Brave Search",
    description: "Search the web and the news with Brave Search",
  },
} satisfies Meta<typeof AddonTile>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Nothing installed: the tile offers the add. */
export const Available: Story = {
  args: {
    children: (
      <>
        <span className="type-small text-faint">Needs setup</span>
        <Button size="sm" icon="plus">
          Add
        </Button>
      </>
    ),
  },
};

/** On the agent: a switch and the way into its settings. */
export const Installed: Story = {
  args: {
    kind: "office",
    name: "Office",
    description: "Read and create Word and Excel files: Markdown, typed cells and CSV previews",
    children: (
      <>
        <span className="flex items-center gap-8 type-ui text-muted">
          <Switch checked aria-label="Office enabled" />
          On
        </span>
        <Button size="sm" variant="ghost" icon="settings">
          Manage
        </Button>
      </>
    ),
  },
};

/** Chosen in the creation wizard: the tile carries the live border. */
export const Chosen: Story = {
  args: {
    kind: "http",
    name: "HTTP",
    description: "Fetch webpages and call HTTP APIs, so the agent can read and post to the web",
    selected: true,
    children: (
      <>
        <span className="type-small text-faint">Added on creation</span>
        <Button size="sm" variant="ghost" icon="settings">
          Settings
        </Button>
      </>
    ),
  },
};
