import type { Meta, StoryObj } from "@storybook/react-vite";
import { MentionMenu } from "./MentionMenu";

const FILES = [
  { path: "crates/vizier/src/cli/agent.rs", kind: "file" as const },
  { path: "crates/vizier-core/src/agents/mod.rs", kind: "file" as const },
  { path: "crates/vizier-core/src/agents/", kind: "dir" as const },
  { path: "docs/src/agents.md", kind: "file" as const },
];

const meta = {
  title: "Components/Chat/MentionMenu",
  component: MentionMenu,
  tags: ["needs-review"],
  parameters: { vizier: { id: "X88", replaces: null } },
  args: { id: "mentions", label: "Files", items: FILES, query: "agent", active: 0, onPick: () => {} },
  decorators: [(Story) => <div className="w-480">{Story()}</div>],
} satisfies Meta<typeof MentionMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Filtering as "@agent" is typed in a project chat. */
export const Filtering: Story = {};

/** A folder row highlighted. */
export const Folder: Story = { args: { active: 2 } };

export const NoMatch: Story = { args: { items: [], query: "zzz" } };

const MEMBERS = [
  { kind: "agent" as const, id: "archivist", name: "Archivist" },
  { kind: "agent" as const, id: "scribe", name: "Scribe", hint: "restarting" },
  { kind: "person" as const, id: "maya", name: "Maya" },
  { kind: "person" as const, id: "rafi", name: "Rafi" },
];

/** In a Workspace the same menu lists agents (their sigil) and people (their face). */
export const AgentsAndPeople: Story = {
  args: { label: "Agents and people", items: MEMBERS, query: "", active: 1 },
};

export const NoMemberMatches: Story = {
  args: { label: "Agents and people", items: [], query: "zed", emptyText: "Nobody in this Workspace is called “@zed”." },
};

const ENTITIES = [
  { kind: "artifact" as const, name: "report", note: "artifact" },
  { kind: "function" as const, name: "fetch_weather", note: "function" },
  { kind: "skill" as const, name: "summarize", note: "skill · agent" },
  { kind: "skill" as const, name: "onboarding", note: "skill · global" },
  { kind: "task" as const, name: "weekly-report", note: "task" },
  { kind: "memory" as const, name: "research/topics/energy", note: "memory" },
];

/** Typed "[[" in the composer: the chat agent's artifacts, functions, skills, tasks and memory (specs/066). */
export const EntityLinks: Story = {
  args: {
    label: "Links",
    items: ENTITIES,
    query: "",
    active: 0,
    footer: "↑↓ to move · Tab or Enter to insert · Esc to close",
  },
};

export const NoEntityMatches: Story = {
  args: {
    label: "Links",
    items: [],
    query: "zzz",
    emptyText: "Nothing matches “[[zzz”. Enter sends it as text.",
    footer: "↑↓ to move · Tab or Enter to insert · Esc to close",
  },
};

