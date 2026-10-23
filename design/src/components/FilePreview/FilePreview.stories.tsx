import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Segment } from "../Segment/Segment";
import { Table } from "../Table/Table";
import { TableRow } from "../TableRow/TableRow";
import { FilePreview } from "./FilePreview";

const DATA = [
  ["qdrant", "4.1", "19", "6.8 GB", "best filtering"],
  ["sqlite-vec", "6.9", "31", "2.2 GB", "embedded, no service"],
  ["pgvector", "5.3", "48", "7.4 GB", "HNSW rebuild is slow"],
  ["lancedb", "5.0", "27", "3.1 GB", "filtered search fixed in 0.19"],
];

const body = (
  <Table label="p99-results.csv" dense columns={[{ label: "db" }, { label: "p50 ms" }, { label: "p99 ms" }, { label: "RAM @2.1M" }, { label: "notes" }]}>
    {DATA.map((r) => (
      <TableRow key={r[0]} cells={r} />
    ))}
  </Table>
);

const meta = {
  title: "Components/Chat/FilePreview",
  component: FilePreview,
  tags: ["approved"],
  parameters: { vizier: { id: "X37", replaces: "webui/app/components/AttachmentPreviewModal.tsx" } },
  args: {
    name: "p99-results.csv",
    meta: "4 KB · 12 rows · sent by Archivist at 14:02 in vector-db-review",
    onDownload: () => {},
    onOpenInTab: () => {},
    onClose: () => {},
    onPrevious: () => {},
    onNext: () => {},
    files: [
      { name: "flame-before.png", onSelect: () => {} },
      { name: "p99-results.csv", current: true, onSelect: () => {} },
      { name: "release-notes.md", onSelect: () => {} },
    ],
    children: body,
  },
  decorators: [(Story) => <div className="w-860">{Story()}</div>],
} satisfies Meta<typeof FilePreview>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Over the chat, with the Table/Raw switch. */
export const Dialog: Story = {
  render: function Render(args) {
    const [view, setView] = useState<"table" | "raw">("table");
    return (
      <FilePreview
        {...args}
        viewBar={
          <>
            <span role="group" aria-label="View" className="flex gap-4">
              <Segment selected={view === "table"} onClick={() => setView("table")}>Table</Segment>
              <Segment selected={view === "raw"} onClick={() => setView("raw")}>Raw</Segment>
            </span>
            <span className="type-small text-faint">Sorted by p99 · click a column to sort</span>
          </>
        }
      >
        {view === "table" ? body : <pre className="type-code-block text-soft">{["db,p50_ms,p99_ms,ram,notes", ...DATA.map((r) => r.join(","))].join("\n")}</pre>}
      </FilePreview>
    );
  },
};

/** In the conversation: compact. */
export const Inline: Story = {
  args: { look: "inline", meta: "4 KB · from Archivist · 14:02" },
  decorators: [(Story) => <div className="w-400">{Story()}</div>],
};
