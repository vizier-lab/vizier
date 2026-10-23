import { Markdown } from "@tiptap/markdown";
import { Table, TableCell, TableHeader, TableRow } from "@tiptap/extension-table";
import { EditorContent, useEditor, type Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { useEffect, useMemo, useRef, useState, type HTMLAttributes } from "react";
import { cx } from "../../lib/cx";
import { EditorToolbar, type EditorCommand, type EditorMode } from "../EditorToolbar/EditorToolbar";
import { OutlineNav } from "../OutlineNav/OutlineNav";
import { Prose } from "../Prose/Prose";
import { TABLE_TEMPLATE, headingsOf, offsetOfLine, prefixLines, wrapSelection } from "./outline";

export type MarkdownEditorProps = {
  /** The document, as markdown. */
  value: string;
  onChange: (markdown: string) => void;
  mode?: EditorMode;
  onModeChange?: (mode: EditorMode) => void;
  /** Core: a compact toolbar and a character count. The skill editor: every tool and the outline. */
  look?: "compact" | "full";
  /** Shows the headings beside the document (full look). */
  outline?: boolean;
  placeholder?: string;
  readOnly?: boolean;
  /** Ctrl or Cmd + S. */
  onSaveShortcut?: () => void;
} & Omit<HTMLAttributes<HTMLDivElement>, "onChange">;

const EXTENSIONS = [
  StarterKit.configure({ link: { openOnClick: false, autolink: false } }),
  Markdown,
  Table.configure({ resizable: false }),
  TableRow,
  TableHeader,
  TableCell,
];

/** The rich pane: Tiptap, with markdown in and out. */
function RichPane({
  value,
  onChange,
  editable,
  onEditor,
  className,
}: {
  value: string;
  onChange?: (markdown: string) => void;
  editable: boolean;
  onEditor?: (editor: Editor | null) => void;
  className?: string;
}) {
  const lastEmitted = useRef(value);
  const editor = useEditor({
    extensions: EXTENSIONS,
    content: value,
    contentType: "markdown",
    editable,
    immediatelyRender: false,
    editorProps: { attributes: { class: "outline-none min-h-160", "aria-label": "Document", role: "textbox", "aria-multiline": "true" } },
    onUpdate: ({ editor }) => {
      const md = editor.getMarkdown();
      lastEmitted.current = md;
      onChange?.(md);
    },
  });

  useEffect(() => onEditor?.(editor), [editor]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    editor?.setEditable(editable);
  }, [editor, editable]);

  // A value that came from elsewhere (a revision restored, the other pane) replaces the document.
  useEffect(() => {
    if (!editor || value === lastEmitted.current || value === editor.getMarkdown()) return;
    lastEmitted.current = value;
    editor.commands.setContent(value, { contentType: "markdown", emitUpdate: false });
  }, [editor, value]);

  return (
    <Prose size="document" className={className}>
      <EditorContent editor={editor} />
    </Prose>
  );
}

function blockLabel(editor: Editor | null): string {
  if (!editor) return "Paragraph";
  for (const level of [1, 2, 3, 4] as const) if (editor.isActive("heading", { level })) return `Heading ${level}`;
  if (editor.isActive("codeBlock")) return "Code block";
  if (editor.isActive("blockquote")) return "Quote";
  return "Paragraph";
}

/**
 * Edits a markdown document (X76): `rich` (WYSIWYG), `markdown` (the source) or `split` (source
 * with a live preview), under the library's own toolbar, outline and document typography.
 */
export function MarkdownEditor({
  value,
  onChange,
  mode: modeProp,
  onModeChange,
  look = "compact",
  outline,
  placeholder,
  readOnly,
  onSaveShortcut,
  className,
  onKeyDown,
  ...rest
}: MarkdownEditorProps) {
  const [modeState, setModeState] = useState<EditorMode>("rich");
  const mode = modeProp ?? modeState;
  const setMode = (m: EditorMode) => {
    setModeState(m);
    onModeChange?.(m);
  };
  const [editor, setEditor] = useState<Editor | null>(null);
  const [showOutline, setShowOutline] = useState(!!outline);
  const source = useRef<HTMLTextAreaElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const [activeHeading, setActiveHeading] = useState<string>();
  const headings = useMemo(() => headingsOf(value), [value]);

  // Tick the editor so the toolbar's active states follow the cursor.
  const [, tick] = useState(0);
  useEffect(() => {
    if (!editor) return;
    const on = () => tick((n) => n + 1);
    editor.on("selectionUpdate", on);
    editor.on("transaction", on);
    return () => {
      editor.off("selectionUpdate", on);
      editor.off("transaction", on);
    };
  }, [editor]);

  const active: EditorCommand[] = [];
  if (mode === "rich" && editor) {
    if (editor.isActive("bold")) active.push("bold");
    if (editor.isActive("italic")) active.push("italic");
    if (editor.isActive("code")) active.push("code");
    if (editor.isActive("link")) active.push("link");
    if (editor.isActive("bulletList")) active.push("bullets");
    if (editor.isActive("orderedList")) active.push("numbers");
  }

  const editSource = (apply: (text: string, start: number, end: number) => { text: string; start: number; end: number }) => {
    const el = source.current;
    if (!el) return;
    const r = apply(value, el.selectionStart, el.selectionEnd);
    onChange(r.text);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(r.start, r.end);
    });
  };

  const run = (cmd: EditorCommand) => {
    if (readOnly) return;
    if (cmd === "outline") return setShowOutline((s) => !s);
    if (mode === "rich" && editor) {
      const chain = editor.chain().focus();
      switch (cmd) {
        case "bold": return void chain.toggleBold().run();
        case "italic": return void chain.toggleItalic().run();
        case "code": return void chain.toggleCode().run();
        case "bullets": return void chain.toggleBulletList().run();
        case "numbers": return void chain.toggleOrderedList().run();
        case "table": return void chain.insertTable({ rows: 2, cols: 2, withHeaderRow: true }).run();
        case "link": {
          if (editor.isActive("link")) return void chain.unsetLink().run();
          const href = window.prompt("Link address");
          if (href) chain.setLink({ href }).run();
          return;
        }
      }
      return;
    }
    switch (cmd) {
      case "bold": return editSource((t, s, e) => wrapSelection(t, s, e, "**", "**", "bold"));
      case "italic": return editSource((t, s, e) => wrapSelection(t, s, e, "_", "_", "italic"));
      case "code": return editSource((t, s, e) => wrapSelection(t, s, e, "`", "`", "code"));
      case "link": return editSource((t, s, e) => wrapSelection(t, s, e, "[", "](https://)", "text"));
      case "bullets": return editSource((t, s, e) => prefixLines(t, s, e, () => "- "));
      case "numbers": return editSource((t, s, e) => prefixLines(t, s, e, (i) => `${i + 1}. `));
      case "table": return editSource((t, s, e) => ({ text: t.slice(0, s) + TABLE_TEMPLATE + t.slice(e), start: s, end: s + TABLE_TEMPLATE.length }));
    }
  };

  const jump = (id: string) => {
    setActiveHeading(id);
    const h = headings.find((x) => x.id === id);
    if (!h) return;
    if (mode === "rich" || mode === "split") {
      const els = body.current?.querySelectorAll<HTMLElement>(".ProseMirror h1, .ProseMirror h2, .ProseMirror h3");
      els?.[Number(id.slice(1))]?.scrollIntoView({ block: "start", behavior: "smooth" });
    }
    if (mode !== "rich" && source.current) {
      const at = offsetOfLine(value, h.line);
      source.current.focus();
      source.current.setSelectionRange(at, at);
    }
  };

  const textarea = (
    <textarea
      ref={source}
      aria-label="Markdown source"
      value={value}
      readOnly={readOnly}
      placeholder={placeholder}
      spellCheck={false}
      onChange={(e) => onChange(e.target.value)}
      className="min-h-240 w-full flex-1 resize-none bg-transparent p-20 type-code text-fg outline-none placeholder:text-faint"
    />
  );

  return (
    <div
      className={cx("flex min-h-0 flex-col overflow-hidden rounded-xl border border-border-strong bg-ground", className)}
      onKeyDown={(e) => {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s" && onSaveShortcut) {
          e.preventDefault();
          onSaveShortcut();
        }
        onKeyDown?.(e);
      }}
      {...rest}
    >
      <EditorToolbar
        look={look}
        blockType={mode === "rich" ? blockLabel(editor) : "Markdown"}
        onCommand={run}
        active={active}
        count={look === "compact" ? `${value.length.toLocaleString("en-US")} chars` : undefined}
        mode={look === "full" ? mode : undefined}
        onModeChange={look === "full" ? setMode : undefined}
      />
      <div className="flex min-h-0 flex-1">
        {look === "full" && showOutline && (
          <OutlineNav className="w-208 shrink-0 overflow-y-auto border-r border-border p-12" items={headings} activeId={activeHeading} onSelect={jump} />
        )}
        <div ref={body} className={cx("flex min-w-0 flex-1 overflow-y-auto", mode === "split" && "divide-x divide-border")}>
          {mode === "markdown" && textarea}
          {mode === "split" && (
            <>
              <div className="flex min-w-0 flex-1">{textarea}</div>
              <div className="min-w-0 flex-1 p-20">
                <RichPane value={value} editable={false} />
              </div>
            </>
          )}
          {mode === "rich" && (
            <div className="min-w-0 flex-1 p-20">
              <RichPane value={value} onChange={onChange} editable={!readOnly} onEditor={setEditor} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
