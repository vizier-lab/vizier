import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { MenuItem } from "../MenuItem/MenuItem";
import { Menu, MenuDivider } from "./Menu";

const meta = {
  title: "Components/Actions & inputs/Menu",
  component: Menu,
  tags: ["approved"],
  parameters: { vizier: { id: "D21", replaces: null } },
  args: { "aria-label": "Document actions", children: null },
} satisfies Meta<typeof Menu>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A row's action menu. ↑ ↓ move between items. */
export const Default: Story = {
  render: (args) => (
    <Menu {...args}>
      <MenuItem icon="pencil" label="Rename" meta="F2" />
      <MenuItem icon="copy" label="Duplicate" />
      <MenuItem icon="download" label="Export as .zip" />
      <MenuItem icon="history" label="Move to bundle" disabled />
      <MenuDivider />
      <MenuItem icon="trash" label="Delete" tone="danger" />
    </Menu>
  ),
};

/** Past about eight items, a filter field leads. */
export const WithSearch: Story = {
  render: function Render(args) {
    const all = ["Rename", "Duplicate", "Export as .zip", "Move to bundle", "Copy link", "Pin", "Show history", "Restore revision", "Open in editor", "Archive"];
    const [q, setQ] = useState("");
    const shown = all.filter((a) => a.toLowerCase().includes(q.toLowerCase()));
    return (
      <Menu {...args} search={{ value: q, onChange: setQ, placeholder: "Search actions" }}>
        {shown.map((a) => (
          <MenuItem key={a} label={a} />
        ))}
      </Menu>
    );
  },
};
