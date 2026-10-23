import type { Meta, StoryObj } from "@storybook/react-vite";
import { DreamJournalEntry } from "./DreamJournalEntry";

const meta = {
  title: "Components/Agents & dreams/DreamJournalEntry",
  component: DreamJournalEntry,
  tags: ["approved"],
  parameters: { vizier: { id: "X56", replaces: "webui/app/routes/agent-dream.tsx" } },
  args: {
    when: "Tonight, 03:00 – 03:12",
    meta: "vector-db-review · #releases · Telegram DM",
    extraction: "6 conversations read",
    consolidation: "4 changes",
    changes: [
      { kind: "wrote", text: "infra/vector-dbs/summary.md — linked from qdrant and sqlite-vec" },
      { kind: "revised", text: "people/ganesha.md — prefers tables for comparisons" },
      { kind: "revised", text: "infra/lancedb.md — v0.19 filtered-search fix" },
      { kind: "core", text: "rev 41: delegate release-note checks to Scout" },
    ],
  },
  decorators: [(Story) => <div className="w-650">{Story()}</div>],
} satisfies Meta<typeof DreamJournalEntry>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Complete: Story = {};
