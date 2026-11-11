import { useId, type HTMLAttributes, type ReactNode } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { Presence, scrimMotion, slideOverBottomMotion, slideOverLeftMotion, slideOverMotion, useNativeDialog } from "../../lib/Motion";
import { IconButton } from "../IconButton/IconButton";

export type SlideOverSide = "right" | "left" | "bottom";

export type SlideOverPanelProps = {
  title: ReactNode;
  /** Footer actions, right-aligned. */
  footer?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  titleId?: string;
  /**
   * Which edge the panel belongs to: the side of the control that opens it.
   * Right (the default) for detail surfaces; left for the phone menu, whose
   * button is on the left. Bottom is a phone's sheet (the agent switcher, the topic picker):
   * full width, rounded on top, with a grab handle and the bottom safe area. It sets the edge
   * line and the slide direction.
   */
  side?: SlideOverSide;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

/** The panel surface, without the modal behaviour. */
export function SlideOverPanel({ title, footer, onClose, children, titleId, side = "right", className, ...rest }: SlideOverPanelProps) {
  const sheet = side === "bottom";
  return (
    <div
      className={cx(
        "flex max-w-full flex-col bg-surface shadow-dialog",
        sheet
          ? "max-h-[85dvh] w-full rounded-t-2xl border-t border-border-strong pb-safe"
          : cx("h-full w-440", side === "left" ? "border-r border-border-strong" : "border-l border-border-strong"),
        className,
      )}
      {...rest}
    >
      {sheet && <span aria-hidden className="mx-auto mt-10 h-5 w-36 shrink-0 rounded-pill bg-border-strong" />}
      <header className={cx("flex items-center gap-10 py-14 pr-14 pl-20", sheet ? "pt-10" : "border-b border-border")}>
        <h2 id={titleId} className="min-w-0 flex-1 truncate type-heading text-fg">
          {title}
        </h2>
        <IconButton variant="ghost" icon="close" aria-label="Close" onClick={onClose} />
      </header>
      <div className={cx("flex min-h-0 flex-1 flex-col gap-12 overflow-y-auto", sheet ? "px-16 pb-16" : "p-20")}>{children}</div>
      {footer && <footer className="flex justify-end gap-10 border-t border-border px-20 py-12">{footer}</footer>}
    </div>
  );
}

export type SlideOverProps = SlideOverPanelProps & { open: boolean };

/** A side panel over the page, full height (D35), or a bottom sheet on a phone. Esc closes it; the panel slides with its scrim. */
export function SlideOver({ open, side = "right", onClose, ...panel }: SlideOverProps) {
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
        {open && (
          <motion.div
            key="scrim"
            aria-hidden
            {...scrimMotion}
            className="absolute inset-0 bg-ground/60"
            // A click on the ground outside the panel dismisses it, as Escape does.
            onClick={onClose}
          />
        )}
        {open && (
          <motion.div
            key="panel"
            {...(side === "left" ? slideOverLeftMotion : side === "bottom" ? slideOverBottomMotion : slideOverMotion)}
            className={cx(
              "absolute flex max-w-full",
              side === "bottom" ? "inset-x-0 bottom-0" : cx("inset-y-0", side === "left" ? "left-0" : "right-0"),
            )}
          >
            <SlideOverPanel {...panel} side={side} titleId={titleId} onClose={onClose} />
          </motion.div>
        )}
      </Presence>
    </dialog>
  );
}
