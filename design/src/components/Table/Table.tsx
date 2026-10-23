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
  return (
    <div className={cx("overflow-hidden rounded-xl border border-border bg-surface", className)} {...rest}>
      <div className="overflow-x-auto">
        <table
          aria-label={label}
          className={cx(
            "w-full border-collapse",
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
