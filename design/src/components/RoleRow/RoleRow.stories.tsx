import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { RoleRow } from "./RoleRow";

const meta = {
  title: "Components/Integrations & settings/RoleRow",
  component: RoleRow,
  tags: ["approved"],
  parameters: { vizier: { id: "X67", replaces: "webui/app/components/RolesSection.tsx" } },
  args: { name: "member", permissions: 4, people: 38, onSelect: () => {} },
  decorators: [(Story) => <div className="w-290 divide-y divide-border rounded-xl border border-border bg-surface">{Story()}</div>],
} satisfies Meta<typeof RoleRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** The roles list, one selected. */
export const List: Story = {
  render: function Render() {
    const [sel, setSel] = useState("member");
    const roles: [string, boolean, number | "all", number][] = [
      ["superadmin", true, "all", 1],
      ["admin", true, 10, 3],
      ["member", false, 4, 38],
      ["viewer", false, 1, 24],
    ];
    return (
      <>
        {roles.map(([n, b, p, c]) => (
          <RoleRow key={n} name={n} builtIn={b} permissions={p} people={c} selected={sel === n} onSelect={() => setSel(n)} />
        ))}
      </>
    );
  },
};
