import {
  MDXEditor,
  headingsPlugin,
  listsPlugin,
  quotePlugin,
  thematicBreakPlugin,
  markdownShortcutPlugin,
  linkPlugin,
  codeBlockPlugin,
  toolbarPlugin,
  BoldItalicUnderlineToggles,
  CodeToggle,
  BlockTypeSelect,
  ListsToggle,
  InsertThematicBreak,
  Separator,
  ButtonWithTooltip,
} from '@mdxeditor/editor'
import type { MDXEditorMethods } from '@mdxeditor/editor'
import { useRef } from 'react'
import { FaPaperclip } from 'react-icons/fa6'

interface MarkdownEditorProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  disabled?: boolean
  className?: string
  onAttach?: () => void
}

export default function MarkdownEditor({
  value,
  onChange,
  placeholder,
  disabled,
  className,
  onAttach,
}: MarkdownEditorProps) {
  const editorRef = useRef<MDXEditorMethods>(null)

  return (
    <div className={`mdx-editor${className ? ` ${className}` : ''}`}>
      <MDXEditor
        ref={editorRef}
        className="mdx-editor-instance"
        markdown={value}
        onChange={onChange}
        placeholder={placeholder ?? 'Type something...'}
        readOnly={disabled}
        plugins={[
          headingsPlugin(),
          listsPlugin(),
          quotePlugin(),
          thematicBreakPlugin(),
          markdownShortcutPlugin(),
          linkPlugin(),
          codeBlockPlugin({ defaultCodeBlockLanguage: '' }),
          toolbarPlugin({
            toolbarClassName: 'mdx-editor-toolbar',
            toolbarContents: () => (
              <>
                {onAttach && (
                  <>
                    <ButtonWithTooltip
                      onClick={onAttach}
                      title="Attach file"
                    >
                      <FaPaperclip size={14} />
                    </ButtonWithTooltip>
                    <Separator />
                  </>
                )}
                <BoldItalicUnderlineToggles
                  options={['Bold', 'Italic']}
                />
                <Separator />
                <CodeToggle />
                <Separator />
                <BlockTypeSelect />
                <Separator />
                <ListsToggle
                  options={['bullet', 'number', 'check']}
                />
                <Separator />
                <InsertThematicBreak />
              </>
            ),
          }),
        ]}
      />
    </div>
  )
}
