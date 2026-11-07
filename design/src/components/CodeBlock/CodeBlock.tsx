import { useState, type HTMLAttributes } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";

/**
 * What a token is, not what colour it is. The block maps a tone to a token of its own, so the
 * highlighter decides meaning and the design system decides palette (FR-015).
 */
export type CodeTone = "keyword" | "string" | "number" | "comment" | "function" | "punctuation";

/** A run of source text that shares one tone. `tone` left out means ordinary code colour. */
export type CodeToken = { text: string; tone?: CodeTone };

/** One line of a highlighted block: its tokens, in reading order. Empty is a blank line. */
export type CodeLine = CodeToken[];

// Code colours are their own tokens, so a scheme can change them without touching a status.
const TONE: Record<CodeTone, string> = {
  keyword: "text-code-keyword",
  string: "text-code-string",
  number: "text-code-number",
  comment: "text-code-comment",
  function: "text-code-function",
  punctuation: "text-code-punctuation",
};

export type CodeBlockProps = {
  code: string;
  /** Highlighted, one entry per line. Without it the source is drawn plain, as it always was. */
  tokens?: CodeLine[];
  /** Shown top left: "python", "diff". `diff` colours + and − lines. */
  language?: string;
  /** Shown instead of the language: "SKILL.md". */
  filename?: string;
  lineNumbers?: boolean;
  /** Called after a successful copy; the button says "Copied" for 2 s. */
  onCopy?: () => void;
} & HTMLAttributes<HTMLDivElement>;

/**
 * Fenced code with its language and a copy button (D41). Lines wrap instead of scrolling: an
 * agent's one-line script is common, and a horizontal scrollbar hides most of it (the activity
 * trail is where that hurt). The grid's code column is `minmax(0, 1fr)` so one unbroken token
 * can still shrink, and `break-words` breaks it rather than overflow.
 */
export function CodeBlock({ code, tokens, language, filename, lineNumbers, onCopy, className, ...rest }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const lines: CodeLine[] = tokens?.length
    ? tokens
    : code.replace(/\n$/, "").split("\n").map((line) => [{ text: line }]);
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
    <div className={cx("flex min-w-0 flex-col gap-10 rounded-md border border-border bg-surface px-16 pt-10 pb-14", className)} {...rest}>
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
      <pre className="overflow-hidden whitespace-pre-wrap break-words type-code-block text-soft">
        <code className="grid" style={{ gridTemplateColumns: lineNumbers ? "max-content minmax(0, 1fr)" : "minmax(0, 1fr)" }}>
          {lines.map((line, i) => {
            const text = line.map((token) => token.text).join("");
            const tone = diff && text.startsWith("+") ? "text-live-text" : diff && text.startsWith("-") ? "text-failed" : "";
            return (
              <span key={i} className="contents">
                {lineNumbers && (
                  <span aria-hidden className="pr-16 text-right text-faint select-none">
                    {i + 1}
                  </span>
                )}
                <span className={tone}>
                  {text === ""
                    ? " "
                    : line.map((token, j) =>
                        token.tone ? (
                          <span key={j} className={TONE[token.tone]}>
                            {token.text}
                          </span>
                        ) : (
                          token.text
                        )
                      )}
                </span>
              </span>
            );
          })}
        </code>
      </pre>
    </div>
  );
}
