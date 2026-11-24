import { useId, type HTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Switch } from "../Switch/Switch";

export type ToggleChipProps = {
  label: ReactNode;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
} & HTMLAttributes<HTMLDivElement>;

/** A bordered label with its switch (X15), for a row of small on/off tools. */
export function ToggleChip({ label, checked, onCheckedChange, disabled, className, ...rest }: ToggleChipProps) {
  const id = useId();
  return (
    <div className={cx("inline-flex h-38 items-center gap-10 rounded-md border border-border-strong bg-ground px-12", disabled && "opacity-40", className)} {...rest}>
      <span id={id} className="type-ui-medium text-fg">
        {label}
      </span>
      <Switch checked={checked} onCheckedChange={onCheckedChange} disabled={disabled} aria-labelledby={id} />
    </div>
  );
}
