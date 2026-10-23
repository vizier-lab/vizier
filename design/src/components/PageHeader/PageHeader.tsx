import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type PageHeaderProps = {
  title: ReactNode;
  /** Mono meta line under the title: "312 tasks · 287 active · next run Mon 09:00". */
  meta?: ReactNode;
  /** Settings pages: a smaller title with a sentence of description. */
  description?: ReactNode;
  /** Right slot: buttons, a range Segment + SortSelect, a Stepper, UnsavedIndicator + buttons. */
  actions?: ReactNode;
  /** app: Type/title. settings: Type/heading with description. */
  size?: "app" | "settings";
} & Omit<HTMLAttributes<HTMLElement>, "title">;

/** A page's title, meta line and actions (D7). */
export function PageHeader({ title, meta, description, actions, size = "app", className, ...rest }: PageHeaderProps) {
  return (
    <header className={cx("flex flex-wrap items-start justify-between gap-x-24 gap-y-12", className)} {...rest}>
      <div className="flex min-w-0 flex-col gap-4">
        <h1 className={cx("text-fg", size === "app" ? "type-title" : "type-heading")}>{title}</h1>
        {meta && <p className="type-mono-xs text-faint">{meta}</p>}
        {description && <p className="type-ui-para text-muted">{description}</p>}
      </div>
      {actions && <div className="flex max-w-full flex-wrap items-center gap-10">{actions}</div>}
    </header>
  );
}
