import type { Meta, StoryObj } from "@storybook/react-vite";
import { AgentRow } from "../AgentRow/AgentRow";
import { SectionHeading } from "./SectionHeading";

const meta = {
  title: "Components/Mobile/SectionHeading",
  component: SectionHeading,
  tags: ["approved"],
  parameters: { vizier: { id: "X90", replaces: null } },
  args: { label: "Agents", onAdd: () => {}, addLabel: "New agent" },
  decorators: [(Story) => <div className="w-370">{Story()}</div>],
} satisfies Meta<typeof SectionHeading>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Without the permission to add, there is no +. */
export const ReadOnly: Story = { args: { onAdd: undefined } };

/** The phone's Home: two lists, each with its +, borderless rows, the current agent marked. */
export const PhoneHome: Story = {
  render: () => (
    <div className="flex flex-col gap-20">
      <section className="flex flex-col gap-4">
        <SectionHeading label="Agents" onAdd={() => {}} addLabel="New agent" />
        <AgentRow agentId="Archivist" name="Archivist" state="running" detail="replying" href="#" selected className="rounded-lg" />
        <AgentRow agentId="Scout" name="Scout" state="live" href="#" className="rounded-lg" />
        <AgentRow agentId="Quill" name="Quill" state="stopped" href="#" className="rounded-lg" />
      </section>
      <section className="flex flex-col gap-4">
        <SectionHeading label="Workspaces" onAdd={() => {}} addLabel="New workspace" />
      </section>
    </div>
  ),
};
