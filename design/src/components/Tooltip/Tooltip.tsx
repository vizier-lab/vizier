import { cloneElement, useId, useState, type ReactElement, type ReactNode } from "react";
import { cx } from "../../lib/cx";

export type TooltipProps = {
  /** First line. */
  label: ReactNode;
  /** A key hint after the label: "Ctrl B". */
  shortcut?: string;
  /** A second, muted line (the context ring: "model · checkpoints automatically"). */
  detail?: ReactNode;
  side?: "top" | "bottom" | "right" | "left";
  /** The element it describes; it gets aria-describedby. */
  children: ReactElement<{ "aria-describedby"?: string }>;
};

const SIDES = {
  top: "bottom-full left-1/2 mb-6 -translate-x-1/2",
  bottom: "top-full left-1/2 mt-6 -translate-x-1/2",
  right: "left-full top-1/2 ml-6 -translate-y-1/2",
  left: "right-full top-1/2 mr-6 -translate-y-1/2",
};

/**
 * A short label on hover or keyboard focus (D33). Ink on ground: light-on-dark
 * in light mode, dark-on-light in dark mode. Esc hides it.
 */
export function Tooltip({ label, shortcut, detail, side = "top", children }: TooltipProps) {
  const id = useId();
  const [open, setOpen] = useState(false);
  return (
    <span
      className="relative inline-flex"
      onPointerEnter={() => setOpen(true)}
      onPointerLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
      onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
    >
      {cloneElement(children, { "aria-describedby": id })}
      <TooltipBubble
        id={id}
        label={label}
        shortcut={shortcut}
        detail={detail}
        className={cx(
          "absolute z-10 transition-opacity",
          SIDES[side],
          !open && "hidden",
          open && "animate-fade-in motion-reduce:animate-none",
        )}
      />
    </span>
  );
}

/** The bubble alone, for static layouts and the catalogue. */
export function TooltipBubble({
  id,
  label,
  shortcut,
  detail,
  className,
}: Pick<TooltipProps, "label" | "shortcut" | "detail"> & { id?: string; className?: string }) {
  return (
    <span
      id={id}
      role="tooltip"
      className={cx(
        "pointer-events-none flex w-max max-w-280 flex-col gap-2 rounded-sm bg-ink px-10 py-6 shadow-popover",
        className,
      )}
    >
      <span className="flex items-center gap-8">
        <span className="type-small-medium text-ground">{label}</span>
        {shortcut && <kbd className="type-mono-xs text-ground opacity-60">{shortcut}</kbd>}
      </span>
      {detail && <span className="type-mono-xs text-ground opacity-70">{detail}</span>}
    </span>
  );
}
