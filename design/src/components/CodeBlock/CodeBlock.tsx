import { useState, type HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

export type CodeBlockProps = {
  code: string;
  /** Shown top left: "python", "diff". `diff` colours + and − lines. */
  language?: string;
  /** Shown instead of the language: "SKILL.md". */
  filename?: string;
  lineNumbers?: boolean;
  /** Called after a successful copy; the button says "Copied" for 2 s. */
  onCopy?: () => void;
} & HTMLAttributes<HTMLDivElement>;

/** Fenced code with its language and a copy button (D41). Highlighting is the caller's. */
export function CodeBlock({ code, language, filename, lineNumbers, onCopy, className, ...rest }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const lines = code.replace(/\n$/, "").split("\n");
  const diff = language === "diff";
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      onCopy?.();
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopied(false);
    }
  };
  return (
    <div className={cx("flex flex-col gap-10 rounded-md border border-border bg-surface px-16 pt-10 pb-14", className)} {...rest}>
      <div className="flex items-center justify-between gap-12">
        <span className="type-mono-sm text-faint">{filename ?? language}</span>
        <button
          type="button"
          onClick={copy}
          className={cx("flex items-center gap-4 rounded-xs type-small-medium", copied ? "text-fg" : "text-faint hover:text-fg")}
        >
          <Icon name={copied ? "check" : "copy"} size={13} />
          <span aria-live="polite">{copied ? "Copied" : "Copy"}</span>
        </button>
      </div>
      <pre className="overflow-x-auto type-code-block text-soft">
        <code className="grid" style={{ gridTemplateColumns: lineNumbers ? "max-content 1fr" : "1fr" }}>
          {lines.map((line, i) => {
            const tone = diff && line.startsWith("+") ? "text-live" : diff && line.startsWith("-") ? "text-failed" : "";
            return (
              <span key={i} className="contents">
                {lineNumbers && (
                  <span aria-hidden className="pr-16 text-right text-faint select-none">
                    {i + 1}
                  </span>
                )}
                <span className={tone}>{line || " "}</span>
              </span>
            );
          })}
        </code>
      </pre>
    </div>
  );
}
