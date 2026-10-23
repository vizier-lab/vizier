import { useId, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { TextLink } from "../TextLink/TextLink";

export type KeyValuePair = { key: string; value: string };

export type KeyValueEditorProps = {
  label: ReactNode;
  value: KeyValuePair[];
  onChange: (value: KeyValuePair[]) => void;
  /** Mask values (environment secrets). */
  secret?: boolean;
  addLabel?: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;

const input =
  "h-38 min-w-0 rounded-md border border-border-strong bg-ground px-12 type-code text-fg outline-none placeholder:text-faint hover:border-border-hover focus:border-live";

/** Rows of key and value, with "+ Add variable" (X20). */
export function KeyValueEditor({ label, value, onChange, secret = true, addLabel = "+ Add variable", className, ...rest }: KeyValueEditorProps) {
  const id = useId();
  const set = (i: number, patch: Partial<KeyValuePair>) => onChange(value.map((p, j) => (j === i ? { ...p, ...patch } : p)));
  return (
    <div role="group" aria-labelledby={id} className={cx("flex flex-col gap-6", className)} {...rest}>
      <span id={id} className="type-ui-medium text-soft">
        {label}
      </span>
      {value.map((p, i) => (
        <div key={i} className="grid grid-cols-[5fr_6fr_auto] items-center gap-8">
          <input aria-label={`Name ${i + 1}`} value={p.key} placeholder="NAME" onChange={(e) => set(i, { key: e.target.value })} className={input} />
          <input
            aria-label={`Value ${i + 1}`}
            type={secret ? "password" : "text"}
            value={p.value}
            placeholder="value"
            onChange={(e) => set(i, { value: e.target.value })}
            className={input}
          />
          <button
            type="button"
            aria-label={`Remove ${p.key || `row ${i + 1}`}`}
            onClick={() => onChange(value.filter((_, j) => j !== i))}
            className="flex size-26 items-center justify-center rounded-sm text-muted hover:bg-raised hover:text-fg"
          >
            <Icon name="close" size={13} />
          </button>
        </div>
      ))}
      <div>
        <TextLink onClick={() => onChange([...value, { key: "", value: "" }])}>{addLabel}</TextLink>
      </div>
    </div>
  );
}
