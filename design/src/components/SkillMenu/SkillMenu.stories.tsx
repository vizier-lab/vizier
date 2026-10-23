import type { Meta, StoryObj } from "@storybook/react-vite";
import { SkillMenu } from "./SkillMenu";

const SKILLS = [
  { name: "code-review", description: "Review a diff for bugs, then for clarity" },
  { name: "compare-tables", description: "Turn benchmark output into a table" },
  { name: "checkpoint", description: "Save checkpoint with handover" },
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
