import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { IntegrationLogo } from "../IntegrationLogo/IntegrationLogo";
import { Notice } from "../Notice/Notice";
import { Status, type StatusState } from "../Status/Status";
import { Switch } from "../Switch/Switch";

export type IntegrationCardProps = {
  kind: string;
  name: string;
  /** "Archivist#4821 · 3 servers", "Web and news search through the Brave API". */
  handle?: ReactNode;
  state: Extract<StatusState, "reading" | "failed" | "installed" | "disabled">;
  enabled: boolean;
  onEnabledChange: (enabled: boolean) => void;
  /** When failed: what happened and what it means. */
  failure?: { title: ReactNode; message: ReactNode };
  /** Settings: a KeyValueList, a SecretField, a ToolList. */
  children?: ReactNode;
  /** Buttons at the foot: Replace token, Delete, Unlink. */
  actions?: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** A channel or addon on its agent (X59): logo, name, state, settings, actions. */
export function IntegrationCard({ kind, name, handle, state, enabled, onEnabledChange, failure, children, actions, className, ...rest }: IntegrationCardProps) {
  return (
    <section
      aria-label={name}
      className={cx("flex flex-col gap-14 rounded-xl border bg-surface px-20 py-20", state === "failed" ? "border-failed-line" : "border-border", className)}
      {...rest}
    >
      <header className="flex items-center gap-14">
        <IntegrationLogo kind={kind} size={44} />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <h3 className="type-heading text-fg">{name}</h3>
          {handle && <p className="truncate type-mono-xs text-muted">{handle}</p>}
        </div>
        <Status state={state} font="sans" />
        <Switch checked={enabled} onCheckedChange={onEnabledChange} aria-label={`${name} enabled`} />
      </header>
      {failure && (
        <Notice tone="failed" marker="dot" title={failure.title}>
          {failure.message}
        </Notice>
      )}
      {children}
      {actions && <div className="flex flex-wrap gap-10">{actions}</div>}
    </section>
  );
}
