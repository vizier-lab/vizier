import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { InlineSelect } from "./InlineSelect";

const meta = {
  title: "Components/Actions & inputs/InlineSelect",
  component: InlineSelect,
  tags: ["approved"],
  parameters: { vizier: { id: "X13", replaces: null } },
  args: { label: "Vision model", value: "Anthropic · claude-haiku-5-5" },
} satisfies Meta<typeof InlineSelect>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[
        {
          cells: interactionCells(() => <InlineSelect label="Vision model" value="Anthropic · claude-haiku-5-5" />, [
            { label: "Disabled", node: <InlineSelect label="Vision model" value="Anthropic · claude-haiku-5-5" disabled /> },
          ]),
        },
      ]}
    />
  ),
};
