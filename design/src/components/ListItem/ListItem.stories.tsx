import type { Meta, StoryObj } from "@storybook/react-vite";
import { Sigil } from "../../brand/Sigil/Sigil";
import { Icon } from "../../icons/Icon";
import { Avatar } from "../Avatar/Avatar";
import { Badge } from "../Badge/Badge";
import { IconButton } from "../IconButton/IconButton";
import { ProviderLogo } from "../ProviderLogo/ProviderLogo";
import { Status } from "../Status/Status";
import { StateGrid, interactionCells, statePseudo } from "../../stories/StateGrid";
import { ListItem } from "./ListItem";

const meta = {
  title: "Components/Lists/ListItem",
  component: ListItem,
  tags: ["needs-review"],
  parameters: {
    vizier: { id: "X24", replaces: null, merges: ["X25", "X26", "X27", "X28", "X29", "X30", "X64", "X67"] },
    docs: {
      description: {
        component:
          "The one row shape behind every list: leading visual, title (sans or mono), subtitle, right-aligned meta, trailing controls. File, activity, revision, run, memory, agent, provider and role rows are built on it, and so are picker rows. With `href` it is a link, with `onSelect` a button; otherwise static.",
      },
    },
  },
  args: {
    leading: <Sigil agentId="Archivist" size="md" />,
    title: "Archivist",
    subtitle: <span className="type-mono-xs text-live">live · replying</span>,
    trailing: <Badge tone="rank">Owner</Badge>,
    onSelect: () => {},
  },
  decorators: [(Story) => <div className="w-340">{Story()}</div>],
} satisfies Meta<typeof ListItem>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

/** The row shapes it replaces, side by side. */
export const Shapes: Story = {
  decorators: [(Story) => <div className="flex w-420 flex-col gap-4">{Story()}</div>],
  render: () => (
    <>
      <ListItem leading={<Icon name="file" tone="muted" />} title="usage-october.csv" titleMono meta="12 KB" trailing={<IconButton variant="ghost" size="sm" icon="download" aria-label="Download usage-october.csv" />} />
      <ListItem leading={<Avatar name="ganesha" size="sm" />} title="ganesha edited CORE" subtitle="Added the review checklist" meta="2h" />
      <ListItem leading={<Icon name="history" tone="muted" />} title="Revision 14" subtitle="by Archivist · dream" meta="Oct 22" href="#" />
      <ListItem leading={<Status state="answered" label="" />} title="Mon Oct 20, 09:00" subtitle="Answered in 14s" meta="412 tokens" href="#" />
      <ListItem leading={<Icon name="memory" tone="muted" />} title="qdrant.md#3" titleMono subtitle="infra" meta="0.82" />
      <ListItem leading={<ProviderLogo provider="ollama" name="Ollama" />} title="Ollama" subtitle={<span className="type-mono-sm text-faint">http://localhost:11434 · no key</span>} meta="2 agents" />
      <ListItem title="maintainer" titleMono subtitle="Held by 3 people" trailing={<Icon name="chevron-right" tone="faint" size={14} />} onSelect={() => {}} />
    </>
  ),
};

export const States: Story = {
  tags: ["!autodocs"],
  parameters: statePseudo,
  decorators: [(Story) => <div>{Story()}</div>],
  render: () => {
    const r = (props: Partial<React.ComponentProps<typeof ListItem>> = {}) => (
      <div className="w-300">
        <ListItem leading={<Sigil agentId="Scout" size="sm" />} title="Scout" subtitle="idle · 2h ago" onSelect={() => {}} {...props} />
      </div>
    );
    return (
      <StateGrid
        rows={[
          { cells: interactionCells(() => r()).slice(0, 2) },
          { cells: [...interactionCells(() => r()).slice(2), { label: "Selected", node: r({ selected: true }) }, { label: "Disabled", node: r({ disabled: true }) }] },
        ]}
      />
    );
  },
};

/** Long mono names are cut in the middle (the full name shows on hover); long text truncates at the end. */
export const LongContent: Story = {
  args: {
    title: "projects/2025-archive/board-meetings/quarterly/q4-board-notes.md",
    titleMono: true,
    subtitle: "Review a diff for bugs, then for clarity. Point at lines, never rewrite whole files.",
    meta: "v1.2.0",
    leading: undefined,
    trailing: undefined,
  },
};
