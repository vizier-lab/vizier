import type { InputHTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { FieldFrame, fieldBox, useFieldIds } from "./Field";

export type TextFieldProps = {
  label?: ReactNode;
  /** Helper text under the field; in the error colour when `invalid`. */
  helper?: ReactNode;
  /** FigCraft "Error". Sets aria-invalid. */
  invalid?: boolean;
  /** Ids, model names, paths: JetBrains Mono. */
  mono?: boolean;
  /** A unit after the value: "tokens", "%". */
  suffix?: ReactNode;
  /** md 40; lg 46 (Login, Onboarding). */
  size?: "md" | "lg";
  /** Read-only with a lock, for values fixed at creation (Agent ID). */
  locked?: boolean;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "size">;

/** A single-line text input with label and helper (D12). */
export function TextField({
  label,
  helper,
  invalid,
  mono,
  suffix,
  size = "md",
  locked,
  readOnly,
  disabled,
  id,
  className,
  ...rest
}: TextFieldProps) {
  const ids = useFieldIds(id);
  const ro = readOnly || locked;
  return (
    <FieldFrame label={label} helper={helper} invalid={invalid} ids={ids} className={className}>
      <div className={cx(fieldBox({ invalid, disabled, readOnly: ro }), size === "lg" ? "h-46 gap-8 px-14" : "h-40 gap-8 pr-10 pl-12")}>
        <input
          id={ids.input}
          readOnly={ro}
          disabled={disabled}
          aria-invalid={invalid || undefined}
          aria-describedby={helper ? ids.helper : undefined}
          className={cx(
            "h-full min-w-0 flex-1 bg-transparent text-inherit outline-none placeholder:text-faint",
            mono ? "type-code" : "type-body",
          )}
          {...rest}
        />
        {suffix && <span className="type-mono-sm text-faint">{suffix}</span>}
        {(locked || disabled) && <Icon name="lock" size={13} tone="faint" />}
      </div>
    </FieldFrame>
  );
}
