import { useRef, type KeyboardEvent, type ReactNode, type TextareaHTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { CODE_TONE_CLASS, type CodeLine } from "../CodeBlock/CodeBlock";
import { FieldFrame, fieldBox, useFieldIds } from "../TextField/Field";

export type CodeEditorProps = {
  label?: ReactNode;
  helper?: ReactNode;
  invalid?: boolean;
  value: string;
  onChange: (value: string) => void;
  /**
   * The value highlighted, one entry per line, as `CodeBlock` takes them: the caller owns the
   * grammar. Without it the code is drawn plain, as a mono `Textarea` would be.
   */
  tokens?: CodeLine[];
  /** The height before it grows with its text. */
  rows?: number;
  /** Spaces inserted by Tab. */
  tabSize?: number;
} & Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "value" | "onChange" | "defaultValue">;

// Both layers must lay text out identically, or the caret drifts from the colours.
const LAYER = "col-start-1 row-start-1 m-0 w-full whitespace-pre-wrap break-words type-code-block";

/**
 * A code field (D13 for code): `Textarea`'s frame with `CodeBlock`'s colours. The text is a real
 * textarea, transparent over a highlighted copy of itself, so typing, selection, undo and the
 * browser's own find all behave as in any field. It grows with its text instead of scrolling,
 * and Tab indents rather than leaving the field.
 */
export function CodeEditor({
  label,
  helper,
  invalid,
  value,
  onChange,
  tokens,
  rows = 8,
  tabSize = 4,
  disabled,
  readOnly,
  id,
  className,
  onKeyDown,
  ...rest
}: CodeEditorProps) {
  const ids = useFieldIds(id);
  const ref = useRef<HTMLTextAreaElement>(null);
  const lines: CodeLine[] = tokens?.length ? tokens : value.split("\n").map((line) => [{ text: line }]);

  const keyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    onKeyDown?.(e);
    if (e.defaultPrevented || e.key !== "Tab" || e.shiftKey || e.altKey || e.ctrlKey || e.metaKey || readOnly) return;
    e.preventDefault();
    const el = e.currentTarget;
    // execCommand keeps the edit on the browser's undo stack; setRangeText is the fallback.
    if (!document.execCommand?.("insertText", false, " ".repeat(tabSize))) {
      el.setRangeText(" ".repeat(tabSize), el.selectionStart, el.selectionEnd, "end");
      onChange(el.value);
    }
  };

  return (
    <FieldFrame label={label} helper={helper} invalid={invalid} ids={ids} className={className}>
      <div className={cx(fieldBox({ invalid, disabled, readOnly }), "items-stretch px-12 py-8")} onClick={() => ref.current?.focus()}>
        <div className="grid w-full min-w-0" style={{ minHeight: `calc(${rows} * 1lh)` }}>
          <pre aria-hidden className={cx(LAYER, "pointer-events-none text-soft")}>
            {lines.map((line, i) => (
              <span key={i}>
                {line.map((token, j) =>
                  token.tone ? (
                    <span key={j} className={CODE_TONE_CLASS[token.tone]}>
                      {token.text}
                    </span>
                  ) : (
                    token.text
                  )
                )}
                {i < lines.length - 1 ? "\n" : " "}
              </span>
            ))}
          </pre>
          <textarea
            ref={ref}
            id={ids.input}
            value={value}
            disabled={disabled}
            readOnly={readOnly}
            spellCheck={false}
            autoCapitalize="off"
            autoComplete="off"
            aria-invalid={invalid || undefined}
            aria-describedby={helper ? ids.helper : undefined}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={keyDown}
            className={cx(LAYER, "h-full resize-none overflow-hidden bg-transparent p-0 text-transparent caret-live outline-none selection:bg-live/25 selection:text-transparent")}
            {...rest}
          />
        </div>
      </div>
    </FieldFrame>
  );
}
