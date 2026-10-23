import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";

export type CardProps = {
  title?: ReactNode;
  /** One line under the title. */
  description?: ReactNode;
  /** Right of the header: a Switch, a count, a close ×. */
  headerTrailing?: ReactNode;
  /** Danger: the red-bordered Danger zone. Dashed: a hint card (see Callout). */
  tone?: "default" | "danger" | "dashed";
  /** Heading level for the title. */
  level?: 2 | 3 | 4;
} & Omit<HTMLAttributes<HTMLElement>, "title">;

/** A surface panel with a heading and description; content goes in children (D34). */
export function Card({ title, description, headerTrailing, tone = "default", level = 2, className, children, ...rest }: CardProps) {
  const H = `h${level}` as const;
  return (
    <section
      className={cx(
        "flex flex-col gap-16 rounded-xl border px-24 pt-20 pb-20",
        tone === "danger" ? "border-failed-line bg-surface" : tone === "dashed" ? "border-dashed border-border-strong" : "border-border bg-surface",
        className,
      )}
      {...rest}
    >
      {(title || headerTrailing) && (
        <header className="flex items-start gap-12">
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            {title && <H className={cx("type-heading", tone === "danger" ? "text-failed-text" : "text-fg")}>{title}</H>}
            {description && <p className="type-small text-faint">{description}</p>}
          </div>
          {headerTrailing && <div className="flex shrink-0 items-center gap-8">{headerTrailing}</div>}
        </header>
      )}
      {children}
    </section>
  );
}
