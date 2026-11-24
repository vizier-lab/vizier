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
  /**
   * A fixed, non-editable tail the value is followed by on the wire — a request path, a domain,
   * a port. Drawn fenced at the field's right edge and never part of the value.
   */
  addon?: ReactNode;
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
  addon,
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
      {/* An addon reaches the box's right edge, so the box carries no right padding while one is
          there and clips its corners to the panel. */}
      <div
        className={cx(
          fieldBox({ invalid, disabled, readOnly: ro }),
          size === "lg" ? "h-46 gap-8 pl-14" : "h-40 gap-8 pl-12",
          addon ? "overflow-hidden pr-0" : size === "lg" ? "pr-14" : "pr-10",
        )}
      >
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
        {addon && (
          <span
            aria-hidden
            className="flex shrink-0 items-center self-stretch rounded-r-sm border-l border-border bg-surface px-12 type-mono-sm text-faint"
          >
            {addon}
          </span>
        )}
        {(locked || disabled) && <Icon name="lock" size={13} tone="faint" />}
      </div>
    </FieldFrame>
  );
}
