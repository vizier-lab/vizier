import type { HTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";
import { Eyebrow } from "../Eyebrow/Eyebrow";
import { KeyValueList } from "../KeyValueList/KeyValueList";

export type AgentPreviewCardProps = {
  agentId: string;
  name: string;
  /** "meeting notes". */
  description?: ReactNode;
  facts: { label: ReactNode; value: ReactNode }[];
  note?: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** The new agent as it will be (X55), beside the New agent wizard. */
export function AgentPreviewCard({ agentId, name, description, facts, note, className, ...rest }: AgentPreviewCardProps) {
  return (
    <aside aria-label="Preview" className={cx("flex flex-col gap-14 rounded-xl border border-border bg-surface px-20 py-20", className)} {...rest}>
      <Eyebrow>Preview</Eyebrow>
      <div className="flex items-center gap-14">
        <Sigil agentId={agentId} size="lg" />
        <div className="flex min-w-0 flex-col gap-2">
          <span className="truncate type-heading text-fg">{name}</span>
          <span className="truncate type-mono-xs text-faint">
            {agentId}
            {description && <> · {description}</>}
          </span>
        </div>
      </div>
      <KeyValueList items={facts} />
      {note && <p className="type-small-para text-muted">{note}</p>}
    </aside>
  );
}
