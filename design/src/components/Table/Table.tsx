import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { TableHeader, type TableColumn } from "../TableHeader/TableHeader";

export type TableProps = {
  /** Names the table for assistive tech. */
  label: string;
  /** Columns, each with an optional width (px or a CSS length); leave one flexible. */
  columns: (TableColumn & { width?: number | string })[];
  /** TableRows. */
  children: ReactNode;
  /** Under the rows: usually Pagination. */
  footer?: ReactNode;
  /** Dense mono cells, for data previews (CSV). */
  dense?: boolean;
} & HTMLAttributes<HTMLDivElement>;

/** A card holding a header, rows and a footer (D36). Widths are set once, for header and rows. */
export function Table({ label, columns, children, footer, dense, className, ...rest }: TableProps) {
  // Fixed widths for the columns that have one; the flexible one takes the rest and truncates, so a
  // long title can't stretch the table past its card. A narrow screen scrolls the table sideways.
  const fixed = columns.some((c) => c.width !== undefined) && columns.some((c) => c.width === undefined);
  const floor = columns.reduce((n, c) => n + (typeof c.width === "number" ? c.width : c.width === undefined ? 100 : 0), 0);
  return (
    <div className={cx("overflow-hidden rounded-xl border border-border bg-surface", className)} {...rest}>
      <div className="overflow-x-auto">
        <table
          aria-label={label}
          style={fixed ? { minWidth: floor } : undefined}
          className={cx(
            "w-full border-collapse",
            fixed && "table-fixed",
            dense && "[&_td]:px-12 [&_td]:py-4 [&_td]:type-mono-sm [&_td]:text-soft [&_th]:px-12",
          )}
        >
          <colgroup>
            {columns.map((c, i) => (
              <col key={i} style={c.width !== undefined ? { width: c.width } : undefined} />
            ))}
          </colgroup>
          <TableHeader columns={columns} />
          <tbody>{children}</tbody>
        </table>
      </div>
      {footer && <div className="border-t border-border">{footer}</div>}
    </div>
  );
}
