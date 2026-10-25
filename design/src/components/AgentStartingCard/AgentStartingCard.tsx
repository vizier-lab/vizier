import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";

export type AgentStartingCardProps = {
  name: string;
  /** "Pulling qwen3:32b from Ollama", "Building tools". */
  step: ReactNode;
  /** 0–100 while pulling; omit while building. */
  percent?: number;
  /** "12.4 GB of 20.0 GB". */
  detail?: ReactNode;
  /** "about 3 min". */
  eta?: ReactNode;
  /** Per-artifact lines under the bar (kind · label · status), from the setup poll. */
  downloads?: ReactNode;
  onDismiss?: () => void;
} & HTMLAttributes<HTMLDivElement>;

/** An agent starting up (X58): what it's doing, and how far along. */
export function AgentStartingCard({ name, step, percent, detail, eta, downloads, onDismiss, className, ...rest }: AgentStartingCardProps) {
  return (
    <div role="status" className={cx("flex w-full max-w-324 flex-col gap-8 rounded-lg border border-border-strong bg-surface px-16 py-14 shadow-popover", className)} {...rest}>
      <div className="flex items-center gap-8">
        <span aria-hidden className="flex gap-4">
          {["bg-muted", "bg-muted", "bg-live"].map((c, i) => (
            <span key={i} className={cx("size-5 rounded-pill animate-dot-wave motion-reduce:animate-none", c)} style={{ animationDelay: `${i * 0.2}s` }} />
          ))}
        </span>
        <span className="min-w-0 flex-1 truncate type-ui-strong text-fg">Starting {name}…</span>
        {onDismiss && <IconButton variant="ghost" size="sm" icon="close" aria-label="Hide" onClick={onDismiss} />}
      </div>
      <p className="type-small text-soft">{step}</p>
      <div role="progressbar" aria-label={`Starting ${name}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} className="relative h-4 overflow-hidden rounded-pill bg-border-strong">
        <div
          className={cx("absolute inset-y-0 left-0 rounded-pill bg-live", percent === undefined && "w-1/3 animate-indeterminate motion-reduce:animate-none")}
          style={percent === undefined ? undefined : { width: `${percent}%` }}
        />
      </div>
      <div className="flex items-center justify-between type-mono-xs text-faint">
        <span>
          {percent !== undefined && `${percent}%`}
          {percent !== undefined && detail && " · "}
          {detail}
        </span>
        {eta && <span>{eta}</span>}
      </div>
      {downloads && <div className="flex flex-col gap-3 border-t border-border pt-8">{downloads}</div>}
    </div>
  );
}
