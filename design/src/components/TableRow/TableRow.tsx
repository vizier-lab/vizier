import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type TableRowProps = {
  cells: ReactNode[];
  /** Align a cell's content to the end (the actions column), by index. */
  endAligned?: number[];
  selected?: boolean;
  /** Makes the row a target: hover, focus and Enter. */
  onSelect?: () => void;
} & Omit<HTMLAttributes<HTMLTableRowElement>, "onSelect">;

/** One row (D38). Keep cell order and widths in step with TableHeader. */
export function TableRow({ cells, endAligned = [], selected, onSelect, className, onKeyDown, ...rest }: TableRowProps) {
  const interactive = !!onSelect;
  return (
    <tr
      tabIndex={interactive ? 0 : undefined}
      aria-selected={interactive ? (selected ?? false) : undefined}
      onClick={onSelect}
      onKeyDown={(e) => {
        onKeyDown?.(e);
        if (interactive && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          onSelect();
        }
      }}
      className={cx(
        "border-b border-border",
        // Faint text falls under 4.5:1 on the pressed ground, so a selected row lifts it to muted.
        selected ? "bg-pressed [&_.text-faint]:text-muted [&_[data-title]]:type-body-strong" : interactive && "cursor-pointer hover:bg-raised",
        "focus-visible:-outline-offset-2",
        className,
      )}
      {...rest}
    >
      {cells.map((c, i) => (
        <td key={i} className={cx("px-16 py-12 align-middle", endAligned.includes(i) && "text-right")}>
          {c}
        </td>
      ))}
    </tr>
  );
}

/** The usual first cell: a title over a mono subtitle. */
export function TableTitle({ title, subtitle }: { title: ReactNode; subtitle?: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <span data-title className="type-body-medium text-fg">
        {title}
      </span>
      {subtitle && <span className="type-mono-xs text-faint">{subtitle}</span>}
    </div>
  );
}
