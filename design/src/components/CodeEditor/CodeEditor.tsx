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
  /** A gutter of line numbers, one per line of the value however it wraps. */
  lineNumbers?: boolean;
} & Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "value" | "onChange" | "defaultValue">;

// Both layers must lay text out identically, or the caret drifts from the colours. `anywhere`
// (not `break-word`) also lowers the text's minimum width, so one long token wraps instead of
// widening the field and stopping every other line from wrapping.
const TEXT = "m-0 whitespace-pre-wrap [overflow-wrap:anywhere] type-code-block";

/**
 * A code field (D13 for code): `Textarea`'s frame with `CodeBlock`'s colours. The text is a real
 * textarea, transparent over a highlighted copy of itself, so typing, selection, undo and the
 * browser's own find all behave as in any field. It grows with its text instead of scrolling,
 * long lines wrap (line numbers count the value's lines, not the wrapped rows), and Tab indents
 * rather than leaving the field.
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
  lineNumbers,
  ...rest
}: CodeEditorProps) {
  const ids = useFieldIds(id);
  const ref = useRef<HTMLTextAreaElement>(null);
  const sourceLines = value.split("\n");
  const lines: CodeLine[] = tokens?.length ? [...tokens] : sourceLines.map((line) => [{ text: line }]);
  // A highlighter may drop a trailing newline; the drawn copy still needs that last line.
  while (lines.length < sourceLines.length) lines.push([]);
  // The gutter is as wide as the largest number, plus the space before the code.
  const gutter = lineNumbers ? `calc(${Math.max(2, String(lines.length).length)}ch + 16px)` : "0px";

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
        <div className="grid w-full min-w-0 grid-cols-[minmax(0,1fr)]" style={{ minHeight: `calc(${rows} * 1lh)` }}>
          <div
            aria-hidden
            className="pointer-events-none col-start-1 row-start-1 grid content-start text-soft"
            style={{ gridTemplateColumns: `${gutter} minmax(0, 1fr)` }}
          >
            {lines.map((line, i) => (
              <span key={i} className="contents">
                <span className="pr-16 text-right type-code-block text-faint select-none">{lineNumbers ? i + 1 : null}</span>
                <span className={TEXT}>
                  {line.length === 0
                    ? " "
                    : line.map((token, j) =>
                        token.tone ? (
                          <span key={j} className={CODE_TONE_CLASS[token.tone]}>
                            {token.text}
                          </span>
                        ) : (
                          token.text
                        )
                      )}
                </span>
              </span>
            ))}
          </div>
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
            style={{ marginLeft: gutter, width: `calc(100% - ${gutter})` }}
            className={cx(TEXT, "col-start-1 row-start-1 h-full min-w-0 resize-none overflow-hidden bg-transparent p-0 text-transparent caret-live outline-none selection:bg-live/25 selection:text-transparent")}
            {...rest}
          />
        </div>
      </div>
    </FieldFrame>
  );
}
