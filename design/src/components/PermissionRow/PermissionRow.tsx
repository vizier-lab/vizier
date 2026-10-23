import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Checkbox } from "../Checkbox/Checkbox";

export type PermissionRowProps = {
  label: ReactNode;
  /** "all_agents:create". */
  permission: string;
  granted: boolean;
  onGrantedChange: (granted: boolean) => void;
  /** Built-in roles can't be edited. */
  disabled?: boolean;
} & Omit<HTMLAttributes<HTMLLabelElement>, "onChange">;

/** One permission in a role (X57): checkbox, what it allows, and its key. */
export function PermissionRow({ label, permission, granted, onGrantedChange, disabled, className, ...rest }: PermissionRowProps) {
  return (
    <label className={cx("flex items-start gap-12 py-6", disabled ? "[&>span:last-child]:opacity-40" : "cursor-pointer", className)} {...rest}>
      <Checkbox checked={granted} disabled={disabled} onChange={(e) => onGrantedChange(e.target.checked)} className="mt-2" />
      <span className="flex flex-col gap-2">
        <span className="type-ui text-fg">{label}</span>
        <span className="type-mono-xs text-faint">{permission}</span>
      </span>
    </label>
  );
}

export type PermissionGroupProps = { title: ReactNode; children: ReactNode } & HTMLAttributes<HTMLFieldSetElement>;

/** A heading over a group of PermissionRows. */
export function PermissionGroup({ title, children, className, ...rest }: PermissionGroupProps) {
  return (
    <fieldset className={cx("flex flex-col gap-2 border-0 p-0", className)} {...rest}>
      <legend className="pb-6 type-ui-strong text-fg">{title}</legend>
      {children}
    </fieldset>
  );
}
