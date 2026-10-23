import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { PermissionGroup, PermissionRow } from "./PermissionRow";

const meta = {
  title: "Components/Agents & dreams/PermissionRow",
  component: PermissionRow,
  tags: ["approved"],
  parameters: { vizier: { id: "X57", replaces: "webui/app/components/RolesSection.tsx" } },
  args: { label: "Create agents", permission: "all_agents:create", granted: true, onGrantedChange: () => {} },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof PermissionRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Granted: Story = {};

export const NotGranted: Story = { args: { granted: false } };

/** A built-in role: shown, not editable. */
export const Disabled: Story = { args: { disabled: true } };

/** A group, as in the role editor. */
export const Group: Story = {
  render: function Render() {
    const [g, setG] = useState<Record<string, boolean>>({ view: false, create: true, edit: false, own: true });
    const rows: [string, string, string][] = [
      ["view", "See every agent", "all_agents:view"],
      ["create", "Create agents", "all_agents:create"],
      ["edit", "Edit any agent", "all_agents:edit"],
      ["own", "Edit agents they own", "owned_agents:edit"],
    ];
    return (
      <PermissionGroup title="Agents">
        {rows.map(([k, label, perm]) => (
          <PermissionRow key={k} label={label} permission={perm} granted={g[k]} onGrantedChange={(v) => setG({ ...g, [k]: v })} />
        ))}
      </PermissionGroup>
    );
  },
};
