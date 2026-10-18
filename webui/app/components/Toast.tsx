import { useEffect } from 'react'
import { FaCircleCheck, FaCircleExclamation, FaTriangleExclamation, FaCircleInfo } from 'react-icons/fa6'
import { useToastStore, type Toast as ToastType } from '../hooks/toastStore'
import { motion, AnimatePresence } from 'motion/react'

const toastIcons = {
  success: FaCircleCheck,
  error: FaCircleExclamation,
  warning: FaTriangleExclamation,
  info: FaCircleInfo,
}

// Neutral surface with a state-coloured accent, the way a chat bubble reads: only the
// left rule and the icon/title carry colour, so it is solid in both light and dark mode.
const toastAccent: Record<ToastType['type'], string> = {
  success: 'var(--accent-primary)',
  error: 'var(--color-error)',
  warning: '#f59e0b',
  info: '#3b82f6',
}

function ToastItem({ toast }: { toast: ToastType }) {
  const { removeToast } = useToastStore()
  const Icon = toastIcons[toast.type]
  const accent = toastAccent[toast.type]

  return (
    <motion.div
      className="flex items-start justify-between gap-3 p-3 rounded-lg shadow-lg"
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderLeft: `3px solid ${accent}`,
      }}
    >
      <div className="flex gap-2">
        <Icon size={18} style={{ color: accent, marginTop: 2, flex: 'none' }} />
        <div className="flex-1">
          <p className="font-medium text-sm" style={{ color: accent }}>
            {toast.message}
          </p>
          {toast.description && (
            <p className="text-xs mt-1" style={{ color: 'var(--text-tertiary)' }}>
              {toast.description}
            </p>
          )}
        </div>
      </div>

      <button
        onClick={() => removeToast(toast.id)}
        className="shrink-0 transition-opacity opacity-60 hover:opacity-100"
        style={{ color: 'var(--text-tertiary)' }}
      >
        ×
      </button>
    </motion.div>
  )
}

export default function ToastContainer() {
  const { toasts } = useToastStore()

  return (
    <div className="fixed top-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
      <AnimatePresence>
        {toasts.map((toast) => (
          <ToastItem key={toast.id} toast={toast} />
        ))}
      </AnimatePresence>
    </div>
  )
}
