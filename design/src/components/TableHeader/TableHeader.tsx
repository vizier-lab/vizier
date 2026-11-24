import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type TableColumn = {
  /** Uppercase mono label; empty for an actions column. */
  label: ReactNode;
  /** Visually hidden label, for columns whose label is empty. */
  srLabel?: string;
  align?: "start" | "end";
  /** Makes the header a sort button. `sorted` is the column's current direction, if it is the sorted one. */
  onSort?: () => void;
  sorted?: "asc" | "desc";
};

export type TableHeaderProps = {
  columns: TableColumn[];
} & HTMLAttributes<HTMLTableSectionElement>;

/** Column labels (D37). Widths come from the table's `<colgroup>`, shared with its rows. */
export function TableHeader({ columns, className, ...rest }: TableHeaderProps) {
  return (
    <thead className={cx("bg-sidebar", className)} {...rest}>
      <tr>
        {columns.map((c, i) => (
          <th
            key={i}
            scope="col"
            aria-sort={c.onSort ? (c.sorted === "asc" ? "ascending" : c.sorted === "desc" ? "descending" : "none") : undefined}
            className={cx(
              "h-34 px-16 py-10 align-middle type-label uppercase text-faint whitespace-nowrap",
              c.align === "end" ? "text-right" : "text-left",
            )}
          >
            {c.onSort ? (
              <button type="button" onClick={c.onSort} className="inline-flex items-center gap-4 uppercase hover:text-fg">
                {c.label}
                <span aria-hidden className="type-mono-xs">{c.sorted === "asc" ? "↑" : c.sorted === "desc" ? "↓" : ""}</span>
              </button>
            ) : (
              c.label
            )}
            {c.srLabel && <span className="sr-only">{c.srLabel}</span>}
          </th>
        ))}
      </tr>
    </thead>
  );
}
