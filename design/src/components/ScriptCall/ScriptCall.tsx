import { useState, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type ScriptCallProps = {
  /** "memory_search". */
  name: string;
  /** A short form of the arguments: "vector db benchmark", "infra/qdrant.md, +3". */
  args?: ReactNode;
  duration?: ReactNode;
  state?: "ok" | "failed";
  /** Several identical calls grouped: shows ×N. */
  repeat?: number;
  /** The full arguments, shown when the row is opened (run detail). */
  details?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** A call made from inside a python run, nested under it (D52). */
export function ScriptCall({ name, args, duration, state = "ok", repeat, details, className, ...rest }: ScriptCallProps) {
  const [open, setOpen] = useState(false);
  const failed = state === "failed";
  const row = (
    <>
      <Icon name={failed ? "close" : "check"} size={12} tone={failed ? "failed" : "live"} title={failed ? "failed" : "ok"} />
      <span className={cx("type-mono-sm", failed ? "text-failed" : "text-soft")}>
        {name}
        {repeat && repeat > 1 && <span className="text-faint"> ×{repeat}</span>}
      </span>
      {args && <span className="min-w-0 flex-1 truncate type-small text-faint">{args}</span>}
      {duration && <span className={cx("ml-auto shrink-0 type-mono-xs", failed ? "text-failed" : "text-faint")}>{duration}</span>}
    </>
  );
  return (
    <div className={cx("flex flex-col gap-4", className)} {...rest}>
      {details ? (
        <button type="button" aria-expanded={open} onClick={() => setOpen(!open)} className="flex min-h-22 w-full items-center gap-8 rounded-xs text-left">
          <Icon name={open ? "chevron-down" : "chevron-right"} size={10} tone="faint" />
          {row}
        </button>
      ) : (
        <div className="flex min-h-22 items-center gap-8">{row}</div>
      )}
      {details && open && <div className="ml-20 rounded-md bg-ground px-12 py-10 type-code-block text-soft">{details}</div>}
    </div>
  );
}
