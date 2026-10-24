import { useId, type ReactNode } from "react";
import { cx } from "../../lib/cx";

/** Label, control and helper, shared by TextField and Textarea. */
export function useFieldIds(id?: string) {
  const auto = useId();
  const base = id ?? auto;
  return { input: base, helper: `${base}-helper` };
}

export function FieldFrame({
  label,
  helper,
  invalid,
  ids,
  className,
  children,
}: {
  label?: ReactNode;
  helper?: ReactNode;
  invalid?: boolean;
  ids: { input: string; helper: string };
  className?: string;
  children: ReactNode;
}) {
  return (
    <div className={cx("flex flex-col gap-6", className)}>
      {label && (
        <label htmlFor={ids.input} className="type-ui-medium text-soft">
          {label}
        </label>
      )}
      {children}
      {helper && (
        <p id={ids.helper} className={cx("type-small", invalid ? "text-failed-text" : "text-faint")}>
          {helper}
        </p>
      )}
    </div>
  );
}

/** The box around a text control: hover edge, emerald focus, error, disabled and read-only grounds. */
export function fieldBox({ invalid, disabled, readOnly }: { invalid?: boolean; disabled?: boolean; readOnly?: boolean }) {
  return cx(
    "flex w-full items-center rounded-md border transition-colors",
    disabled
      ? "border-border-strong bg-surface text-muted"
      : readOnly
        ? "border-border bg-surface text-soft"
        : invalid
          ? "border-failed bg-ground text-fg focus-within:border-failed"
          : "border-border-strong bg-ground text-fg hover:border-border-hover focus-within:border-live hover:focus-within:border-live",
  );
}
