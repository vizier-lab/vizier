import { useRef, type HTMLAttributes } from "react";
import { Sigil } from "../../brand/Sigil/Sigil";
import { cx } from "../../lib/cx";

export type SigilPickerProps = {
  agentId: string;
  /** An uploaded picture, shown instead of the sigil. */
  imageSrc?: string;
  /** A file chosen to upload; the caller crops and saves it. */
  onUpload: (file: File) => void;
  /** Back to the generated sigil. */
  onReset?: () => void;
} & HTMLAttributes<HTMLDivElement>;

/** An agent's face on Config (X22): its generated sigil, or an uploaded picture. */
export function SigilPicker({ agentId, imageSrc, onUpload, onReset, className, ...rest }: SigilPickerProps) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <div className={cx("flex flex-col items-center gap-6", className)} {...rest}>
      {imageSrc ? (
        <img src={imageSrc} alt="Agent picture" className="size-86 rounded-xl border border-border-strong object-cover" />
      ) : (
        <Sigil agentId={agentId} size="xl" label="Generated sigil" />
      )}
      <p className="type-small text-faint">
        {imageSrc && onReset ? (
          <button type="button" onClick={onReset} className="rounded-xs hover:text-fg hover:underline">
            Sigil
          </button>
        ) : (
          "Sigil"
        )}{" "}
        ·{" "}
        <button type="button" onClick={() => input.current?.click()} className="rounded-xs hover:text-fg hover:underline">
          or upload
        </button>
      </p>
      <input ref={input} type="file" accept="image/*" className="hidden" onChange={(e) => e.target.files?.[0] && onUpload(e.target.files[0])} />
    </div>
  );
}
