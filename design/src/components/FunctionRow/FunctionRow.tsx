import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Badge } from "../Badge/Badge";
import { IconButton } from "../IconButton/IconButton";
import { ListItem } from "../ListItem/ListItem";
import { Switch } from "../Switch/Switch";

export type FunctionRowProps = {
  /** What a script calls: `custom_weekly_digest`. */
  callAs: string;
  description?: ReactNode;
  version: number;
  /** "a dream · 2 h ago", "you · yesterday". */
  meta?: ReactNode;
  enabled: boolean;
  /** Without it the switch is not shown: a person who can only view the agent. */
  onEnabledChange?: (enabled: boolean) => void;
  /** Its result comes from a model somewhere down its calls. */
  modelBacked?: boolean;
  /** The functions it calls that the agent no longer has. */
  missing?: string[];
  /** "switched off by alice". */
  offNote?: ReactNode;
  href?: string;
  onSelect?: () => void;
} & Omit<HTMLAttributes<HTMLElement>, "title" | "onSelect">;

/**
 * One function on an agent's Functions page (spec 061): its call name and description,
 * version, model backing, needs-attention, author and time, and its switch.
 */
export function FunctionRow({
  callAs,
  description,
  version,
  meta,
  enabled,
  onEnabledChange,
  modelBacked,
  missing,
  offNote,
  href,
  onSelect,
  className,
  ...rest
}: FunctionRowProps) {
  const attention = missing && missing.length > 0;
  return (
    <ListItem
      shape="flush"
      align="start"
      titleMono
      title={<span className={cx(!enabled && "text-faint")}>{callAs}</span>}
      subtitle={
        <span className="flex min-w-0 flex-col gap-4">
          {description && <span className={cx("truncate", enabled ? "text-muted" : "text-faint")}>{description}</span>}
          {!enabled && offNote && <span className="type-small text-faint">{offNote}</span>}
          {attention && (
            <span className="inline-flex w-fit items-center gap-4 rounded-pill border border-dashed border-border-strong px-6 type-small text-fg">
              Needs attention: missing {missing.join(", ")}
            </span>
          )}
        </span>
      }
      meta={meta}
      trailing={
        <span className="flex items-center gap-6">
          <Badge>v{version}</Badge>
          {modelBacked && <Badge>model-backed</Badge>}
        </span>
      }
      action={
        onEnabledChange ? (
          <Switch checked={enabled} onCheckedChange={onEnabledChange} aria-label={`${enabled ? "Switch off" : "Switch on"} ${callAs}`} />
        ) : undefined
      }
      href={href}
      onSelect={onSelect}
      className={className}
      {...rest}
    />
  );
}

export type FunctionGroupProps = {
  name: string;
  description?: ReactNode;
  /** Without it there is no edit action: a person who can only view the agent. */
  onEdit?: () => void;
  children: ReactNode;
} & Omit<HTMLAttributes<HTMLElement>, "title">;

/** A category of functions (spec 061): its name, description and edit action over its rows. */
export function FunctionGroup({ name, description, onEdit, children, className, ...rest }: FunctionGroupProps) {
  return (
    <section className={cx("overflow-hidden rounded-lg border border-border bg-surface", className)} {...rest}>
      <header className="flex items-start justify-between gap-12 border-b border-border px-14 py-10">
        <div className="flex min-w-0 flex-col gap-2">
          <h2 className="type-mono-sm text-fg">{name}</h2>
          {description && <p className="type-small text-muted">{description}</p>}
        </div>
        {onEdit && <IconButton icon="pencil" aria-label={`Rename or describe ${name}`} onClick={onEdit} />}
      </header>
      <div className="divide-y divide-border">{children}</div>
    </section>
  );
}
