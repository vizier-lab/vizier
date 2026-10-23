import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ToggleChip } from "./ToggleChip";

const meta = {
  title: "Components/Actions & inputs/ToggleChip",
  component: ToggleChip,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X15", replaces: null } },
  args: { label: "Fetch Webpage", checked: true, onCheckedChange: () => {} },
} satisfies Meta<typeof ToggleChip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** New agent's "Always on" row. */
export const Default: Story = {
  render: function Render() {
    const [s, set] = useState({ fetch: true, http: false, webui: true });
    return (
      <div className="flex gap-12">
        <ToggleChip label="Fetch Webpage" checked={s.fetch} onCheckedChange={(v) => set({ ...s, fetch: v })} />
        <ToggleChip label="HTTP Client" checked={s.http} onCheckedChange={(v) => set({ ...s, http: v })} />
        <ToggleChip label="WebUI messaging" checked={s.webui} onCheckedChange={(v) => set({ ...s, webui: v })} />
      </div>
    );
  },
};

export const Off: Story = { args: { checked: false } };
