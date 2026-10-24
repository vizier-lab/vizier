import { useId, type HTMLAttributes, type ReactNode } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { Presence, scrimMotion, slideOverMotion, useNativeDialog } from "../../lib/Motion";
import { IconButton } from "../IconButton/IconButton";

export type SlideOverPanelProps = {
  title: ReactNode;
  /** Footer actions, right-aligned. */
  footer?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  titleId?: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

/** The panel surface, without the modal behaviour. */
export function SlideOverPanel({ title, footer, onClose, children, titleId, className, ...rest }: SlideOverPanelProps) {
  return (
    <div className={cx("flex h-full w-440 max-w-full flex-col border-l border-border-strong bg-surface shadow-dialog", className)} {...rest}>
      <header className="flex items-center gap-10 border-b border-border py-14 pr-14 pl-20">
        <h2 id={titleId} className="min-w-0 flex-1 truncate type-heading text-fg">
          {title}
        </h2>
        <IconButton variant="ghost" icon="close" aria-label="Close" onClick={onClose} />
      </header>
      <div className="flex min-h-0 flex-1 flex-col gap-12 overflow-y-auto p-20">{children}</div>
      {footer && <footer className="flex justify-end gap-10 border-t border-border px-20 py-12">{footer}</footer>}
    </div>
  );
}

export type SlideOverProps = SlideOverPanelProps & { open: boolean };

/** A right-hand panel over the page, full height (D35). Esc closes it; the panel slides with its scrim. */
export function SlideOver({ open, onClose, ...panel }: SlideOverProps) {
  const titleId = useId();
  const { ref, onExitComplete } = useNativeDialog(open);
  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      // A native dialog is sized fit-content by the browser, so it needs its
      // own size: the scrim and panel inside are absolute, and would
      // otherwise give it nothing to size to.
      className="fixed inset-0 h-dvh w-full m-0 max-h-none max-w-none bg-transparent p-0 backdrop:bg-transparent"
    >
      <Presence onExitComplete={onExitComplete}>
        {open && <motion.div key="scrim" aria-hidden {...scrimMotion} className="absolute inset-0 bg-ground/60" />}
        {open && (
          <motion.div key="panel" {...slideOverMotion} className="absolute inset-y-0 right-0 flex max-w-full">
            <SlideOverPanel {...panel} titleId={titleId} onClose={onClose} />
          </motion.div>
        )}
      </Presence>
    </dialog>
  );
}
