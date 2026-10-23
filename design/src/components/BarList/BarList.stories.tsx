import type { Meta, StoryObj } from "@storybook/react-vite";
import { Table } from "../Table/Table";
import { TableRow } from "../TableRow/TableRow";
import { BarList, Meter } from "./BarList";

const meta = {
  title: "Components/Visualisation/BarList",
  component: BarList,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X71", replaces: null } },
  args: {
    items: [
      { label: "claude-sonnet-5-5", share: 0.58, value: "$10.68" },
      { label: "gpt-5-mini", share: 0.17, value: "$3.13" },
      { label: "claude-haiku-5-5", share: 0.11, value: "$2.03" },
      { label: "gemini-3-flash", share: 0.06, value: "$1.10" },
    ],
    other: { label: "Other · 14", share: 0.08, value: "$1.48" },
  },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof BarList>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Standalone, as in "Cost by model": the top few and Other. */
export const Default: Story = {};

/** A share column in a table, as in Usage's "Where it went". */
export const ShareColumn: Story = {
  decorators: [(Story) => <div className="w-580">{Story()}</div>],
  render: () => (
    <Table label="Where it went" columns={[{ label: "Activity", width: 140 }, { label: "Share of cost", width: 180 }, { label: "Cost" }, { label: "Calls", align: "end" }]}>
      {[
        ["Chat", 0.61, "ink", "$11.20", "1,402"],
        ["Background", 0.17, "chart-4", "$3.05", "388"],
        ["Dreams", 0.07, "chart-1", "$1.21", "124"],
        ["Compaction", 0.05, "chart-2", "$0.96", "31"],
      ].map(([l, s, c, v, n]) => (
        <TableRow
          key={l as string}
          endAligned={[3]}
          cells={[
            <span className="type-ui text-fg">{l}</span>,
            <Meter share={s as number} colour={c as "ink"} />,
            <span className="type-mono-sm text-fg">{v}</span>,
            <span className="type-mono-sm text-muted">{n}</span>,
          ]}
        />
      ))}
    </Table>
  ),
};
