import type { Meta, StoryObj } from "@storybook/react-vite";
import { DocumentDetail, SelectedBundle } from "./DocumentDetail";

const meta = {
  title: "Components/Visualisation/DocumentDetail",
  component: DocumentDetail,
  tags: ["approved"],
  parameters: { vizier: { id: "X74", replaces: "webui/app/routes/memory.tsx" } },
  args: {
    title: "Vector databases — summary",
    path: "infra / vector-dbs/summary.md · 3 passages",
    tags: ["vector-db", "benchmark", "infra"],
    summary: "All three handled the 2.1M-passage corpus; the difference is tail latency and growth. Qdrant filters best; sqlite-vec needs no service; pgvector's HNSW rebuild is slow.",
    linksTo: [
      { label: "qdrant.md", href: "#qdrant" },
      { label: "sqlite-vec.md", href: "#sqlite-vec" },
      { label: "pgvector-notes.md", href: "#pgvector" },
      { label: "lancedb.md", href: "#lancedb" },
    ],
    linkedFrom: [
      { label: "bench-2026-10.md", href: "#bench", note: "journal" },
      { label: "storage-choice.md", href: "#storage" },
    ],
    provenance: "Created by Archivist · today 14:03 (chat vector-db-review) · rev 3",
    onEdit: () => {},
    onHistory: () => {},
    onClose: () => {},
  },
  decorators: [(Story) => <div className="w-320">{Story()}</div>],
} satisfies Meta<typeof DocumentDetail>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Document: Story = {};

/** The all-bundles view: the selected bundle. */
export const Bundle: Story = {
  render: () => (
    <SelectedBundle
      name="infra"
      meta="42 documents · 12 links to 6 bundles · last written today by Archivist"
      onOpen={() => {}}
      onExport={() => {}}
      onDelete={() => {}}
    />
  ),
};
