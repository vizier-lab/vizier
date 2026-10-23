import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Button } from "../Button/Button";
import { FileTypeTag } from "../FileTypeTag/FileTypeTag";
import { IconButton } from "../IconButton/IconButton";

export type FilePreviewProps = {
  name: string;
  /** "4 KB · 12 rows · sent by Archivist at 14:02 in vector-db-review". */
  meta?: ReactNode;
  /** Table/Raw switch and its note, above the body. */
  viewBar?: ReactNode;
  /** The file itself: a Table, text, an image. */
  children: ReactNode;
  /** The reply's other files, for stepping through. */
  files?: { name: string; current?: boolean; onSelect: () => void }[];
  onPrevious?: () => void;
  onNext?: () => void;
  onDownload: () => void;
  onOpenInTab?: () => void;
  onClose: () => void;
  /** Dialog: over the chat (wrap it in Dialog's modal). Inline: in the conversation, compact. */
  look?: "dialog" | "inline";
} & HTMLAttributes<HTMLDivElement>;

/** A file at full size, with download, open in a new tab, and the reply's other files (X37). */
export function FilePreview({
  name,
  meta,
  viewBar,
  children,
  files,
  onPrevious,
  onNext,
  onDownload,
  onOpenInTab,
  onClose,
  look = "dialog",
  className,
  ...rest
}: FilePreviewProps) {
  const inline = look === "inline";
  return (
    <div
      className={cx("flex flex-col overflow-hidden rounded-xl border border-border-strong bg-surface", !inline && "shadow-dialog", className)}
      {...rest}
    >
      <header className="flex items-center gap-12 border-b border-border px-16 py-12">
        {!inline && <FileTypeTag file={name} />}
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <h2 className="truncate type-code-medium text-fg">{name}</h2>
          {meta && <p className="truncate type-mono-xs text-faint">{meta}</p>}
        </div>
        {inline ? (
          <>
            <button type="button" onClick={onDownload} className="rounded-xs type-ui-medium text-fg hover:underline">
              Download
            </button>
            {onOpenInTab && <IconButton variant="ghost" size="sm" icon="arrow-up-right" aria-label="Open in new tab" onClick={onOpenInTab} />}
          </>
        ) : (
          <>
            <Button onClick={onDownload}>Download</Button>
            {onOpenInTab && <Button onClick={onOpenInTab}>Open in new tab</Button>}
          </>
        )}
        <IconButton variant="ghost" size={inline ? "sm" : "md"} icon="close" aria-label="Close preview" onClick={onClose} />
      </header>
      {viewBar && <div className="flex items-center gap-12 border-b border-border px-16 py-8">{viewBar}</div>}
      <div className="min-h-0 flex-1 overflow-auto px-16 py-12">{children}</div>
      {(onPrevious || onNext || files) && (
        <footer className="flex flex-col gap-6 border-t border-border px-16 py-10">
          <div className="flex items-center gap-12">
            <button type="button" onClick={onPrevious} disabled={!onPrevious} className="inline-flex items-center gap-4 rounded-xs type-ui text-fg disabled:opacity-40">
              <Icon name="chevron-left" size={13} /> Previous
            </button>
            <div className="flex min-w-0 flex-1 items-center justify-center gap-6">
              {inline && files ? (
                <span className="type-mono-xs text-faint">
                  {files.findIndex((f) => f.current) + 1} of {files.length} files in this reply
                </span>
              ) : (
                files?.map((f) => (
                  <button
                    key={f.name}
                    type="button"
                    aria-current={f.current ? "true" : undefined}
                    onClick={f.onSelect}
                    className={cx(
                      "inline-flex items-center gap-6 rounded-sm px-8 py-4",
                      f.current ? "border border-border-strong bg-raised" : "hover:bg-raised",
                    )}
                  >
                    <span className="type-mono-xs text-faint">{f.name.split(".").pop()?.toUpperCase()}</span>
                    <span className={cx("type-code", f.current ? "text-fg" : "text-muted")}>{f.name}</span>
                  </button>
                ))
              )}
            </div>
            <button type="button" onClick={onNext} disabled={!onNext} className="inline-flex items-center gap-4 rounded-xs type-ui text-fg disabled:opacity-40">
              Next <Icon name="chevron-right" size={13} />
            </button>
          </div>
          {!inline && <p className="text-center type-mono-xs text-faint">← → switch file · Esc close</p>}
        </footer>
      )}
    </div>
  );
}
