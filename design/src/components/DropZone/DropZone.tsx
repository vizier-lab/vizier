import { useRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type DropZoneProps = {
  onFiles: (files: File[]) => void;
  /** Inline: a dashed strip with "browse". Overlay: fills its (relative) parent while dragging. */
  look?: "inline" | "overlay";
  /** Overlay only: show it (the parent tracks drag-enter/leave). */
  active?: boolean;
  accept?: string;
  multiple?: boolean;
  children?: ReactNode;
} & Omit<HTMLAttributes<HTMLDivElement>, "onDrop">;

/** Where files are dropped (X19). */
export function DropZone({ onFiles, look = "inline", active, accept, multiple = true, children, className, ...rest }: DropZoneProps) {
  const input = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  const handlers = {
    onDragOver: (e: React.DragEvent) => {
      e.preventDefault();
      setOver(true);
    },
    onDragLeave: () => setOver(false),
    onDrop: (e: React.DragEvent) => {
      e.preventDefault();
      setOver(false);
      onFiles(Array.from(e.dataTransfer.files));
    },
  };
  if (look === "overlay") {
    if (!active) return null;
    return (
      <div
        className={cx("absolute inset-0 z-20 flex flex-col items-center justify-center gap-8 rounded-xl border border-dashed border-live bg-ground/90", className)}
        {...handlers}
        {...rest}
      >
        <Icon name="attach" size={22} tone="live" />
        <p className="type-body-medium text-fg">{children ?? "Drop to attach"}</p>
      </div>
    );
  }
  return (
    <div
      className={cx(
        "flex items-center justify-center rounded-md border border-dashed px-10 py-14 text-center",
        over ? "border-live bg-surface" : "border-border-strong",
        className,
      )}
      {...handlers}
      {...rest}
    >
      <p className="type-small text-faint">
        {children ?? "Drop files here, or"}{" "}
        <button type="button" onClick={() => input.current?.click()} className="rounded-xs text-fg underline-offset-2 hover:underline">
          browse
        </button>
      </p>
      <input
        ref={input}
        type="file"
        accept={accept}
        multiple={multiple}
        className="hidden"
        onChange={(e) => e.target.files && onFiles(Array.from(e.target.files))}
      />
    </div>
  );
}
