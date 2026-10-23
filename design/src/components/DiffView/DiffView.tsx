import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Button } from "../Button/Button";

export type DiffLine = { kind: "add" | "del" | "ctx"; text: string };

export type DiffViewProps = {
  /** "rev 40 → 41". */
  title: ReactNode;
  lines: DiffLine[];
  /** "Restore rev 40". */
  restoreLabel?: ReactNode;
  onRestore?: () => void;
} & HTMLAttributes<HTMLDivElement>;

/** What a revision changed (X52): added, removed and context lines, and Restore. */
export function DiffView({ title, lines, restoreLabel, onRestore, className, ...rest }: DiffViewProps) {
  return (
    <div className={cx("flex flex-col gap-10", className)} {...rest}>
      <p className="type-mono-xs text-faint">{title}</p>
      <ul className="flex flex-col gap-4">
        {lines.map((l, i) => (
          <li
            key={i}
            className={cx(
              "grid grid-cols-[14px_1fr] gap-6 rounded-sm px-6 py-4 type-ui-para",
              l.kind === "add" && "bg-raised text-fg",
              l.kind === "del" && "text-faint [&>span:last-child]:line-through",
              l.kind === "ctx" && "text-muted",
            )}
          >
            {/* Code-block type shares the 20px line, so the marker sits on the text's first line at full size. */}
            <span aria-hidden className={cx("type-code-block no-underline", l.kind === "add" ? "text-live" : "text-muted")}>
              {l.kind === "add" ? "+" : l.kind === "del" ? "-" : ""}
            </span>
            <span>
              <span className="sr-only">{l.kind === "add" ? "Added: " : l.kind === "del" ? "Removed: " : ""}</span>
              {l.text}
            </span>
          </li>
        ))}
      </ul>
      {onRestore && (
        <div>
          <Button icon="restore" onClick={onRestore}>
            {restoreLabel ?? "Restore"}
          </Button>
        </div>
      )}
    </div>
  );
}
