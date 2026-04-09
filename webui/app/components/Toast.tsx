import { useEffect } from 'react'
import { FiCheckCircle, FiAlertCircle, FiAlertTriangle, FiInfo } from 'react-icons/fi'
import { useToastStore, type Toast } from '../hooks/toastStore'
import { motion, AnimatePresence } from 'motion/react'

const toastIcons = {
  success: FiCheckCircle,
  error: FiAlertCircle,
  warning: FiAlertTriangle,
  info: FiInfo,
}

const toastColors = {
  success: 'border-emerald-500 bg-emerald-50 dark:bg-emerald-950/20 text-emerald-700 dark:text-emerald-400',
  error: 'border-red-500 bg-red-50 dark:bg-red-950/20 text-red-700 dark:text-red-400',
  warning: 'border-amber-500 bg-amber-50 dark:bg-amber-950/20 text-amber-700 dark:text-amber-400',
  info: 'border-blue-500 bg-blue-50 dark:bg-blue-950/20 text-blue-700 dark:text-blue-400',
}

function ToastItem({ toast }: { toast: Toast }) {
  const { removeToast } = useToastStore()
  const Icon = toastIcons[toast.type]

  return (
    <motion.div
      // initial={{ opacity: 0, y: -20, scale: 0.95 }}
      // animate={{ opacity: 1, y: 0, scale: 1 }}
      // exit={{ opacity: 0, y: -20, scale: 0.95 }}
      className={`flex items-start justify-between gap-3 p-2! pt-0! rounded-lg border shadow-lg ${toastColors[toast.type]}`}
    >
      <div className='flex gap-2 pt-2! pb-2!'>
        <Icon className="mt-1!" size={18} />
        <div className="flex-1">
          <p className="font-medium text-sm">{toast.message}</p>
          {toast.description && (
            <p className="text-xs mt-1 opacity-80">{toast.description}</p>
          )}
        </div>

      </div>

      <button
        onClick={() => removeToast(toast.id)}
        className="shrink-0 opacity-60 hover:opacity-100 transition-opacity"
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
