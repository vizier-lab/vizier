import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import type { IconName } from "../../icons/map";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";
import { Segment } from "../Segment/Segment";

export type EditorCommand = "bold" | "italic" | "code" | "link" | "bullets" | "numbers" | "outline" | "table" | "insert";
export type EditorMode = "rich" | "markdown" | "split";

export type EditorToolbarProps = {
  /** Compact (Core): block type, B I list code link, a count. Full (skill editor): every tool, modes, full screen. */
  look?: "compact" | "full";
  /** "Paragraph", "Heading 2": the caller's block-type control, or its label. */
  blockType: ReactNode;
  onCommand: (command: EditorCommand) => void;
  /** Commands active at the cursor (bold inside bold text). */
  active?: EditorCommand[];
  /** Compact: "1,284 chars". */
  count?: ReactNode;
  mode?: EditorMode;
  onModeChange?: (mode: EditorMode) => void;
  onFullscreen?: () => void;
} & HTMLAttributes<HTMLDivElement>;

const TOOLS: { cmd: EditorCommand; icon: IconName; label: string; full?: boolean }[] = [
  { cmd: "bold", icon: "bold", label: "Bold" },
  { cmd: "italic", icon: "italic", label: "Italic" },
  { cmd: "bullets", icon: "list", label: "Bulleted list" },
  { cmd: "code", icon: "code", label: "Code" },
  { cmd: "link", icon: "link", label: "Link" },
  { cmd: "numbers", icon: "list-ordered", label: "Numbered list", full: true },
  { cmd: "outline", icon: "list-tree", label: "Outline", full: true },
  { cmd: "table", icon: "table", label: "Table", full: true },
];

/** The markdown editor's toolbar (X50). */
export function EditorToolbar({ look = "compact", blockType, onCommand, active = [], count, mode = "rich", onModeChange, onFullscreen, className, ...rest }: EditorToolbarProps) {
  const full = look === "full";
  const tools = TOOLS.filter((t) => full || !t.full);
  return (
    <div role="toolbar" aria-label="Formatting" className={cx("flex items-center gap-4 border-b border-border px-12 py-8", className)} {...rest}>
      <span className="pr-8 type-ui text-soft">{blockType}</span>
      {tools.map((t) => (
        <IconButton
          key={t.cmd}
          variant="ghost"
          size="sm"
          icon={t.icon}
          aria-label={t.label}
          selected={active.includes(t.cmd)}
          onClick={() => onCommand(t.cmd)}
        />
      ))}
      {full && (
        <button type="button" onClick={() => onCommand("insert")} className="ml-8 inline-flex items-center gap-6 rounded-sm px-6 py-4 type-ui text-muted hover:bg-raised hover:text-fg">
          <span className="inline-flex size-16 items-center justify-center rounded-xs border border-border-strong">
            <Icon name="slash" size={11} />
          </span>
          Insert block
        </button>
      )}
      <span className="flex-1" />
      {count && <span className="type-mono-xs text-faint">{count}</span>}
      {full && onModeChange && (
        <div role="group" aria-label="Editor mode" className="inline-flex rounded-md border border-border-strong bg-surface p-2">
          {(["rich", "markdown", "split"] as const).map((m) => (
            <Segment key={m} selected={mode === m} onClick={() => onModeChange(m)}>
              {{ rich: "Rich", markdown: "Markdown", split: "Split" }[m]}
            </Segment>
          ))}
        </div>
      )}
      {full && onFullscreen && <IconButton variant="ghost" size="sm" icon="maximize" aria-label="Full screen" onClick={onFullscreen} />}
    </div>
  );
}
