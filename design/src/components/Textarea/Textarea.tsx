import type { ReactNode, TextareaHTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { FieldFrame, fieldBox, useFieldIds } from "../TextField/Field";

export type TextareaProps = {
  label?: ReactNode;
  helper?: ReactNode;
  invalid?: boolean;
  /** Arguments, environment, code: JetBrains Mono. */
  mono?: boolean;
  /** Shows "n / max" under the text; over the limit turns it red. */
  maxLength?: number;
  /** Visible rows before scrolling. */
  rows?: number;
} & TextareaHTMLAttributes<HTMLTextAreaElement>;

/** Multi-line text input with an optional counter (D13). */
export function Textarea({
  label,
  helper,
  invalid,
  mono,
  maxLength,
  rows = 4,
  disabled,
  readOnly,
  id,
  value,
  defaultValue,
  className,
  ...rest
}: TextareaProps) {
  const ids = useFieldIds(id);
  const length = String(value ?? defaultValue ?? "").length;
  const over = maxLength !== undefined && length > maxLength;
  return (
    <FieldFrame label={label} helper={helper} invalid={invalid || over} ids={ids} className={className}>
      <div className={cx(fieldBox({ invalid: invalid || over, disabled, readOnly }), "flex-col items-stretch gap-6 px-12 pt-8 pb-8")}>
        <textarea
          id={ids.input}
          rows={rows}
          disabled={disabled}
          readOnly={readOnly}
          value={value}
          defaultValue={defaultValue}
          aria-invalid={invalid || over || undefined}
          aria-describedby={helper ? ids.helper : undefined}
          className={cx("w-full resize-y bg-transparent text-inherit outline-none placeholder:text-faint", mono ? "type-code-block" : "type-ui-para")}
          {...rest}
        />
        {maxLength !== undefined && (
          <span aria-live="polite" className={cx("type-mono-xs", over ? "text-failed-text" : "text-faint")}>
            {length} / {maxLength}
          </span>
        )}
      </div>
    </FieldFrame>
  );
}
