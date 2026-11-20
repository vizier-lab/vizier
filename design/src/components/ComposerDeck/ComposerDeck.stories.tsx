import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type ReactNode } from "react";
import { Composer } from "../Composer/Composer";
import { StatusPanel } from "../StatusStrip/StatusPanel";
import type { StatusStripItem, StatusStripItemId } from "../StatusStrip/StatusStrip";
import { ComposerDeck, type ComposerDeckAside } from "./ComposerDeck";

const items: StatusStripItem[] = [
  { id: "work", icon: "briefcase", label: "background work running", count: 2, busy: true },
  { id: "subagents", icon: "agent", label: "subagents at work", count: 0 },
  { id: "files", icon: "attach", label: "session files", count: 3 },
];

const projectItems: StatusStripItem[] = [
  ...items,
  { id: "changes", icon: "git-branch", label: "changed files", count: 4 },
  { id: "lsp", icon: "code", label: "language servers ready", count: 2, dot: "live" },
];

const rows: Record<string, string[]> = {
  work: ["Draft the incident follow-up · running · 0:42", "Re-index ops runbooks · running · 2:10", "Summarise webhook storm logs · done"],
  subagents: [],
  files: ["incident-report.pdf", "timeline.md", "storm-graph.png"],
  lsp: ["rust-analyzer · ready", "typescript-language-server · ready"],
};

function Sheet({ id, onClose }: { id: StatusStripItemId; onClose: () => void }) {
  const list = rows[id] ?? [];
  return (
    <StatusPanel title={id} meta={id === "work" ? "2 running" : `${list.length}`} onClose={onClose}>
      {list.length === 0 ? (
        <p className="px-8 py-12 type-small text-faint">Nothing here yet.</p>
      ) : (
        <ul className="flex flex-col">
          {list.map((r) => (
            <li key={r} className="truncate rounded-md px-8 py-8 type-ui text-fg hover:bg-raised">
              {r}
            </li>
          ))}
        </ul>
      )}
    </StatusPanel>
  );
}

function Stage({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-640 flex-col justify-end bg-panel p-24">
      <div className="mx-auto w-full max-w-[920px]">{children}</div>
    </div>
  );
}

function Demo({
  initial = null,
  project,
  aside: withAside,
}: {
  initial?: StatusStripItemId | null;
  project?: boolean;
  aside?: boolean;
}) {
  const [open, setOpen] = useState<StatusStripItemId | null>(initial);
  const [value, setValue] = useState("");
  const [asideOpen, setAsideOpen] = useState(!!withAside);
  const aside: ComposerDeckAside | null = asideOpen
    ? {
        label: "/btw",
        onClose: () => setAsideOpen(false),
        content: (
          <div className="flex flex-col gap-12 px-16 py-14">
            <p className="self-end rounded-lg bg-raised px-12 py-8 type-ui text-fg">Which of these runbooks is the oldest?</p>
            <p className="type-body text-fg">
              ops/escalation-policy — last edited 14 months ago. The on-call rotation note replaced most of it in March, so it's the likeliest to be stale.
            </p>
          </div>
        ),
      }
    : null;
  return (
    <Stage>
      <ComposerDeck
        items={project ? projectItems : items}
        open={open}
        onOpenChange={setOpen}
        aside={aside}
        sheet={(item, close) => <Sheet id={item.id} onClose={close} />}
        trailing={<span className="truncate px-6 type-mono-xs text-muted">openrouter/deepseek/deepseek-v4.1-flash ⌄</span>}
      >
        <Composer agentName="talos" value={value} onValueChange={setValue} onSend={() => setValue("")} onAttach={() => {}} lifted context={{ used: 21000, window: 200000 }} />
      </ComposerDeck>
    </Stage>
  );
}

const meta = {
  title: "Components/Chat/ComposerDeck",
  component: ComposerDeck,
  tags: ["approved"],
  parameters: { vizier: { id: "D62", replaces: "StatusStrip under the composer" } },
} satisfies Meta<typeof ComposerDeck>;

export default meta;
type Story = StoryObj<typeof meta>;

const args = { items, open: null, onOpenChange: () => {}, children: null };

/** Closed: only the back panel's lip shows above the composer, carrying the strip. */
export const Closed: Story = { args, render: () => <Demo /> };

/** An item open: the back panel grows upward into its sheet and the strip becomes the header. */
export const Open: Story = { args, render: () => <Demo initial="work" /> };

/** A project chat: Changes and language servers join the strip. */
export const Project: Story = { args, render: () => <Demo project /> };

/** A language-server sheet in a project chat. */
export const ProjectOpen: Story = { args, render: () => <Demo project initial="lsp" /> };

/** `/btw`: the aside takes the pressed slot and the sheet. Pressing a strip item closes it. */
export const SideChat: Story = { args, render: () => <Demo aside /> };
