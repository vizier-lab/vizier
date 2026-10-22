import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type KeyValueListProps = {
  items: { label: ReactNode; value: ReactNode }[];
} & HTMLAttributes<HTMLDListElement>;

/** Label/value rows (X3). A value may be text, a Status, a link or an action. */
export function KeyValueList({ items, className, ...rest }: KeyValueListProps) {
  return (
    <dl className={cx("flex flex-col gap-8", className)} {...rest}>
      {items.map((item, i) => (
        <div key={i} className="flex min-h-16 items-center justify-between gap-12">
          <dt className="type-ui text-faint">{item.label}</dt>
          <dd className="m-0 min-w-0 truncate text-right type-ui text-fg">{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}
