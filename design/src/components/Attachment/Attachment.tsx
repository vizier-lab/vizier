import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { DotLoader } from "../DotLoader/DotLoader";

export type AttachmentProps = {
  name: string;
  /** Size ("12 KB"), upload progress ("48%"), or the reason it failed ("over 25 MB"). */
  detail?: ReactNode;
  state?: "ready" | "uploading" | "failed";
  /** Shows remove ×: in the composer, not on sent messages. */
  onRemove?: () => void;
} & HTMLAttributes<HTMLSpanElement>;

/** A file chip on a message or in the composer (D57). */
export function Attachment({ name, detail, state = "ready", onRemove, className, ...rest }: AttachmentProps) {
  const failed = state === "failed";
  return (
    <span
      className={cx(
        "inline-flex h-23 max-w-full items-center gap-6 rounded-sm pr-6 pl-6",
        failed ? "border border-failed-line bg-failed-bg" : "bg-raised",
        className,
      )}
      {...rest}
    >
      {state === "uploading" ? (
        <DotLoader tone="muted" label={`Uploading ${name}`} />
      ) : (
        <Icon name={failed ? "alert" : "file"} size={13} tone={failed ? "failed" : "muted"} />
      )}
      <span className="min-w-0 truncate type-small-medium text-fg">{name}</span>
      {detail && <span className={cx("shrink-0 type-mono-xs", failed ? "text-failed-text" : "text-faint")}>{detail}</span>}
      {onRemove && (
        <button type="button" aria-label={`Remove ${name}`} onClick={onRemove} className="flex size-16 items-center justify-center rounded-xs text-muted hover:text-fg">
          <Icon name="close" size={12} />
        </button>
      )}
    </span>
  );
}
