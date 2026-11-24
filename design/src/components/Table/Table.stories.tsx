import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Badge } from "../Badge/Badge";
import { Button } from "../Button/Button";
import { IconButton } from "../IconButton/IconButton";
import { Pagination } from "../Pagination/Pagination";
import { Status } from "../Status/Status";
import { Switch } from "../Switch/Switch";
import { TableRow, TableTitle } from "../TableRow/TableRow";
import { Table } from "./Table";

const meta = {
  title: "Components/Surfaces & data/Table",
  component: Table,
  tags: ["approved"],
  parameters: { vizier: { id: "D36", replaces: null } },
  args: { label: "Tasks", columns: [], children: null },
  decorators: [(Story) => <div className="w-760">{Story()}</div>],
} satisfies Meta<typeof Table>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tasks: two-line cells, a status, a toggle column and an action column, with pagination. */
export const Default: Story = {
  render: function Render() {
    const [sel, setSel] = useState("Price watch");
    const [on, setOn] = useState<Record<string, boolean>>({ "Morning brief": true, "Weekly memory tidy": true, "Price watch": false });
    const rows: [string, string, React.ComponentProps<typeof Status>["state"], string][] = [
      ["Morning brief", "every weekday · 07:00", "answered", "ganesha"],
      ["Weekly memory tidy", "Sundays · 03:00", "no-response", "Archivist"],
      ["Price watch", "every hour", "running", "ganesha"],
    ];
    return (
      <Table
        label="Tasks"
        columns={[
          { label: "Task", width: 280 },
          { label: "Latest run", width: 180 },
          { label: "Requested by", width: 140 },
          { label: "On", width: 70 },
          { label: "", srLabel: "Actions", align: "end" },
        ]}
        footer={<Pagination page={1} pageCount={1} total={3} perPage={25} onPageChange={() => {}} />}
      >
        {rows.map(([t, s, st, who]) => (
          <TableRow
            key={t}
            selected={sel === t}
            onSelect={() => setSel(t)}
            endAligned={[4]}
            cells={[
              <TableTitle title={t} subtitle={s} />,
              <Status state={st} font="sans" />,
              <span className="type-ui text-muted">{who}</span>,
              <Switch checked={on[t]} onCheckedChange={(v) => setOn((o) => ({ ...o, [t]: v }))} aria-label={`${t} enabled`} onClick={(e) => e.stopPropagation()} />,
              <IconButton variant="ghost" icon="more" aria-label={`More for ${t}`} onClick={(e) => e.stopPropagation()} />,
            ]}
          />
        ))}
      </Table>
    );
  },
};

/** API keys: an action column with a button. */
export const ActionColumn: Story = {
  render: () => (
    <Table
      label="API keys"
      columns={[{ label: "Name", width: 260 }, { label: "Created", width: 160 }, { label: "Scope", width: 140 }, { label: "", srLabel: "Actions", align: "end" }]}
    >
      {[
        ["ci-deploy", "Oct 2", "agents:read"],
        ["home-assistant", "Sep 14", "chat"],
      ].map(([n, d, s]) => (
        <TableRow
          key={n}
          endAligned={[3]}
          cells={[
            <span className="type-code-medium text-fg">{n}</span>,
            <span className="type-mono-xs text-muted">{d}</span>,
            <Badge>{s}</Badge>,
            <Button size="sm" variant="danger">Revoke</Button>,
          ]}
        />
      ))}
    </Table>
  ),
};

/** The CSV preview: dense mono cells. */
export const Dense: Story = {
  render: () => (
    <Table label="usage-october.csv" dense columns={[{ label: "date" }, { label: "agent" }, { label: "tokens", align: "end" }, { label: "cost", align: "end" }]}>
      {[
        ["2026-10-01", "Archivist", "41,220", "0.62"],
        ["2026-10-01", "Scout", "8,104", "0.11"],
        ["2026-10-02", "Archivist", "39,870", "0.58"],
        ["2026-10-02", "Treasurer", "2,311", "0.03"],
      ].map((r, i) => (
        <TableRow key={i} cells={r} endAligned={[2, 3]} />
      ))}
    </Table>
  ),
};

/** Columns with `onSort` become buttons that report their direction (the CSV preview). */
export const Sortable: Story = {
  render: function Render() {
    const [sort, setSort] = useState<{ col: number; dir: "asc" | "desc" } | null>({ col: 2, dir: "desc" });
    const cols = ["date", "agent", "tokens"];
    const rows = [
      ["2026-10-01", "Archivist", 41220],
      ["2026-10-01", "Scout", 8104],
      ["2026-10-02", "Treasurer", 2311],
    ];
    const sorted = [...rows].sort((a, b) => (sort ? (a[sort.col] > b[sort.col] ? 1 : -1) * (sort.dir === "asc" ? 1 : -1) : 0));
    return (
      <Table
        label="usage.csv"
        dense
        columns={cols.map((label, i) => ({
          label,
          align: i === 2 ? ("end" as const) : undefined,
          sorted: sort?.col === i ? sort.dir : undefined,
          onSort: () => setSort((s) => (s?.col === i ? (s.dir === "asc" ? { col: i, dir: "desc" } : null) : { col: i, dir: "asc" })),
        }))}
      >
        {sorted.map((r, i) => (
          <TableRow key={i} cells={r.map(String)} endAligned={[2]} />
        ))}
      </Table>
    );
  },
};
