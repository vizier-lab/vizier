import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { FileTypeTag } from "../FileTypeTag/FileTypeTag";
import { TextLink } from "../TextLink/TextLink";

export type FilePeekProps = {
  name: string;
  /** "4 KB · 12 rows", "310 KB · 6 pages", "28 KB · no preview for this type". */
  meta?: ReactNode;
  /** The first rows or lines; omit for files with no preview. */
  preview?: ReactNode;
  /** Open (previewable), Download (no preview) or Retry (failed). */
  action: { label: "Open" | "Download" | "Retry"; onClick: () => void };
  failed?: boolean;
} & HTMLAttributes<HTMLDivElement>;

/** A file an agent sent, peeked inside its reply (X38): type, name, meta, and its first lines. */
export function FilePeek({ name, meta, preview, action, failed, className, ...rest }: FilePeekProps) {
  return (
    <div
      className={cx(
        "flex flex-col overflow-hidden rounded-lg border",
        failed ? "border-failed-line bg-failed-bg" : "border-border bg-surface",
        className,
      )}
      {...rest}
    >
      <div className="flex items-center gap-12 px-14 py-10">
        <FileTypeTag file={name} />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <span className="truncate type-code-medium text-fg">{name}</span>
          {(meta || failed) && <span className={cx("type-mono-xs", failed ? "text-failed-text" : "text-faint")}>{failed ? "couldn't load" : meta}</span>}
        </div>
        <TextLink size="ui" onClick={action.onClick} aria-label={`${action.label} ${name}`}>
          {action.label}
        </TextLink>
      </div>
      {preview && !failed && <div className="border-t border-border px-14 py-10">{preview}</div>}
    </div>
  );
}
