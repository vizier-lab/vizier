import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type ToolListProps = {
  tools: { name: string; description: ReactNode }[];
} & HTMLAttributes<HTMLDListElement>;

/** The tools an integration adds (X61): mono name, then what it does. */
export function ToolList({ tools, className, ...rest }: ToolListProps) {
  return (
    <dl className={cx("grid grid-cols-[max-content_1fr] items-baseline gap-x-12 gap-y-6", className)} {...rest}>
      {tools.map((t) => (
        <div key={t.name} className="contents">
          <dt className="type-mono-sm-medium text-fg">{t.name}</dt>
          <dd className="m-0 type-ui text-muted">{t.description}</dd>
        </div>
      ))}
    </dl>
  );
}
