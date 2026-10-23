import type { Meta, StoryObj } from "@storybook/react-vite";
import { FilePeek } from "./FilePeek";

const rows = [
  ["db", "p50", "p99"],
  ["qdrant", "4.1", "19"],
  ["sqlite-vec", "6.9", "31"],
  ["pgvector", "5.3", "48"],
];

const meta = {
  title: "Components/Chat/FilePeek",
  component: FilePeek,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X38", replaces: null } },
  args: {
    name: "p99-results.csv",
    meta: "4 KB · 12 rows",
    action: { label: "Open", onClick: () => {} },
    preview: (
      <table className="w-full type-mono-sm text-soft">
        <tbody>
          {rows.map((r, i) => (
            <tr key={i} className={i === 0 ? "text-faint" : undefined}>
              {r.map((c, j) => (
                <td key={j} className="py-2">{c}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    ),
  },
  decorators: [(Story) => <div className="w-396">{Story()}</div>],
} satisfies Meta<typeof FilePeek>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A table: its first rows. */
export const Table: Story = {};

/** Markdown: its first lines, rendered. */
export const Markdown: Story = {
  args: {
    name: "release-notes.md",
    meta: "2 KB",
    preview: (
      <div className="flex flex-col gap-4">
        <p className="type-body-medium text-fg">vizier 0.13</p>
        <ul className="list-disc pl-16 type-small text-soft">
          <li>Skills you can run with /name</li>
          <li>Background jobs report back</li>
        </ul>
      </div>
    ),
  },
};

export const Pdf: Story = { args: { name: "lancedb-0.19-notes.pdf", meta: "310 KB · 6 pages", preview: undefined } };

/** A type the browser can't show. */
export const NoPreview: Story = { args: { name: "benchmarks.xlsx", meta: "28 KB · no preview for this type", preview: undefined, action: { label: "Download", onClick: () => {} } } };

export const Failed: Story = { args: { name: "flame-before.png", failed: true, preview: undefined, action: { label: "Retry", onClick: () => {} } } };
