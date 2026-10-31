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
