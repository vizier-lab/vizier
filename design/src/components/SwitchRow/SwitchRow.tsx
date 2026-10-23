import { useId, type HTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Badge } from "../Badge/Badge";
import { Switch } from "../Switch/Switch";

export type SwitchRowProps = {
  label: ReactNode;
  /** A rank badge after the label, e.g. "granted". */
  badge?: ReactNode;
  /** Under the label; says why when disabled. */
  helper?: ReactNode;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
} & HTMLAttributes<HTMLDivElement>;

/** A label, optional badge and helper, with a switch at the end (X16). */
export function SwitchRow({ label, badge, helper, checked, onCheckedChange, disabled, className, ...rest }: SwitchRowProps) {
  const id = useId();
  return (
    <div className={cx("flex items-start justify-between gap-16", className)} {...rest}>
      <div className="flex min-w-0 flex-col gap-2">
        <span className="flex items-center gap-8">
          <span id={id} className="type-ui-medium text-fg">
            {label}
          </span>
          {badge && <Badge tone="rank">{badge}</Badge>}
        </span>
        {helper && (
          <span id={`${id}-helper`} className="type-small-para text-muted">
            {helper}
          </span>
        )}
      </div>
      <Switch
        checked={checked}
        onCheckedChange={onCheckedChange}
        disabled={disabled}
        aria-labelledby={id}
        aria-describedby={helper ? `${id}-helper` : undefined}
      />
    </div>
  );
}
