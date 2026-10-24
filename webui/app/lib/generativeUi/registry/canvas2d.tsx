import type { ComponentRenderProps } from '@json-render/react'
import { getToken } from '../../../api/client'
import { cssColour } from './colours'
import { fileUrl, findFile } from '../media'
import { useSessionFiles } from './mediaContext'
import { useEmitter } from './shared'

type P = Record<string, unknown>
type Shape = {
    id?: string
    kind: 'rect' | 'circle' | 'line' | 'path' | 'text' | 'image'
    file?: string
    x?: number
    y?: number
    w?: number
    h?: number
    r?: number
    x1?: number
    y1?: number
    x2?: number
    y2?: number
    d?: string
    fill?: string
    stroke?: string
    strokeWidth?: number
    text?: string
    size?: number
}

const num = (value: unknown, fallback = 0) => (typeof value === 'number' && Number.isFinite(value) ? value : fallback)

/** One shape as SVG; unknown geometry draws nothing rather than failing the page. */
function ShapeSvg({ shape }: { shape: Shape }) {
    const files = useSessionFiles()
    const paint = {
        fill: shape.kind === 'line' ? 'none' : shape.fill ? cssColour(shape.fill) : 'none',
        stroke: shape.stroke ? cssColour(shape.stroke) : shape.kind === 'line' ? cssColour('ink') : 'none',
        strokeWidth: num(shape.strokeWidth, shape.stroke || shape.kind === 'line' ? 1 : 0),
    }
    switch (shape.kind) {
        case 'rect':
            return <rect x={num(shape.x)} y={num(shape.y)} width={num(shape.w)} height={num(shape.h)} {...paint} />
        case 'circle':
            return <circle cx={num(shape.x)} cy={num(shape.y)} r={num(shape.r)} {...paint} />
        case 'line':
            return <line x1={num(shape.x1)} y1={num(shape.y1)} x2={num(shape.x2)} y2={num(shape.y2)} {...paint} />
        case 'image': {
            const file = typeof shape.file === 'string' ? findFile(files, shape.file) : undefined
            // A missing file draws nothing in its place; the rest of the drawing stays.
            return file ? <image href={fileUrl(file, getToken())} x={num(shape.x)} y={num(shape.y)} width={num(shape.w)} height={num(shape.h)} preserveAspectRatio="xMidYMid meet" /> : null
        }
        case 'path':
            return typeof shape.d === 'string' ? <path d={shape.d} {...paint} /> : null
        case 'text':
            return (
                <text x={num(shape.x)} y={num(shape.y)} fontSize={num(shape.size, 12)} fill={cssColour(shape.fill ?? 'ink')} style={{ userSelect: 'none' }}>
                    {shape.text}
                </text>
            )
        default:
            return null
    }
}

export function Canvas2DC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const send = useEmitter(emit)
    const width = num(p.width, 100)
    const height = num(p.height, 100)
    const shapes = (Array.isArray(p.shapes) ? p.shapes : []) as Shape[]

    // Pointer positions are reported in the drawing's own units, whatever size it is scaled to.
    const locate = (event: React.PointerEvent<SVGSVGElement>) => {
        const box = event.currentTarget.getBoundingClientRect()
        return {
            x: Math.round(((event.clientX - box.left) / box.width) * width * 100) / 100,
            y: Math.round(((event.clientY - box.top) / box.height) * height * 100) / 100,
        }
    }

    let lastMove = 0
    return (
        <svg
            role="img"
            aria-label={typeof p.label === 'string' ? p.label : 'Drawing'}
            viewBox={`0 0 ${width} ${height}`}
            style={{ width: '100%', height: 'auto', background: p.background ? cssColour(p.background, 'surface') : 'transparent', touchAction: 'manipulation' }}
            onPointerDown={(e) => {
                const hit = (e.target as SVGElement).closest('[data-shape-id]')?.getAttribute('data-shape-id') ?? undefined
                send('press', e.nativeEvent, { ...locate(e), ...(hit ? { id: hit } : {}) })
            }}
            onPointerMove={(e) => {
                const now = Date.now()
                if (now - lastMove < 66) return
                lastMove = now
                send('move', e.nativeEvent, locate(e))
            }}
        >
            {shapes.map((shape, i) => (
                <g key={shape.id ?? i} data-shape-id={shape.id}>
                    <ShapeSvg shape={shape} />
                </g>
            ))}
        </svg>
    )
}
