import { useRef, useState, useCallback, useMemo, useEffect } from 'react'
import { forceSimulation, forceLink, forceManyBody, forceCenter, forceCollide, forceX, forceY, type SimulationNodeDatum } from 'd3-force'
import type { MemoryGraph as MemoryGraphType, MemoryVisibility } from '../interfaces/types'
import { FaPlus, FaMinus, FaCrosshairs, FaSliders } from 'react-icons/fa6'

interface GraphNode extends SimulationNodeDatum {
  slug: string
  title: string
  tags: string[]
  visibility: MemoryVisibility
  agent_id: string
}

interface TooltipData {
  node: GraphNode
  x: number
  y: number
}

interface MemoryGraphProps {
  graph: MemoryGraphType
  searchQuery: string
  onNodeClick: (slug: string) => void
}

const VISIBILITY_COLORS: Record<MemoryVisibility, string> = {
  private: '#6b7280',
  global: '#3b82f6',
  shared: '#f59e0b',
}

const COLOR_CONNECTED = '#10b981'
const COLOR_ORPHANED = '#6b7280'

function getNodeSize(slug: string, edges: { source: string; target: string }[]): number {
  const count = edges.filter((e) => e.source === slug || e.target === slug).length
  return Math.max(6, Math.min(18, 4 + count * 2))
}

export default function MemoryGraph({ graph, searchQuery, onNodeClick }: MemoryGraphProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const simulationRef = useRef<any>(null)
  const nodesRef = useRef<GraphNode[]>([])
  const linksRef = useRef<{ source: string; target: string; broken: boolean }[]>([])
  const transformRef = useRef({ x: 0, y: 0, k: 1 })
  const rafRef = useRef<number>(0)
  const dragRef = useRef<{ type: 'pan' | 'node'; startX: number; startY: number; startTx: number; startTy: number; node?: GraphNode; nodeStartX: number; nodeStartY: number; moved: boolean } | null>(null)
  const justDraggedRef = useRef(false)

  const [dimensions, setDimensions] = useState({ width: 800, height: 600 })
  const [tooltip, setTooltip] = useState<TooltipData | null>(null)
  const [showForceControls, setShowForceControls] = useState(false)
  const [chargeStrength, setChargeStrength] = useState(-400)
  const [linkDistance, setLinkDistance] = useState(120)
  const [orphanPull, setOrphanPull] = useState(0.3)

  const highlightedSlugs = useMemo(() => {
    if (!searchQuery.trim()) return null
    const q = searchQuery.toLowerCase()
    return new Set(
      graph.nodes
        .filter((n) => n.slug.toLowerCase().includes(q) || n.title.toLowerCase().includes(q) || n.tags.some((t) => t.toLowerCase().includes(q)))
        .map((n) => n.slug)
    )
  }, [graph.nodes, searchQuery])

  useEffect(() => {
    const container = containerRef.current
    if (!container) return
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setDimensions({ width: entry.contentRect.width, height: entry.contentRect.height })
      }
    })
    observer.observe(container)
    return () => observer.disconnect()
  }, [])

  const startSimulation = useCallback(() => {
    if (simulationRef.current) simulationRef.current.stop()

    const nodes: GraphNode[] = graph.nodes.map((n) => ({ ...n }))
    const nodeSlugs = new Set(nodes.map((n) => n.slug))
    const validLinks = graph.edges
      .filter((e) => nodeSlugs.has(e.source) && nodeSlugs.has(e.target))
      .map((e) => ({ source: e.source, target: e.target, broken: e.broken }))
    nodesRef.current = nodes
    linksRef.current = graph.edges.map((e) => ({ source: e.source, target: e.target, broken: e.broken }))

    const connectedSlugs = new Set<string>()
    for (const link of graph.edges) {
      if (!link.broken) {
        connectedSlugs.add(link.source)
        connectedSlugs.add(link.target)
      }
    }

    const sim = forceSimulation(nodes)
      .force('link', forceLink(validLinks as any).id((d: any) => d.slug).distance(linkDistance))
      .force('charge', forceManyBody().strength(chargeStrength))
      .force('center', forceCenter(dimensions.width / 2, dimensions.height / 2).strength(0.05))
      .force('collide', forceCollide().radius(30))
      .force('x', forceX<GraphNode>(dimensions.width / 2).strength((d) => connectedSlugs.has(d.slug) ? 0 : orphanPull))
      .force('y', forceY<GraphNode>(dimensions.height / 2).strength((d) => connectedSlugs.has(d.slug) ? 0 : orphanPull))
      .alphaDecay(0.02)
      .velocityDecay(0.3)

    sim.on('tick', () => draw())
    simulationRef.current = sim
  }, [graph, dimensions.width, dimensions.height, chargeStrength, linkDistance, orphanPull])

  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const { width, height } = dimensions
    const t = transformRef.current

    canvas.width = width * devicePixelRatio
    canvas.height = height * devicePixelRatio
    ctx.scale(devicePixelRatio, devicePixelRatio)

    ctx.clearRect(0, 0, width, height)
    ctx.save()
    ctx.translate(t.x, t.y)
    ctx.scale(t.k, t.k)

    const nodes = nodesRef.current
    const links = linksRef.current

    const connectedSlugs = new Set<string>()
    const brokenSlugs = new Set<string>()
    for (const link of links) {
      const srcSlug = typeof link.source === 'object' ? (link.source as any).slug : link.source
      const tgtSlug = typeof link.target === 'object' ? (link.target as any).slug : link.target
      if (!link.broken) {
        connectedSlugs.add(srcSlug)
        connectedSlugs.add(tgtSlug)
      } else {
        brokenSlugs.add(srcSlug)
      }
    }

    for (const link of links) {

      const src = nodes.find((n) => n.slug === link.source)
      const tgt = nodes.find((n) => n.slug === link.target)
      // console.log('>>', { link })
      if (!src || !tgt || src.x == null || src.y == null || tgt.x == null || tgt.y == null) continue

      const isMatch = highlightedSlugs === null || (highlightedSlugs.has(src.slug) && highlightedSlugs.has(tgt.slug))
      ctx.globalAlpha = isMatch ? 0.5 : 0.07
      ctx.strokeStyle = link.broken ? '#ef4444' : '#9ca3af'
      ctx.lineWidth = (link.broken ? 1.5 : 1) / t.k
      if (link.broken) ctx.setLineDash([4 / t.k, 4 / t.k])
      else ctx.setLineDash([])

      ctx.beginPath()
      ctx.moveTo(src.x, src.y)
      ctx.lineTo(tgt.x, tgt.y)
      ctx.stroke()
      ctx.setLineDash([])

      const angle = Math.atan2(tgt.y - src.y, tgt.x - src.x)
      const as = 6 / t.k
      const mx = (src.x + tgt.x) / 2
      const my = (src.y + tgt.y) / 2
      ctx.beginPath()
      ctx.moveTo(mx + as * Math.cos(angle), my + as * Math.sin(angle))
      ctx.lineTo(mx - as * Math.cos(angle - Math.PI / 6), my - as * Math.sin(angle - Math.PI / 6))
      ctx.lineTo(mx - as * Math.cos(angle + Math.PI / 6), my - as * Math.sin(angle + Math.PI / 6))
      ctx.closePath()
      ctx.fillStyle = link.broken ? '#ef4444' : '#9ca3af'
      ctx.fill()
    }

    for (const node of nodes) {
      if (node.x == null || node.y == null) continue
      const size = getNodeSize(node.slug, graph.edges)
      const isConnected = connectedSlugs.has(node.slug)
      const nodeColor = isConnected ? COLOR_CONNECTED : COLOR_ORPHANED
      const isMatch = highlightedSlugs === null || highlightedSlugs.has(node.slug)
      ctx.globalAlpha = isMatch ? 1 : 0.15

      ctx.fillStyle = nodeColor + '33'
      ctx.strokeStyle = nodeColor
      ctx.lineWidth = 1.5 / t.k

      if (node.visibility === 'global') {
        ctx.beginPath()
        ctx.arc(node.x, node.y, size, 0, 2 * Math.PI)
        ctx.fill()
        ctx.stroke()
      } else if (node.visibility === 'shared') {
        ctx.setLineDash([3 / t.k, 2 / t.k])
        ctx.beginPath()
        ctx.arc(node.x, node.y, size, 0, 2 * Math.PI)
        ctx.fill()
        ctx.stroke()
        ctx.setLineDash([])
      } else if (brokenSlugs.has(node.slug) && !connectedSlugs.has(node.slug)) {
        const arm = size * 0.7
        ctx.beginPath()
        ctx.moveTo(node.x - arm, node.y - arm)
        ctx.lineTo(node.x + arm, node.y + arm)
        ctx.moveTo(node.x + arm, node.y - arm)
        ctx.lineTo(node.x - arm, node.y + arm)
        ctx.stroke()
      } else {
        const half = size * 0.75
        const r = 3 / t.k
        ctx.beginPath()
        ctx.roundRect(node.x - half, node.y - half, half * 2, half * 2, r)
        ctx.fill()
        ctx.stroke()
      }

      if (highlightedSlugs !== null && highlightedSlugs.has(node.slug)) {
        ctx.beginPath()
        ctx.arc(node.x, node.y, size + 4, 0, 2 * Math.PI)
        ctx.strokeStyle = nodeColor
        ctx.lineWidth = 2 / t.k
        ctx.globalAlpha = 0.4
        ctx.stroke()
        ctx.globalAlpha = isMatch ? 1 : 0.15
      }

      const label = node.slug.length > 18 ? `${node.slug.slice(0, 16)}...` : node.slug
      ctx.font = `${10 / t.k}px IBM Plex Mono, monospace`
      ctx.textAlign = 'center'
      ctx.textBaseline = 'top'
      ctx.fillStyle = '#9ca3af'
      ctx.globalAlpha = isMatch ? 0.8 : 0.1
      ctx.fillText(label, node.x, node.y + size + 3 / t.k)
    }

    ctx.globalAlpha = 1
    ctx.restore()
  }, [dimensions, graph.edges, highlightedSlugs])

  useEffect(() => {
    startSimulation()
    return () => { simulationRef.current?.stop() }
  }, [startSimulation])

  useEffect(() => { draw() }, [draw])

  const screenToGraph = useCallback((sx: number, sy: number) => {
    const t = transformRef.current
    return { x: (sx - t.x) / t.k, y: (sy - t.y) / t.k }
  }, [])

  const findNodeAt = useCallback((sx: number, sy: number): GraphNode | null => {
    const { x, y } = screenToGraph(sx, sy)
    for (const node of nodesRef.current) {
      if (node.x == null || node.y == null) continue
      const size = getNodeSize(node.slug, graph.edges)
      if (Math.hypot(node.x - x, node.y - y) < size + 4) return node
    }
    return null
  }, [graph.edges, screenToGraph])

  const handleWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault()
    const rect = canvasRef.current!.getBoundingClientRect()
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    const t = transformRef.current
    const factor = e.deltaY > 0 ? 0.9 : 1.1
    const newK = Math.max(0.1, Math.min(5, t.k * factor))
    const ratio = newK / t.k
    t.x = mx - (mx - t.x) * ratio
    t.y = my - (my - t.y) * ratio
    t.k = newK
    draw()
  }, [draw])

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return
    const rect = canvasRef.current!.getBoundingClientRect()
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    const node = findNodeAt(mx, my)

    if (node) {
      node.fx = node.x
      node.fy = node.y
      dragRef.current = { type: 'node', startX: e.clientX, startY: e.clientY, startTx: 0, startTy: 0, node, nodeStartX: node.x!, nodeStartY: node.y!, moved: false }
    } else {
      const t = transformRef.current
      dragRef.current = { type: 'pan', startX: e.clientX, startY: e.clientY, startTx: t.x, startTy: t.y, nodeStartX: 0, nodeStartY: 0, moved: false }
    }
  }, [findNodeAt])

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const rect = canvasRef.current!.getBoundingClientRect()
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top

    if (dragRef.current) {
      const d = dragRef.current
      d.moved = true
      if (d.type === 'pan') {
        transformRef.current.x = d.startTx + (e.clientX - d.startX)
        transformRef.current.y = d.startTy + (e.clientY - d.startY)
      } else if (d.node) {
        const { x, y } = screenToGraph(mx, my)
        d.node.fx = x
        d.node.fy = y
        simulationRef.current?.alpha(0.1).restart()
      }
      draw()
      return
    }

    const node = findNodeAt(mx, my)
    if (node) {
      const outgoing = graph.edges.filter((e) => e.source === node.slug).length
      const incoming = graph.edges.filter((e) => e.target === node.slug).length
      setTooltip({ node, x: e.clientX, y: e.clientY })
      canvasRef.current!.style.cursor = 'pointer'
    } else {
      setTooltip(null)
      canvasRef.current!.style.cursor = 'grab'
    }
  }, [findNodeAt, draw, graph.edges, screenToGraph])

  const handleMouseUp = useCallback(() => {
    if (dragRef.current?.type === 'node' && dragRef.current.node) {
      dragRef.current.node.fx = null
      dragRef.current.node.fy = null
    }
    if (dragRef.current?.moved) {
      justDraggedRef.current = true
    }
    dragRef.current = null
  }, [])

  const handleClick = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (justDraggedRef.current) {
      justDraggedRef.current = false
      return
    }
    const rect = canvasRef.current!.getBoundingClientRect()
    const node = findNodeAt(e.clientX - rect.left, e.clientY - rect.top)
    if (node) onNodeClick(node.slug)
  }, [findNodeAt, onNodeClick])

  const zoomIn = useCallback(() => {
    const t = transformRef.current
    const cx = dimensions.width / 2
    const cy = dimensions.height / 2
    const newK = Math.min(5, t.k * 1.3)
    t.x = cx - (cx - t.x) * (newK / t.k)
    t.y = cy - (cy - t.y) * (newK / t.k)
    t.k = newK
    draw()
  }, [dimensions, draw])

  const zoomOut = useCallback(() => {
    const t = transformRef.current
    const cx = dimensions.width / 2
    const cy = dimensions.height / 2
    const newK = Math.max(0.1, t.k / 1.3)
    t.x = cx - (cx - t.x) * (newK / t.k)
    t.y = cy - (cy - t.y) * (newK / t.k)
    t.k = newK
    draw()
  }, [dimensions, draw])

  const resetView = useCallback(() => {
    transformRef.current = { x: 0, y: 0, k: 1 }
    draw()
  }, [draw])

  return (
    <div ref={containerRef} style={{ position: 'relative', width: '100%', height: '100%' }}>
      <canvas
        ref={canvasRef}
        width={dimensions.width}
        height={dimensions.height}
        style={{ width: dimensions.width, height: dimensions.height, background: 'var(--background)' }}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onClick={handleClick}
      />

      {tooltip && (
        <div
          style={{
            position: 'fixed',
            left: tooltip.x + 12,
            top: tooltip.y - 10,
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            padding: '12px',
            maxWidth: '280px',
            zIndex: 1000,
            boxShadow: 'var(--shadow-lg)',
            pointerEvents: 'none',
          }}
        >
          <div style={{ fontWeight: 600, marginBottom: '4px' }}>{tooltip.node.title}</div>
          <div style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-tertiary)', marginBottom: '6px' }}>
            {tooltip.node.slug}
          </div>
          <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginBottom: '6px' }}>
            <span style={{ fontSize: '10px', padding: '2px 6px', borderRadius: '4px', background: VISIBILITY_COLORS[tooltip.node.visibility] + '20', color: VISIBILITY_COLORS[tooltip.node.visibility], fontWeight: 500 }}>
              {tooltip.node.visibility}
            </span>
            {tooltip.node.tags.slice(0, 3).map((tag) => (
              <span key={tag} style={{ fontSize: '10px', padding: '2px 6px', borderRadius: '4px', background: 'var(--background)', color: 'var(--text-secondary)' }}>
                {tag}
              </span>
            ))}
          </div>
          <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
            {graph.edges.filter((e) => e.source === tooltip.node.slug).length} outgoing · {graph.edges.filter((e) => e.target === tooltip.node.slug).length} incoming
          </div>
        </div>
      )}

      <div style={{ position: 'absolute', bottom: '12px', left: '12px', display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '11px', color: 'var(--text-tertiary)', background: 'var(--surface)', padding: '8px 12px', borderRadius: '8px', border: '1px solid var(--border)' }}>
        <div style={{ fontWeight: 600, marginBottom: '2px' }}>Shapes</div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ width: '10px', height: '10px', borderRadius: '2px', border: 'solid 2px #9ca3af', background: '#9ca3af33' }} /> Private
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ width: '12px', height: '12px', borderRadius: '50%', border: 'solid 2px #9ca3af', background: '#9ca3af33' }} /> Global
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ width: '12px', height: '12px', borderRadius: '50%', border: 'dashed 2px #9ca3af', background: '#9ca3af33' }} /> Shared
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <svg width="12" height="12" viewBox="0 0 12 12"><line x1="2" y1="2" x2="10" y2="10" stroke="#9ca3af" strokeWidth="2" /><line x1="10" y1="2" x2="2" y2="10" stroke="#9ca3af" strokeWidth="2" /></svg> Broken
        </div>
        <div style={{ height: '1px', background: 'var(--border)', margin: '2px 0' }} />
        <div style={{ fontWeight: 600, marginBottom: '2px' }}>Color</div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ width: '10px', height: '10px', borderRadius: '2px', background: COLOR_CONNECTED }} /> Connected
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ width: '10px', height: '10px', borderRadius: '2px', background: COLOR_ORPHANED }} /> Orphaned
        </div>
      </div>

      <div style={{ position: 'absolute', bottom: '12px', right: '12px', display: 'flex', flexDirection: 'column', gap: '4px', background: 'var(--surface)', padding: '4px', borderRadius: '8px', border: '1px solid var(--border)' }}>
        <button className="btn btn-ghost" onClick={zoomIn} style={{ padding: '8px', minWidth: '32px', justifyContent: 'center' }} title="Zoom in">
          <FaPlus size={14} />
        </button>
        <button className="btn btn-ghost" onClick={zoomOut} style={{ padding: '8px', minWidth: '32px', justifyContent: 'center' }} title="Zoom out">
          <FaMinus size={14} />
        </button>
        <div style={{ height: '1px', background: 'var(--border)', margin: '0 4px' }} />
        <button className="btn btn-ghost" onClick={resetView} style={{ padding: '8px', minWidth: '32px', justifyContent: 'center' }} title="Reset view">
          <FaCrosshairs size={14} />
        </button>
        <div style={{ height: '1px', background: 'var(--border)', margin: '0 4px' }} />
        <button className="btn btn-ghost" onClick={() => setShowForceControls(!showForceControls)} style={{ padding: '8px', minWidth: '32px', justifyContent: 'center', color: showForceControls ? 'var(--accent-primary)' : undefined }} title="Force controls">
          <FaSliders size={14} />
        </button>
      </div>

      {showForceControls && (
        <div style={{ position: 'absolute', bottom: '12px', right: '60px', background: 'var(--surface)', padding: '12px', borderRadius: '8px', border: '1px solid var(--border)', width: '220px', fontSize: '11px', color: 'var(--text-secondary)' }}>
          <div style={{ fontWeight: 600, marginBottom: '8px', color: 'var(--text)' }}>Forces</div>
          <div style={{ marginBottom: '8px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2px' }}>
              <span>Repulsion</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>{chargeStrength}</span>
            </div>
            <input type="range" min="-1000" max="-50" step="10" value={chargeStrength} onChange={(e) => setChargeStrength(Number(e.target.value))} style={{ width: '100%' }} />
          </div>
          <div style={{ marginBottom: '8px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2px' }}>
              <span>Link Distance</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>{linkDistance}</span>
            </div>
            <input type="range" min="30" max="300" step="10" value={linkDistance} onChange={(e) => setLinkDistance(Number(e.target.value))} style={{ width: '100%' }} />
          </div>
          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2px' }}>
              <span>Orphan Pull</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>{orphanPull.toFixed(2)}</span>
            </div>
            <input type="range" min="0" max="1" step="0.05" value={orphanPull} onChange={(e) => setOrphanPull(Number(e.target.value))} style={{ width: '100%' }} />
          </div>
        </div>
      )}

      <div style={{ position: 'absolute', top: '12px', right: '12px', fontSize: '11px', color: 'var(--text-tertiary)', background: 'var(--surface)', padding: '4px 8px', borderRadius: '4px', border: '1px solid var(--border)' }}>
        {graph.nodes.length} nodes · {graph.edges.length} edges
      </div>
    </div>
  )
}
