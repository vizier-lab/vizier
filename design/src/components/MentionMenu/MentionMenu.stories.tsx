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

