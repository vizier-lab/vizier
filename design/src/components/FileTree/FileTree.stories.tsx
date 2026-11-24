import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { FileTree, type FileTreeNode } from "./FileTree";

const NODES: FileTreeNode[] = [
  {
    id: "people",
    label: "people",
    kind: "folder",
    icon: "memory",
    mono: true,
    meta: "4",
    children: [
      { id: "people/ana", label: "ana.md", kind: "file", mono: true },
      { id: "people/bram", label: "bram.md", kind: "file", mono: true },
      {
        id: "people/team",
        label: "team",
        kind: "folder",
        mono: true,
        children: [
          { id: "people/team/ops", label: "ops.md", kind: "file", mono: true },
          { id: "people/team/design", label: "design.md", kind: "file", mono: true },
        ],
      },
    ],
  },
  { id: "projects", label: "projects", kind: "folder", icon: "memory", mono: true, meta: "112" },
  {
    id: "recipes",
    label: "recipes",
    kind: "folder",
    icon: "memory",
    mono: true,
    meta: "61",
    children: [
      { id: "recipes/bread", label: "bread.md", kind: "file", mono: true },
      { id: "recipes/more", label: "Load 60 more", kind: "action" },
    ],
  },
];

const meta = {
  title: "Components/Lists/FileTree",
  component: FileTree,
  tags: ["approved"],
  parameters: { vizier: { id: "X82", replaces: null } },
  args: { nodes: NODES, label: "Memory", expanded: new Set(["people", "people/team"]), onToggle: () => {}, onSelect: () => {}, selectedId: "people/bram" },
  decorators: [(Story) => <div className="w-280">{Story()}</div>],
} satisfies Meta<typeof FileTree>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: function Render(args) {
    const [open, setOpen] = useState<Set<string>>(new Set(args.expanded));
    const [sel, setSel] = useState(args.selectedId);
    return (
      <FileTree
        {...args}
        expanded={open}
        selectedId={sel}
        onToggle={(id, o) => setOpen((s) => {
          const n = new Set(s);
          if (o) n.add(id);
          else n.delete(id);
          return n;
        })}
        onSelect={(n) => n.kind === "file" && setSel(n.id)}
      />
    );
  },
};

/** "projects" is open and its documents have not arrived. */
export const Loading: Story = { args: { expanded: new Set(["projects"]), selectedId: undefined } };

export const Empty: Story = { args: { nodes: [], empty: <p className="type-small text-faint">Nothing remembered yet.</p> } };
