import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { FilterTabs } from "../FilterTabs/FilterTabs";
import { MemoryGraph, type GraphEdge, type GraphNode } from "./MemoryGraph";

const DOCS: GraphNode[] = [
  { id: "summary", label: "summary.md", x: 0.46, y: 0.56, weight: 6 },
  { id: "sqlite-vec", label: "sqlite-vec.md", x: 0.68, y: 0.3, weight: 4 },
  { id: "lancedb", label: "lancedb.md", x: 0.73, y: 0.56, weight: 2 },
  { id: "pgvector", label: "pgvector-notes.md", x: 0.22, y: 0.73, weight: 4 },
  { id: "storage", label: "storage-choice.md", x: 0.46, y: 0.18, weight: 2, faded: true },
  { id: "retrieval", label: "retrieval-eval.md", x: 0.71, y: 0.18, weight: 1, faded: true },
  { id: "bench", label: "bench-2026-10.md", x: 0.68, y: 0.79, weight: 3, faded: true },
  { id: "hnsw", label: "hnsw.md", x: 0.37, y: 0.91, weight: 2, faded: true },
  { id: "ops", label: "ops-runbook.md", x: 0.1, y: 0.9, weight: 1, faded: true },
  { id: "chunking", label: "chunking.md", x: 0.6, y: 0.97, weight: 1, faded: true },
  { id: "ganesha", label: "ganesha.md", x: 0.83, y: 0.94, weight: 1, faded: true },
];

const LINKS: GraphEdge[] = [
  { from: "summary", to: "sqlite-vec" },
  { from: "summary", to: "lancedb" },
  { from: "summary", to: "pgvector" },
  { from: "summary", to: "bench", dashed: true },
  { from: "sqlite-vec", to: "storage" },
  { from: "sqlite-vec", to: "retrieval" },
  { from: "sqlite-vec", to: "lancedb" },
  { from: "pgvector", to: "hnsw" },
  { from: "pgvector", to: "ops" },
  { from: "bench", to: "chunking" },
  { from: "bench", to: "ganesha", dashed: true },
  { from: "hnsw", to: "chunking" },
];

const meta = {
  title: "Components/Visualisation/MemoryGraph",
  component: MemoryGraph,
  tags: ["approved"],
  parameters: {
    vizier: { id: "X73", replaces: "webui/app/components/MemoryGraph.tsx" },
    docs: {
      description: {
        component:
          "Live, like the WebUI's graph: a force layout (d3-force). Drag a star to move it, drag the background to pan, scroll or use the buttons to zoom. Hover lights a star's links; click selects; double-click (or Enter) opens.",
      },
    },
  },
  args: {
    nodes: DOCS,
    edges: LINKS,
    selectedId: "summary",
    caption: "Star size = links · faded = 2 hops · dashed = another bundle · 11 of 42 shown",
  },
  decorators: [(Story) => <div className="h-640 w-900">{Story()}</div>],
} satisfies Meta<typeof MemoryGraph>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A document's neighbourhood. Click a star to select it. */
export const Neighbourhood: Story = {
  render: function Render(args) {
    const [sel, setSel] = useState<string | undefined>(args.selectedId);
    const [opened, setOpened] = useState<string | null>(null);
    return (
      <div className="flex h-full flex-col gap-8">
        <MemoryGraph {...args} selectedId={sel} onSelect={(id) => setSel(id ?? undefined)} onOpen={setOpened} className="min-h-0 flex-1" />
        <p className="type-mono-xs text-faint">{opened ? `Opened ${opened}` : "Double-click a star to open it."}</p>
      </div>
    );
  },
};

/** All bundles: the largest, with the ordering switch. */
export const Bundles: Story = {
  render: function Render() {
    const [order, setOrder] = useState("largest");
    const [sel, setSel] = useState<string | undefined>("infra");
    const nodes: GraphNode[] = [
      { id: "infra", label: "infra", x: 0.7, y: 0.2, weight: 42 },
      { id: "archive", label: "projects/2025-archive", x: 0.5, y: 0.55, weight: 1402 },
      { id: "journal", label: "journal", x: 0.38, y: 0.63, weight: 911 },
      { id: "people", label: "people", x: 0.46, y: 0.4, weight: 640 },
      { id: "meetings", label: "meetings/2026", x: 0.56, y: 0.7, weight: 486 },
      { id: "books", label: "books", x: 0.3, y: 0.5, weight: 388 },
      { id: "rag", label: "research/rag", x: 0.62, y: 0.43, weight: 296 },
      { id: "atlas", label: "project-atlas", x: 0.42, y: 0.82, weight: 212 },
      { id: "evals", label: "research/evals", x: 0.36, y: 0.33, weight: 174, faded: true },
      { id: "releases", label: "releases", x: 0.68, y: 0.66, weight: 133 },
      { id: "ops", label: "ops-runbooks", x: 0.2, y: 0.74, weight: 121 },
      { id: "q4", label: "projects/2026-q4", x: 0.2, y: 0.4, weight: 88 },
    ];
    const edges: GraphEdge[] = ["archive", "journal", "rag", "ops", "q4", "atlas", "releases"].map((id) => ({ from: "infra", to: id }));
    return (
      <MemoryGraph
        nodes={nodes}
        edges={[...edges, { from: "journal", to: "people" }, { from: "journal", to: "meetings" }, { from: "books", to: "evals" }]}
        selectedId={sel}
        onSelect={(id) => setSel(id ?? undefined)}
        className="h-full"
        controls={
          <FilterTabs
            aria-label="Show bundles by"
            value={order}
            onChange={setOrder}
            options={[
              { value: "largest", label: "Largest" },
              { value: "recent", label: "Recently changed" },
              { value: "linked", label: "Most linked" },
            ]}
          />
        }
        caption="Dot size = documents · line = links between bundles · 12 largest of 1,024 · double-click to open"
      />
    );
  },
};
