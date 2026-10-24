import { useId, useState, type HTMLAttributes, type ReactNode } from "react";
import { motion } from "motion/react";
import { cx } from "../../lib/cx";
import { Presence, dialogMotion, scrimMotion, useNativeDialog } from "../../lib/Motion";
import { Button } from "../Button/Button";
import { TextField } from "../TextField/TextField";

export type DialogPanelProps = {
  title: ReactNode;
  /** The consequence, in a sentence or two. */
  message?: ReactNode;
  /** Default: a primary confirm. Danger: a destructive confirm. */
  tone?: "default" | "danger";
  /** Asks the person to type this word before the confirm enables. */
  confirmWord?: string;
  /** Empty: a content dialog whose children carry their own buttons, so no actions row. */
  confirmLabel: ReactNode;
  confirmIcon?: React.ComponentProps<typeof Button>["icon"];
  cancelLabel?: ReactNode;
  onConfirm: () => void;
  onCancel: () => void;
  /** Large: a wider panel for forms and previews. */
  size?: "md" | "lg";
  /** Content dialogs put their body here. */
  children?: ReactNode;
  titleId?: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "title">;

/** The dialog surface, without the modal behaviour. */
export function DialogPanel({
  title,
  message,
  tone = "default",
  confirmWord,
  confirmLabel,
  confirmIcon,
  cancelLabel = "Cancel",
  onConfirm,
  onCancel,
  size = "md",
  children,
  titleId,
  className,
  ...rest
}: DialogPanelProps) {
  const [typed, setTyped] = useState("");
  const ready = !confirmWord || typed === confirmWord;
  return (
    <div
      className={cx(
        "flex flex-col gap-16 rounded-xl border border-border-strong bg-surface px-24 py-20 shadow-dialog",
        size === "md" ? "w-440" : "w-760",
        "max-w-full",
        className,
      )}
      {...rest}
    >
      <h2 id={titleId} className="type-heading text-fg">
        {title}
      </h2>
      {message && <p className="type-ui-para text-muted">{message}</p>}
      {children}
      {confirmWord && (
        <TextField
          label={
            <>
              Type <span className="type-code">{confirmWord}</span> to confirm
            </>
          }
          mono
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          autoComplete="off"
          spellCheck={false}
        />
      )}
      {confirmLabel && (
        <div className="flex justify-end gap-10 pt-4">
          <Button onClick={onCancel}>{cancelLabel}</Button>
          <Button variant={tone === "danger" ? "destructive" : "primary"} icon={confirmIcon} disabled={!ready} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </div>
      )}
    </div>
  );
}

export type DialogProps = DialogPanelProps & { open: boolean };

/**
 * A modal confirmation over a 60% ground scrim (D28). Native `<dialog>`:
 * focus moves in, Tab stays inside, Esc cancels. The scrim and panel settle
 * in and animate out before the dialog closes.
 */
export function Dialog({ open, onCancel, ...panel }: DialogProps) {
  const titleId = useId();
  const { ref, onExitComplete } = useNativeDialog(open);
  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      className="fixed inset-0 m-0 max-h-none max-w-none bg-transparent p-0 backdrop:bg-transparent"
    >
      <Presence onExitComplete={onExitComplete}>
        {open && <motion.div key="scrim" aria-hidden {...scrimMotion} className="absolute inset-0 bg-ground/60" />}
        {open && (
          <motion.div key="panel" {...dialogMotion} className="absolute inset-0 flex overflow-y-auto p-20">
            <DialogPanel {...panel} titleId={titleId} onCancel={onCancel} className="m-auto" />
          </motion.div>
        )}
      </Presence>
    </dialog>
  );
}

/**
 * The modal shell alone: a native `<dialog>` with the ground scrim, holding whatever you give it
 * (a file preview). Esc, an outside click and the scrim's cancel call `onClose`.
 */
export function Modal({ open, onClose, label, children }: { open: boolean; onClose: () => void; label: string; children: ReactNode }) {
  const { ref, onExitComplete } = useNativeDialog(open);
  return (
    <dialog
      ref={ref}
      aria-label={label}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      className="fixed inset-0 m-0 max-h-none max-w-none bg-transparent p-0 backdrop:bg-transparent"
    >
      <Presence onExitComplete={onExitComplete}>
        {open && <motion.div key="scrim" aria-hidden {...scrimMotion} className="absolute inset-0 bg-ground/60" />}
        {open && (
          <motion.div
            key="panel"
            {...dialogMotion}
            className="absolute inset-0 flex overflow-y-auto p-20"
            onClick={(e) => e.target === e.currentTarget && onClose()}
          >
            <div className="m-auto max-h-[92vh] max-w-[94vw] overflow-visible">{children}</div>
          </motion.div>
        )}
      </Presence>
    </dialog>
  );
}
