import { useState, type HTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { Button } from "../Button/Button";

export type SecretRevealProps = {
  /** "Key “home-assistant” created. Copy it now: it won't be shown again." */
  title: ReactNode;
  secret: string;
} & HTMLAttributes<HTMLDivElement>;

/** A new secret, shown once, with Copy (X23). */
export function SecretReveal({ title, secret, className, ...rest }: SecretRevealProps) {
  const [copied, setCopied] = useState(false);
  return (
    <div role="status" className={cx("flex flex-col gap-10 rounded-lg border border-border-strong bg-raised px-16 py-16", className)} {...rest}>
      <p className="type-body-medium text-fg">{title}</p>
      <div className="flex items-center gap-10">
        <input
          readOnly
          value={secret}
          aria-label="New secret"
          onFocus={(e) => e.target.select()}
          className="h-40 min-w-0 flex-1 rounded-md border border-border-strong bg-ground px-12 type-code text-fg outline-none focus:border-live"
        />
        <Button
          icon={copied ? "check" : "copy"}
          onClick={async () => {
            try {
              await navigator.clipboard.writeText(secret);
              setCopied(true);
            } catch {
              setCopied(false);
            }
          }}
        >
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </div>
  );
}
