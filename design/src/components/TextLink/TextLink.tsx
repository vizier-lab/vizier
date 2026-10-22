import type { AnchorHTMLAttributes, ButtonHTMLAttributes } from "react";
import { cx } from "../../lib/cx";

type Common = {
  tone?: "default" | "danger";
  /** → for in-app navigation, ↗ for leaving the conversation or app. */
  arrow?: "forward" | "external";
  /** small (notices, jobs) or ui (pages). */
  size?: "small" | "ui";
};

export type TextLinkProps = Common &
  (
    | ({ href: string } & AnchorHTMLAttributes<HTMLAnchorElement>)
    | ({ href?: undefined } & ButtonHTMLAttributes<HTMLButtonElement>)
  );

/** A text-only action (X7): Retry, Switch model, Open Providers →, Delete skill. */
export function TextLink({ tone = "default", arrow, size = "small", className, children, ...rest }: TextLinkProps) {
  const classes = cx(
    "inline-flex items-center gap-4 rounded-xs underline-offset-2 hover:underline disabled:pointer-events-none disabled:opacity-40",
    size === "small" ? "type-small-medium" : "type-ui-medium",
    tone === "danger" ? "text-failed" : "text-fg",
    className,
  );
  const content = (
    <>
      {children}
      {arrow && <span aria-hidden>{arrow === "forward" ? "→" : "↗"}</span>}
    </>
  );
  if (rest.href !== undefined) {
    const { href, ...a } = rest as AnchorHTMLAttributes<HTMLAnchorElement>;
    return (
      <a href={href} className={classes} {...a}>
        {content}
      </a>
    );
  }
  const { type = "button", ...b } = rest as ButtonHTMLAttributes<HTMLButtonElement>;
  return (
    <button type={type} className={classes} {...b}>
      {content}
    </button>
  );
}
