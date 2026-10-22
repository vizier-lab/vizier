import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { Radio } from "./Radio";

const meta = {
  title: "Components/Actions & inputs/Radio",
  component: Radio,
  tags: ["needs-review"],
  parameters: {
    vizier: { id: "D20", replaces: null },
    docs: { description: { component: "Not drawn on any screen yet; kept as designed for short exclusive choices (shell: Docker or Local)." } },
  },
  args: { label: "Docker", name: "shell", defaultChecked: true },
} satisfies Meta<typeof Radio>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** A group, as for the shell backend. */
export const Group: Story = {
  render: () => (
    <fieldset className="flex flex-col gap-10 border-0 p-0">
      <legend className="pb-8 type-ui-medium text-soft">Shell</legend>
      <Radio name="shell-group" label="Docker" defaultChecked />
      <Radio name="shell-group" label="Local" />
    </fieldset>
  ),
};

export const States: Story = {
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={[true, false].map((checked) => ({
        label: checked ? "selected" : "not selected",
        cells: interactionCells((l) => <Radio name={`r-${checked}-${l}`} label={checked ? "Docker" : "Local"} defaultChecked={checked} />, [
          { label: "Disabled", node: <Radio name={`r-${checked}-d`} label={checked ? "Docker" : "Local"} defaultChecked={checked} disabled /> },
        ]),
      }))}
    />
  ),
};
