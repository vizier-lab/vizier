import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Button } from "../Button/Button";

/** `hunk` is an `@@` header: its text is the range and context, and it carries no line numbers. */
export type DiffLine = { kind: "add" | "del" | "ctx" | "hunk"; text: string; oldNo?: number; newNo?: number };

export type DiffViewProps = {
  /** "rev 40 → 41". */
  title?: ReactNode;
  lines: DiffLine[];
  /** Prose: a revision's text changes. Code: a source diff with line numbers and tinted rows. */
  variant?: "prose" | "code";
  /** "Restore rev 40". */
  restoreLabel?: ReactNode;
  onRestore?: () => void;
} & HTMLAttributes<HTMLDivElement>;

const MARK = { add: "+", del: "−", ctx: "", hunk: "" } as const;
const SR = { add: "Added: ", del: "Removed: ", ctx: "", hunk: "" } as const;

/** What a revision or a file changed (X52): added, removed and context lines, and Restore. */
export function DiffView({ title, lines, variant = "prose", restoreLabel, onRestore, className, ...rest }: DiffViewProps) {
  const restore = onRestore && (
    <div>
      <Button icon="restore" onClick={onRestore}>
        {restoreLabel ?? "Restore"}
      </Button>
    </div>
  );

  if (variant === "code") {
    return (
      <div className={cx("flex flex-col gap-10", className)} {...rest}>
        {title && <p className="type-mono-xs text-faint">{title}</p>}
        <ul className="overflow-hidden rounded-md border border-border bg-ground">
          {lines.map((l, i) =>
            l.kind === "hunk" ? (
              <li key={i} className="bg-raised px-12 py-4 type-mono-xs text-faint">
                {l.text}
              </li>
            ) : (
              <li
                key={i}
                className={cx(
                  "grid grid-cols-[40px_40px_16px_1fr] gap-x-4 px-8 type-code-block",
                  l.kind === "add" && "bg-live-bg text-live-text",
                  l.kind === "del" && "bg-failed-bg text-failed-text",
                  l.kind === "ctx" && "text-muted",
                )}
              >
                {/* Gutter numbers stay faint on tinted rows; only the number of the side that exists is drawn. */}
                <span aria-hidden className="select-none text-right text-faint">
                  {l.oldNo}
                </span>
                <span aria-hidden className="select-none text-right text-faint">
                  {l.newNo}
                </span>
                <span aria-hidden className="select-none text-center">
                  {MARK[l.kind]}
                </span>
                <span className="min-w-0 whitespace-pre-wrap break-words">
                  <span className="sr-only">{SR[l.kind]}</span>
                  {l.text}
                </span>
              </li>
            ),
          )}
        </ul>
        {restore}
      </div>
    );
  }

  return (
    <div className={cx("flex flex-col gap-10", className)} {...rest}>
      {title && <p className="type-mono-xs text-faint">{title}</p>}
      <ul className="flex flex-col gap-4">
        {lines
          .filter((l) => l.kind !== "hunk")
          .map((l, i) => (
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
              <span aria-hidden className={cx("type-code-block no-underline", l.kind === "add" ? "text-live-text" : "text-muted")}>
                {l.kind === "add" ? "+" : l.kind === "del" ? "-" : ""}
              </span>
              <span>
                <span className="sr-only">{SR[l.kind]}</span>
                {l.text}
              </span>
            </li>
          ))}
      </ul>
      {restore}
    </div>
  );
}
