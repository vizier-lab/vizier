import type { Meta, StoryObj } from "@storybook/react-vite";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { TextLink } from "./TextLink";

const meta = {
  title: "Components/Layout/TextLink",
  component: TextLink,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X7", replaces: null } },
  args: { children: "Retry" },
} satisfies Meta<typeof TextLink>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** As drawn across notices, jobs, reports and settings. */
export const InUse: Story = {
  render: () => (
    <div className="flex flex-col items-start gap-12">
      <span className="flex gap-14">
        <TextLink>Retry</TextLink>
        <TextLink>Switch model</TextLink>
      </span>
      <TextLink href="#" arrow="forward">Open Providers</TextLink>
      <TextLink href="#" arrow="forward" size="ui">See them in Users</TextLink>
      <TextLink href="#" arrow="external">Open run conversation</TextLink>
      <TextLink>+ Add variable</TextLink>
      <TextLink tone="danger" size="ui">Delete skill</TextLink>
    </div>
  ),
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  render: () => (
    <StateGrid
      rows={(["default", "danger"] as const).map((tone) => ({
        label: tone,
        cells: interactionCells(() => <TextLink tone={tone}>{tone === "danger" ? "Delete skill" : "Retry"}</TextLink>, [
          { label: "Disabled", node: <TextLink tone={tone} disabled>{tone === "danger" ? "Delete skill" : "Retry"}</TextLink> },
        ]),
      }))}
    />
  ),
};
