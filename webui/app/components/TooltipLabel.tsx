import { FaCircleInfo } from 'react-icons/fa6'

interface TooltipLabelProps {
  label: string
  tooltip: string
}

export default function TooltipLabel({ label, tooltip }: TooltipLabelProps) {
  return (
    <span className="tooltip-wrapper">
      {label}
      <span className="tooltip-icon">
        <FaCircleInfo size={13} />
      </span>
      <span className="tooltip-content">{tooltip}</span>
    </span>
  )
}
