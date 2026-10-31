import type { Meta, StoryObj } from "@storybook/react-vite";
import { SkillMenu } from "./SkillMenu";

const SKILLS = [
  { name: "code-review", description: "Review a diff for bugs, then for clarity" },
  { name: "compare-tables", description: "Turn benchmark output into a table" },
  { name: "checkpoint", description: "Save checkpoint with handover" },
];

// The names the menu really holds: long slugs and the paragraphs skills actually write.
const LONG = [
  {
    name: "speckit-taskstoissues",
    description: "Convert existing tasks into actionable, dependency-ordered GitHub issues for the feature",
  },
  {
    name: "deep-repo-orientation",
    description:
      "Take over maintenance of an unfamiliar repository: read docs of record, tree, CI, tests and release machinery, then persist a linked memory bundle",
  },
  { name: "impeccable", description: "Design, critique, audit and polish a frontend interface" },
  { name: "btw", description: "Ask a side question that is never saved" },
];

const MANY = [
  ...SKILLS,
  ...LONG,
  { name: "access-review", description: "Check who can reach what before a release" },
  { name: "agent-form", description: "Shape an agent's config: provider, tools, limits" },
  { name: "api-surface", description: "List the HTTP routes an agent exposes" },
  { name: "assess", description: "Read a change and say whether it is done" },
  { name: "batch-tests", description: "Run the suites in the order they fail fastest" },
  { name: "catalog-tables", description: "Turn a component inventory into a table" },
  { name: "checkpoint", description: "Save checkpoint with handover" },
  { name: "clarify", description: "Ask up to five questions the spec leaves open" },
  { name: "commit-split", description: "Split a working tree into reviewable commits" },
  { name: "dependency-audit", description: "Diff the lockfiles before anything ships" },
  { name: "diff-explain", description: "Explain a diff to someone who did not write it" },
  { name: "docs-drift", description: "Find docs that no longer match the tree" },
  { name: "flake-hunt", description: "Run a failing test until it lies twice" },
  { name: "log-triage", description: "Read a trace and name the first wrong step" },
];

const meta = {
  title: "Components/Chat/SkillMenu",
  component: SkillMenu,
  tags: ["approved"],
  parameters: { vizier: { id: "X33", replaces: null } },
  args: { id: "skills", items: SKILLS, query: "co", active: 0, onPick: () => {} },
  decorators: [(Story) => <div className="w-396">{Story()}</div>],
} satisfies Meta<typeof SkillMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Filtering as "/co" is typed. */
export const Filtering: Story = {};

export const NoMatch: Story = { args: { items: [], query: "deploy" } };

/** Long slugs stay on one line and every description starts at the same x. */
export const LongNames: Story = { args: { items: LONG, query: "", active: 0 } };

/** More rows than fit: the list caps and scrolls, the heading and the key hints stay put. */
export const ManyItems: Story = { args: { items: MANY, query: "", active: 3 } };
