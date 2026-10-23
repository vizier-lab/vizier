import type { HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type PaginationProps = {
  /** 1-based. */
  page: number;
  pageCount: number;
  total: number;
  perPage: number;
  onPageChange: (page: number) => void;
  /** Summary and ‹ › only (Memory bundles, Roles). */
  compact?: boolean;
} & HTMLAttributes<HTMLElement>;

/** The page numbers to show: first, last, and the current page's neighbours, with gaps. */
export function pageWindow(page: number, count: number): (number | "gap")[] {
  if (count <= 5) return Array.from({ length: count }, (_, i) => i + 1);
  const keep = new Set([1, count, page - 1, page, page + 1]);
  if (page <= 3) [2, 3].forEach((p) => keep.add(p));
  if (page >= count - 2) [count - 1, count - 2].forEach((p) => keep.add(p));
  const pages = [...keep].filter((p) => p >= 1 && p <= count).sort((a, b) => a - b);
  return pages.flatMap((p, i) => (i > 0 && p - pages[i - 1] > 1 ? ["gap" as const, p] : [p]));
}

const fmt = (n: number) => n.toLocaleString("en-US");

/** Under every paginated list (D39). The arrow that leads nowhere is disabled. */
export function Pagination({ page, pageCount, total, perPage, onPageChange, compact, className, ...rest }: PaginationProps) {
  const from = total === 0 ? 0 : (page - 1) * perPage + 1;
  const to = Math.min(total, page * perPage);
  const arrow = (dir: "prev" | "next") => {
    const target = dir === "prev" ? page - 1 : page + 1;
    const off = target < 1 || target > pageCount;
    return (
      <button
        type="button"
        aria-label={dir === "prev" ? "Previous page" : "Next page"}
        disabled={off}
        onClick={() => onPageChange(target)}
        className="flex size-24 items-center justify-center rounded-sm text-muted enabled:hover:bg-raised enabled:hover:text-fg disabled:opacity-30"
      >
        <Icon name={dir === "prev" ? "chevron-left" : "chevron-right"} size={13} />
      </button>
    );
  };
  return (
    <nav aria-label="Pages" className={cx("flex items-center justify-between gap-12 px-12 py-10", className)} {...rest}>
      <span className="type-mono-xs text-faint">
        {fmt(from)}–{fmt(to)} of {fmt(total)}
        {!compact && <> · {perPage} per page</>}
      </span>
      {pageCount > 1 && (
        <div className="flex items-center gap-4">
          {arrow("prev")}
          {!compact &&
            pageWindow(page, pageCount).map((p, i) =>
              p === "gap" ? (
                <span key={`gap-${i}`} aria-hidden className="flex size-24 items-center justify-center type-mono-xs text-muted">
                  …
                </span>
              ) : (
                <button
                  key={p}
                  type="button"
                  aria-current={p === page ? "page" : undefined}
                  aria-label={`Page ${p}`}
                  onClick={() => onPageChange(p)}
                  className={cx(
                    "flex h-24 min-w-24 items-center justify-center rounded-sm px-4",
                    p === page ? "bg-raised type-mono-xs-medium text-fg" : "type-mono-xs text-muted hover:bg-raised hover:text-fg",
                  )}
                >
                  {p}
                </button>
              ),
            )}
          {arrow("next")}
        </div>
      )}
    </nav>
  );
}
