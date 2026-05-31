import { useState, useId } from 'react'
import {
  useFloating,
  autoUpdate,
  offset,
  flip,
  shift,
  FloatingPortal,
} from '@floating-ui/react'
import { FaCircleInfo } from 'react-icons/fa6'

interface TooltipLabelProps {
  label: string
  tooltip: string
}

export default function TooltipLabel({ label, tooltip }: TooltipLabelProps) {
  const [open, setOpen] = useState(false)
  const tooltipId = useId()

  const { refs, floatingStyles, context } = useFloating({
    open,
    placement: 'top',
    middleware: [offset(8), flip(), shift({ padding: 8 })],
    whileElementsMounted: autoUpdate,
  })

  return (
    <>
      {label}
      <span
        ref={refs.setReference}
        className="tooltip-icon"
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        aria-describedby={open ? tooltipId : undefined}
        style={{ display: 'inline-flex', alignItems: 'center' }}
      >
        <FaCircleInfo size={13} />
      </span>
      <FloatingPortal>
        {open && (
          <span
            id={tooltipId}
            ref={refs.setFloating}
            role="tooltip"
            className="tooltip-content"
            style={{
              ...floatingStyles,
              position: 'absolute',
              background: 'var(--sidebar-bg, #18181b)',
              color: '#fafafa',
              padding: '6px 10px',
              borderRadius: '6px',
              fontSize: '12px',
              fontWeight: 400,
              lineHeight: 1.4,
              whiteSpace: 'normal',
              width: 'max-content',
              maxWidth: '280px',
              zIndex: 1000,
              boxShadow: '0 4px 12px rgba(0, 0, 0, 0.15)',
              pointerEvents: 'none',
            }}
          >
            {tooltip}
          </span>
        )}
      </FloatingPortal>
    </>
  )
}
