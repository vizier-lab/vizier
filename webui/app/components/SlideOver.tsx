import { useEffect, type ReactNode } from 'react'
import { motion, AnimatePresence } from 'motion/react'

interface SlideOverProps {
  open: boolean
  onClose: () => void
  title: string
  children: ReactNode
}

export default function SlideOver({ open, onClose, title, children }: SlideOverProps) {
  useEffect(() => {
    if (!open) return
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handleEsc)
    return () => document.removeEventListener('keydown', handleEsc)
  }, [open, onClose])

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            onClick={onClose}
            style={{
              position: 'fixed',
              inset: 0,
              background: 'rgba(0, 0, 0, 0.4)',
              backdropFilter: 'blur(4px)',
              zIndex: 1000,
            }}
          />
          <motion.div
            initial={{ x: '100%' }}
            animate={{ x: 0 }}
            exit={{ x: '100%' }}
            transition={{ type: 'spring', damping: 30, stiffness: 300 }}
            style={{
              position: 'fixed',
              top: 0,
              right: 0,
              bottom: 0,
              width: 'min(800px, 90vw)',
              background: 'var(--background)',
              borderLeft: '1px solid var(--border)',
              boxShadow: 'var(--shadow-xl)',
              zIndex: 1001,
              display: 'flex',
              flexDirection: 'column',
              overflowX: 'hidden'
            }}
          >
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              padding: '1rem 1.5rem',
              borderBottom: '1px solid var(--border)',
              flexShrink: 0,
            }}>
              <h2 style={{ margin: 0, fontSize: '1.1rem' }}>{title}</h2>
              <button className="btn btn-ghost" onClick={onClose} style={{ padding: '6px' }}>
                &#10005;
              </button>
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: '1.5rem', display: 'flex', flexDirection: 'column', height: '100%' }}>
              <div style={{ flex: 1, display: 'flex', flexDirection: 'column', height: '100%' }}>
                {children}
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  )
}
