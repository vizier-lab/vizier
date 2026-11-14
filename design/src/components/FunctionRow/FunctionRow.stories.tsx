import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { FunctionGroup, FunctionRow } from "./FunctionRow";

const meta = {
  title: "Components/Agents & dreams/FunctionRow",
  component: FunctionRow,
  parameters: { vizier: { id: "061-function-row" } },
  args: {
    callAs: "custom_weekly_digest",
    description: "Summarise a topic's notes from the last week into the digests bundle",
    version: 3,
    meta: "a dream · 2 h ago",
    enabled: true,
    onEnabledChange: () => {},
    href: "#",
  },
  decorators: [(Story) => <div className="w-720">{Story()}</div>],
} satisfies Meta<typeof FunctionRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const ModelBacked: Story = { args: { modelBacked: true } };

export const NeedsAttention: Story = { args: { callAs: "custom_send_report", version: 1, missing: ["stripe_list_charges"] } };

export const SwitchedOff: Story = { args: { enabled: false, offNote: "switched off by alice" } };

/** A person who can only view the agent: no switch. */
export const ViewOnly: Story = { args: { onEnabledChange: undefined } };

/** The page: categories as groups, `custom` last. */
export const Groups: Story = {
  render: function Render() {
    const [on, setOn] = useState({ digest: true, send: true, sum: false });
    return (
      <div className="flex flex-col gap-14">
        <FunctionGroup name="reports" description="Pull and send the weekly status reports" onEdit={() => {}}>
          <FunctionRow callAs="custom_weekly_digest" description="Summarise a topic's notes" version={3} meta="a dream · 2 h ago" modelBacked enabled={on.digest} onEnabledChange={(v) => setOn({ ...on, digest: v })} href="#" />
          <FunctionRow callAs="custom_send_report" description="Post the digest to the team channel" version={1} meta="you · yesterday" missing={["stripe_list_charges"]} enabled={on.send} onEnabledChange={(v) => setOn({ ...on, send: v })} href="#" />
        </FunctionGroup>
        <FunctionGroup name="maths" description="Small arithmetic helpers" onEdit={() => {}}>
          <FunctionRow callAs="custom_echo_sum" description="Add two integers" version={4} meta="alice · 3 d ago" offNote="switched off by alice" enabled={on.sum} onEnabledChange={(v) => setOn({ ...on, sum: v })} href="#" />
        </FunctionGroup>
      </div>
    );
  },
};
