import { useState, type HTMLAttributes, type ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";
import { TextLink } from "../TextLink/TextLink";

export type BackgroundReportCardProps = {
  /** The agent that reported, for its sigil. */
  agentId: string;
  /** "delegate_agent → Scout · 2m 14s". */
  meta: ReactNode;
  /** "Open Scout's conversation". With `details`, the toggle's label instead ("What each piece found"). */
  linkLabel: ReactNode;
  /** Where the conversation is, when it can be opened. */
  href?: string;
  /** What the job found, shown in place when the toggle opens (no page to open). */
  details?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** A background job's report, arriving in the conversation (X44). It wakes the agent's next turn. */
export function BackgroundReportCard({ agentId, meta, linkLabel, href, details, className, children, ...rest }: BackgroundReportCardProps) {
  const [open, setOpen] = useState(false);
  return (
    <div className={cx("flex flex-col gap-8 rounded-lg border border-dashed border-border-strong px-16 py-14", className)} {...rest}>
      <div className="flex items-center gap-8">
        <Sigil agentId={agentId} size="sm" />
        <span className="type-ui-medium text-fg">Background job reported</span>
        <span className="min-w-0 truncate type-mono-xs text-faint">{meta}</span>
      </div>
      <div className="type-body-para text-fg">{children}</div>
      {href ? (
        <div>
          <TextLink href={href} arrow="forward" size="ui">
            {linkLabel}
          </TextLink>
        </div>
      ) : (
        details && (
          <>
            <div>
              <TextLink onClick={() => setOpen(!open)} aria-expanded={open} size="ui">
                {open ? "Hide" : linkLabel}
              </TextLink>
            </div>
            {open && <div className="flex flex-col gap-12 border-t border-border pt-12">{details}</div>}
          </>
        )
      )}
    </div>
  );
}
