import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { PanelHeader } from "../PanelHeader/PanelHeader";
import { RevisionRow } from "./RevisionRow";

const meta = {
  title: "Components/Lists/RevisionRow",
  component: RevisionRow,
  tags: ["approved"],
  parameters: { vizier: { id: "X27", replaces: "webui/app/components/VersionHistory.tsx" } },
  args: { revision: 41, author: "dream cycle", time: "today 03:12", added: 2, removed: 1, onSelect: () => {} },
  decorators: [(Story) => <div className="w-380">{Story()}</div>],
} satisfies Meta<typeof RevisionRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** Core's History panel. */
export const History: Story = {
  render: function Render() {
    const [sel, setSel] = useState(41);
    const revs: [number, string, string, number, number][] = [
      [41, "dream cycle", "today 03:12", 2, 1],
      [40, "ganesha · WebUI", "yesterday 18:40", 5, 0],
      [39, "Archivist · write_core", "Oct 17, 14:05", 1, 1],
      [38, "dream cycle", "Oct 17, 03:10", 3, 2],
    ];
    return (
      <div className="flex flex-col rounded-xl border border-border bg-surface">
        <PanelHeader title="History" meta="41 revisions" className="px-16 py-16" />
        {revs.map(([r, a, t, add, rem]) => (
          <RevisionRow key={r} revision={r} author={a} time={t} added={add} removed={rem} selected={sel === r} onSelect={() => setSel(r)} />
        ))}
      </div>
    );
  },
};
