import type { HTMLAttributes, ReactNode } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";

export type ChatGreetingProps = {
  agentId: string;
  name: string;
  /** "What shall we deliberate?". */
  greeting?: ReactNode;
  /** "Topic 4 of 5 · nothing sent yet". */
  meta?: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** What an empty topic shows in place of history (X34). */
export function ChatGreeting({ agentId, name, greeting = "What shall we deliberate?", meta, className, ...rest }: ChatGreetingProps) {
  return (
    <div className={cx("flex flex-col items-center gap-6 text-center", className)} {...rest}>
      <span className="flex items-center gap-8">
        <Sigil agentId={agentId} size="sm" />
        <span className="type-body-medium text-fg">{name}</span>
      </span>
      <h2 className="type-title text-fg">{greeting}</h2>
      {meta && <p className="type-mono-xs text-faint">{meta}</p>}
    </div>
  );
}
