import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type ThinkingProps = {
  /** "Archivist is thinking…", "Archivist is answering Scout's report…". */
  children: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** A turn in progress with nothing to show yet (D47): three equal dots, the last one live. */
export function Thinking({ className, children, ...rest }: ThinkingProps) {
  return (
    <div role="status" className={cx("flex items-center gap-8", className)} {...rest}>
      <span aria-hidden className="flex gap-4">
        {["bg-muted", "bg-muted", "bg-live"].map((c, i) => (
          <span key={i} className={cx("size-5 rounded-pill animate-dot-wave motion-reduce:animate-none", c)} style={{ animationDelay: `${i * 0.2}s` }} />
        ))}
      </span>
      <span className="type-ui text-muted">{children}</span>
    </div>
  );
}
