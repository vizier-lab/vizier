import type { PythonSandboxConfig } from '../interfaces/types'
import { DEFAULT_PYTHON_SANDBOX } from '../interfaces/types'

const rowStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '0.4rem',
  fontSize: '0.8rem',
  cursor: 'pointer',
}

/** The two Python switches. Code mode requires the sandbox, so turning the
 *  sandbox off turns code mode off in the same change. */
export default function PythonSandboxSection({
  value,
  onChange,
}: {
  value?: Partial<PythonSandboxConfig>
  onChange: (value: PythonSandboxConfig) => void
}) {
  const { enabled, code_mode } = { ...DEFAULT_PYTHON_SANDBOX, ...value }
  const set = (nextEnabled: boolean, nextCodeMode: boolean) =>
    onChange({ enabled: nextEnabled, code_mode: nextEnabled && nextCodeMode })

  return (
    <div>
      <h4
        style={{
          fontSize: '0.85rem',
          fontWeight: 600,
          color: 'var(--text-primary)',
          marginBottom: '0.75rem',
          paddingBottom: '0.5rem',
          borderBottom: '1px solid var(--border)',
        }}
      >
        Python
      </h4>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
        <label style={rowStyle}>
          <input type="checkbox" checked={enabled} onChange={(e) => set(e.target.checked, code_mode)} />
          Python sandbox
        </label>
        <p style={{ fontSize: '0.75rem', color: 'var(--text-tertiary)', margin: 0 }}>
          Lets the agent run Python scripts in an isolated sandbox for exact computation. No filesystem,
          network or tool access. Scripts are bounded by this agent's tool timeout. There is no
          separate memory or output limit.
        </p>
        <label
          title={enabled ? undefined : 'Turn on the Python sandbox first'}
          style={{ ...rowStyle, cursor: enabled ? 'pointer' : 'not-allowed', opacity: enabled ? 1 : 0.5 }}
        >
          <input
            type="checkbox"
            disabled={!enabled}
            checked={code_mode}
            onChange={(e) => set(true, e.target.checked)}
          />
          Code mode (programmatic tool calling)
        </label>
        {code_mode && (
          <div
            role="note"
            style={{
              fontSize: '0.75rem',
              padding: '0.5rem 0.75rem',
              borderRadius: '0.375rem',
              border: '1px solid var(--warning, #d97706)',
              color: 'var(--text-secondary)',
              background: 'color-mix(in srgb, var(--warning, #d97706) 10%, transparent)',
            }}
          >
            While code mode is on, this agent's other tools are hidden from the model and reachable only
            from scripts. The model sees just <code>execute_python</code>, <code>think</code>, and two
            documentation tools.
          </div>
        )}
      </div>
    </div>
  )
}
