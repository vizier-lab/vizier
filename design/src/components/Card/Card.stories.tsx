import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../Button/Button";
import { Switch } from "../Switch/Switch";
import { TextField } from "../TextField/TextField";
import { Card } from "./Card";

const meta = {
  title: "Components/Surfaces & data/Card",
  component: Card,
  tags: ["needs-review"],
  parameters: { vizier: { id: "D34", replaces: null } },
  args: {
    title: "Model",
    description: "From the models.dev catalogue and your custom providers.",
    children: <TextField label="Model" mono defaultValue="qwen3:32b" />,
  },
  decorators: [(Story) => <div className="w-520">{Story()}</div>],
} satisfies Meta<typeof Card>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** A header switch, as on integration cards. */
export const WithHeaderSwitch: Story = {
  args: {
    title: "Self-management",
    description: "Let Archivist add and change its own MCP servers.",
    headerTrailing: <Switch checked aria-label="Self-management" />,
    children: undefined,
  },
};

/** The Danger zone. */
export const Danger: Story = {
  args: {
    tone: "danger",
    title: "Danger zone",
    description: "Deleting Scribe removes its memory, tasks and sessions.",
    children: (
      <div>
        <Button variant="danger" icon="trash">Delete Scribe</Button>
      </div>
    ),
  },
};

export const Dashed: Story = {
  args: { tone: "dashed", title: "Nothing pinned yet", description: "Pin a conversation to keep it here.", children: undefined },
};
