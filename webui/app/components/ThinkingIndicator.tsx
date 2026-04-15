import { memo } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'

interface InlineEvent {
  id: string
  type: 'start' | 'tool_choice' | 'thinking'
  content?: string
  timestamp: number
}

interface ThinkingIndicatorProps {
  inlineEvents: InlineEvent[]
  agentName: string
}

function ThinkingIndicatorComponent({ inlineEvents, agentName }: ThinkingIndicatorProps) {
  if (inlineEvents.length === 0) {
    return null
  }

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    }}>
      <div style={{
        fontWeight: '600',
        fontSize: '14px',
        color: 'var(--accent-primary)',
      }}>
        {agentName}
      </div>
      <div style={{
        padding: '12px 16px',
        borderRadius: '8px',
        borderLeft: '3px solid var(--accent-primary)',
        display: 'flex',
        flexDirection: 'column',
        color: 'var(--text-secondary)',
        background: 'var(--surface)',
      }}>
        <div style={{
          display: 'flex',
          alignItems: 'center',
          color: 'var(--text-tertiary)',
        }}>
          thinking
          <div className="thinking-dots">
            <span>.</span>
            <span>.</span>
            <span>.</span>
          </div>
        </div>
        {inlineEvents.map((evt) => (
          <div key={evt.id} style={{
            display: 'flex',
            alignItems: 'flex-start',
            gap: '8px',
            fontSize: '14px',
          }}>
            {evt.type === 'tool_choice' && evt.content && (
              <div className="prose">
                <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                  {evt.content}
                </ReactMarkdown>
              </div>
            )}
            {evt.type === 'thinking' && evt.content && (
              <div className="prose">
                <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                  {evt.content.split('\n').map(line => `> ${line} `).join('\n')}
                </ReactMarkdown>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

// Memoize component to prevent re-renders when parent re-renders
// Only re-render if inlineEvents or agentName changes
export const ThinkingIndicator = memo(ThinkingIndicatorComponent)
