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
import { useEffect, useRef } from 'react'
import { FaPaperclip } from 'react-icons/fa6'

interface EditorProps {
  value: string
  onChange: (value: string) => void
  onSubmit: () => void
  onAttach?: () => void
  placeholder?: string
  disabled?: boolean
}

export default function Editor({
  value,
  onChange,
  onSubmit,
  onAttach,
  placeholder,
  disabled,
}: EditorProps) {
  const editorRef = useRef<MDXEditorMethods>(null)
  const wrapperRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const wrapper = wrapperRef.current
    if (!wrapper) return

    let cleanup: (() => void) | null = null

    const attachListener = (el: HTMLElement) => {
      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
          const md = editorRef.current?.getMarkdown()
          if (!md?.trim()) return

          e.preventDefault()
          e.stopPropagation()
          onSubmit()
        }
      }

      el.addEventListener('keydown', handleKeyDown, true)
      cleanup = () => el.removeEventListener('keydown', handleKeyDown, true)
    }

    const contentEditable = wrapper.querySelector<HTMLElement>(
      '[contenteditable="true"]'
    )

    if (contentEditable) {
      attachListener(contentEditable)
    } else {
      const observer = new MutationObserver(() => {
        const el = wrapper.querySelector<HTMLElement>(
          '[contenteditable="true"]'
        )
        if (el) {
          observer.disconnect()
          attachListener(el)
        }
      })
      observer.observe(wrapper, { childList: true, subtree: true })
      cleanup = () => observer.disconnect()
    }

    return () => cleanup?.()
  }, [onSubmit])

  return (
    <div className="chat-mdx-editor" ref={wrapperRef}>
      <MDXEditor
        ref={editorRef}
        className="chat-mdx-editor-instance"
        markdown={value}
        onChange={onChange}
        placeholder={placeholder ?? 'Type a message...'}
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
            toolbarClassName: 'chat-editor-toolbar',
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
