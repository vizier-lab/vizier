import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "../../lib/cx";
import { TextLink } from "../TextLink/TextLink";

export type SecretFieldProps = {
  /** "Token", "API key"; shown as a prefix inside the field. Omit inside a KeyValueList. */
  label?: ReactNode;
  /** The masked value as the server reports it: "••••7c1e". */
  masked: string;
  onReplace: () => void;
} & HTMLAttributes<HTMLDivElement>;

/** A saved secret: masked, never shown again, only replaceable (X14). */
export function SecretField({ label, masked, onReplace, className, ...rest }: SecretFieldProps) {
  if (!label) {
    return (
      <div className={cx("inline-flex items-center gap-12", className)} {...rest}>
        <span className="type-code text-soft">{masked}</span>
        <TextLink size="ui" onClick={onReplace}>Replace</TextLink>
      </div>
    );
  }
  return (
    <div className={cx("flex h-40 items-center gap-10 rounded-md border border-border-strong bg-ground pr-12 pl-12", className)} {...rest}>
      <span className="type-ui text-faint">{label}</span>
      <span className="min-w-0 flex-1 truncate type-code text-soft">{masked}</span>
      <TextLink size="ui" onClick={onReplace} aria-label={`Replace ${typeof label === "string" ? label : "secret"}`}>
        Replace
      </TextLink>
    </div>
  );
}
