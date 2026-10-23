import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type BreadcrumbProps = {
  /** Path segments, mono; the last is the current page. Give earlier ones an href. */
  segments?: { label: ReactNode; href?: string }[];
  /** Instead of a path: one back link ("← Skills") and a badge after the title. */
  back?: { label: ReactNode; href: string };
  trailing?: ReactNode;
} & HTMLAttributes<HTMLElement>;

/** Where you are (X9): a mono path, or a back link. */
export function Breadcrumb({ segments, back, trailing, className, ...rest }: BreadcrumbProps) {
  if (back) {
    return (
      <nav aria-label="Breadcrumb" className={cx("flex items-center gap-8", className)} {...rest}>
        <a href={back.href} className="inline-flex items-center gap-4 rounded-xs type-ui-medium text-muted hover:text-fg">
          <Icon name="chevron-left" size={14} />
          {back.label}
        </a>
        {trailing}
      </nav>
    );
  }
  return (
    <nav aria-label="Breadcrumb" className={cx("flex items-center gap-8", className)} {...rest}>
      <ol className="flex min-w-0 items-center gap-6 type-code">
        {segments?.map((s, i) => {
          const last = i === segments.length - 1;
          return (
            <li key={i} className="flex min-w-0 items-center gap-6">
              {i > 0 && <span aria-hidden className="text-faint">/</span>}
              {last || !s.href ? (
                <span aria-current={last ? "page" : undefined} className={cx("truncate", last ? "text-fg" : "text-muted")}>
                  {s.label}
                </span>
              ) : (
                <a href={s.href} className="truncate rounded-xs text-muted hover:text-fg hover:underline">
                  {s.label}
                </a>
              )}
            </li>
          );
        })}
      </ol>
      {trailing}
    </nav>
  );
}
