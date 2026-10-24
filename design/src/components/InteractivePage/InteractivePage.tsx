import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { IconButton } from "../IconButton/IconButton";

export type InteractivePageProps = {
  title: ReactNode;
  /** Opens the page full screen. */
  onFullscreen?: () => void;
  /** Beside the title: "sandboxed page" for agent HTML (the default), "interface" for a generated one. `null` hides it. */
  label?: ReactNode;
  /** More header actions beside full screen: Download, Save as PDF. */
  actions?: ReactNode;
  /** The sandboxed iframe (or, in the catalogue, a stand-in). */
  children: ReactNode;
} & Omit<HTMLAttributes<HTMLElement>, "title">;

/** The frame around agent-made HTML in a reply (X47): title, "sandboxed page", full screen. */
export function InteractivePage({ title, label = "sandboxed page", onFullscreen, actions, children, className, ...rest }: InteractivePageProps) {
  return (
    <section className={cx("overflow-hidden rounded-lg border border-border-strong", className)} {...rest}>
      <header className="flex items-center gap-8 border-b border-border bg-surface py-6 pr-6 pl-12">
        <h3 className="min-w-0 flex-1 truncate type-ui-medium text-fg">{title}</h3>
        {label !== null && <span className="type-mono-xs text-faint">{label}</span>}
        {actions}
        {onFullscreen && <IconButton variant="ghost" size="sm" icon="maximize" aria-label="Open full screen" onClick={onFullscreen} />}
      </header>
      <div className="bg-ground">{children}</div>
    </section>
  );
}
