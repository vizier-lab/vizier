import { useId, useState, type HTMLAttributes, type ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type KeywordInputProps = {
  label: ReactNode;
  value: string[];
  onChange: (value: string[]) => void;
  /** Placeholder while adding. */
  placeholder?: string;
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;

/** Keyword chips with ×, and a dashed "+ Add" that turns into a field (X18). Enter adds, Esc cancels. */
export function KeywordInput({ label, value, onChange, placeholder = "keyword", className, ...rest }: KeywordInputProps) {
  const id = useId();
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState("");
  const commit = () => {
    const k = draft.trim();
    if (k && !value.includes(k)) onChange([...value, k]);
    setDraft("");
    setAdding(false);
  };
  return (
    <div role="group" aria-labelledby={id} className={cx("flex flex-col gap-8", className)} {...rest}>
      <span id={id} className="type-small-medium text-muted">
        {label}
      </span>
      <ul className="flex flex-wrap items-center gap-6">
        {value.map((k) => (
          <li key={k} className="inline-flex h-23 items-center gap-6 rounded-sm bg-raised pr-6 pl-8">
            <span className="type-mono-sm text-fg">{k}</span>
            <button
              type="button"
              aria-label={`Remove ${k}`}
              onClick={() => onChange(value.filter((v) => v !== k))}
              className="flex size-14 items-center justify-center rounded-xs text-muted hover:text-fg"
            >
              <Icon name="close" size={11} />
            </button>
          </li>
        ))}
        <li>
          {adding ? (
            <input
              autoFocus
              value={draft}
              aria-label={`New ${typeof label === "string" ? label.toLowerCase() : "keyword"}`}
              placeholder={placeholder}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={commit}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  commit();
                } else if (e.key === "Escape") {
                  setDraft("");
                  setAdding(false);
                }
              }}
              className="h-25 w-120 rounded-sm border border-live bg-ground px-8 type-mono-sm text-fg outline-none placeholder:text-faint"
            />
          ) : (
            <button
              type="button"
              onClick={() => setAdding(true)}
              className="h-25 rounded-sm border border-dashed border-border-strong px-8 type-small-medium text-muted hover:border-border-hover hover:text-fg"
            >
              + Add
            </button>
          )}
        </li>
      </ul>
    </div>
  );
}
