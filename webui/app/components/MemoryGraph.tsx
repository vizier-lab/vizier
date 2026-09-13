import { useRef, useState, useCallback, useMemo, useEffect } from 'react'
import { forceSimulation, forceLink, forceManyBody, forceCenter, forceCollide, forceX, forceY, type SimulationNodeDatum } from 'd3-force'
import type { MemoryGraph as MemoryGraphType } from '../interfaces/types'
import { FaPlus, FaMinus, FaCrosshairs, FaSliders, FaArrowRotateLeft, FaXmark } from 'react-icons/fa6'

interface GraphNode extends SimulationNodeDatum {
  slug: string
  bundle: string
  title: string
  tags: string[]
  agent_id: string
  boundary: boolean
}

interface TooltipData {
  node: GraphNode
  x: number
  y: number
}

interface MemoryGraphProps {
  graph: MemoryGraphType
  searchQuery: string
  onNodeClick: (node: { slug: string; bundle: string; boundary: boolean }) => void
}

const COLOR_BOUNDARY = '#3b82f6'
const COLOR_CONNECTED = '#10b981'
const COLOR_ORPHANED = '#6b7280'

const NODE_SIZE_CAP = 14

function getNodeSize(degree: number): number {
  return Math.min(NODE_SIZE_CAP, 4 + Math.sqrt(degree) * 1.5)
}

export default function MemoryGraph({ graph, searchQuery, onNodeClick }: MemoryGraphProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const simulationRef = useRef<any>(null)
  const linkForceRef = useRef<any>(null)
  const chargeForceRef = useRef<any>(null)
  const centerXForceRef = useRef<any>(null)
  const centerYForceRef = useRef<any>(null)
  const nodesRef = useRef<GraphNode[]>([])
  const linksRef = useRef<{ source: string; target: string; broken: boolean }[]>([])
  const transformRef = useRef({ x: 0, y: 0, k: 1 })
  const dragRef = useRef<{ type: 'pan' | 'node'; startX: number; startY: number; startTx: number; startTy: number; node?: GraphNode; nodeStartX: number; nodeStartY: number; moved: boolean } | null>(null)
  const justDraggedRef = useRef(false)
  const activeNodeSlugRef = useRef<string | null>(null)

  const [dimensions, setDimensions] = useState({ width: 800, height: 600 })
  const [tooltip, setTooltip] = useState<TooltipData | null>(null)
  const [showForceControls, setShowForceControls] = useState(false)
  const [chargeStrength, setChargeStrength] = useState(-150)
  const [linkDistance, setLinkDistance] = useState(60)
  const [centerPull, setCenterPull] = useState(0.05)

  const [visibleSlugs, setVisibleSlugs] = useState<Set<string>>(
    () => new Set(graph.initial_slugs)
  )

  const [selectedSlug, setSelectedSlug] = useState<string | null>(null)
  const selectedSlugRef = useRef<string | null>(null)
  selectedSlugRef.current = selectedSlug

  const selectedNode = useMemo(
    () => (selectedSlug ? graph.nodes.find((n) => n.slug === selectedSlug) ?? null : null),
    [selectedSlug, graph.nodes]
  )

  useEffect(() => {
    setVisibleSlugs(new Set(graph.initial_slugs))
    setSelectedSlug(null)
  }, [graph.initial_slugs])

  const degreeMap = useMemo(() => {
    const m = new Map<string, number>()
    for (const n of graph.nodes) m.set(n.slug, 0)
    for (const e of graph.edges) {
      m.set(e.source, (m.get(e.source) ?? 0) + 1)
      m.set(e.target, (m.get(e.target) ?? 0) + 1)
    }
    return m
  }, [graph.nodes, graph.edges])

  const nodeSizeMap = useMemo(() => {
    const m = new Map<string, number>()
    for (const [slug, deg] of degreeMap) m.set(slug, getNodeSize(deg))
    return m
  }, [degreeMap])

  const degreeOutInMap = useMemo(() => {
    const out = new Map<string, number>()
    const inc = new Map<string, number>()
    for (const e of graph.edges) {
      out.set(e.source, (out.get(e.source) ?? 0) + 1)
      inc.set(e.target, (inc.get(e.target) ?? 0) + 1)
    }
    return { out, inc }
  }, [graph.edges])

  const adjacency = useMemo(() => {
    const m = new Map<string, Set<string>>()
    for (const n of graph.nodes) m.set(n.slug, new Set())
    for (const e of graph.edges) {
      if (!m.has(e.source) || !m.has(e.target)) continue
      m.get(e.source)!.add(e.target)
      m.get(e.target)!.add(e.source)
    }
    return m
  }, [graph.nodes, graph.edges])

  const baseSlugs = useMemo(() => new Set(graph.initial_slugs), [graph.initial_slugs])

  const oneHopNeighbors = useCallback((slug: string): Set<string> => {
    const result = new Set<string>([slug])
    const neighbors = adjacency.get(slug)
    if (neighbors) {
      for (const n of neighbors) result.add(n)
    }
    return result
  }, [adjacency])

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

    const cx = dimensions.width / 2
    const cy = dimensions.height / 2
    const previousPositions = new Map<string, { x: number; y: number }>()
    for (const n of nodesRef.current) {
      if (n.x != null && n.y != null) previousPositions.set(n.slug, { x: n.x, y: n.y })
    }

    const visibleNodes: GraphNode[] = []
    for (const n of graph.nodes) {
      if (!visibleSlugs.has(n.slug)) continue
      const prev = previousPositions.get(n.slug)
      visibleNodes.push({
        ...n,
        x: prev?.x ?? cx + (Math.random() - 0.5) * 20,
        y: prev?.y ?? cy + (Math.random() - 0.5) * 20,
      })
    }

    const visibleLinks = graph.edges
      .filter((e) => visibleSlugs.has(e.source) && visibleSlugs.has(e.target))
      .map((e) => ({ source: e.source, target: e.target, broken: e.broken }))

    nodesRef.current = visibleNodes
    linksRef.current = graph.edges

    const linkForce = forceLink(visibleLinks as any).id((d: any) => d.slug).distance(linkDistance)
    const chargeForce = forceManyBody().strength(chargeStrength)
    const cxForce = forceX<GraphNode>(dimensions.width / 2).strength(centerPull)
    const cyForce = forceY<GraphNode>(dimensions.height / 2).strength(centerPull)
    linkForceRef.current = linkForce
    chargeForceRef.current = chargeForce
    centerXForceRef.current = cxForce
    centerYForceRef.current = cyForce

    const sim = forceSimulation(visibleNodes)
      .force('link', linkForce)
      .force('charge', chargeForce)
      .force('center', forceCenter(dimensions.width / 2, dimensions.height / 2).strength(0.05))
      .force('collide', forceCollide().radius(30))
      .force('x', cxForce)
      .force('y', cyForce)
      .alphaDecay(0.02)
      .velocityDecay(0.3)

    sim.on('tick', () => draw())

    simulationRef.current = sim
  }, [graph, dimensions.width, dimensions.height, linkDistance, chargeStrength, centerPull, visibleSlugs])

  const updateForces = useCallback(() => {
    const sim = simulationRef.current
    if (!sim) return
    chargeForceRef.current?.strength(chargeStrength)
    centerXForceRef.current?.x(dimensions.width / 2).strength(centerPull)
    centerYForceRef.current?.y(dimensions.height / 2).strength(centerPull)
    linkForceRef.current?.distance(linkDistance)
    sim.alpha(0.3).restart()
  }, [chargeStrength, linkDistance, centerPull, dimensions.width, dimensions.height])

  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const { width, height } = dimensions
    const t = transformRef.current
    const dpr = window.devicePixelRatio || 1
    const targetW = Math.round(width * dpr)
    const targetH = Math.round(height * dpr)
    if (canvas.width !== targetW || canvas.height !== targetH) {
      canvas.width = targetW
      canvas.height = targetH
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0)
    ctx.scale(dpr, dpr)
    ctx.clearRect(0, 0, width, height)
    ctx.save()
    ctx.translate(t.x, t.y)
    ctx.scale(t.k, t.k)

    const nodes = nodesRef.current
    const visible = visibleSlugs
    const allEdges = linksRef.current

    const connectedSlugs = new Set<string>()
    for (const link of allEdges) {
      if (!visible.has(link.source) || !visible.has(link.target)) continue
      if (!link.broken) {
        connectedSlugs.add(link.source)
        connectedSlugs.add(link.target)
      }
    }

    for (const link of allEdges) {
      if (!visible.has(link.source) || !visible.has(link.target)) continue
      const src = nodes.find((n) => n.slug === link.source)
      const tgt = nodes.find((n) => n.slug === link.target)
      if (!src || !tgt || src.x == null || src.y == null || tgt.x == null || tgt.y == null) continue

      const anyExpanded = !baseSlugs.has(src.slug) || !baseSlugs.has(tgt.slug)
      const bothMatch = highlightedSlugs !== null && highlightedSlugs.has(src.slug) && highlightedSlugs.has(tgt.slug)
      const isMatch = highlightedSlugs === null || bothMatch || anyExpanded
      const isActive = activeNodeSlugRef.current !== null && (src.slug === activeNodeSlugRef.current || tgt.slug === activeNodeSlugRef.current)
      ctx.globalAlpha = isMatch ? (isActive ? 1 : 0.5) : 0.07
      ctx.strokeStyle = isActive ? COLOR_CONNECTED : (link.broken ? '#ef4444' : '#9ca3af')
      ctx.lineWidth = ((isActive ? 2 : 1) * (link.broken ? 1.5 : 1)) / t.k
      if (link.broken) ctx.setLineDash([4 / t.k, 4 / t.k])
      else ctx.setLineDash([])

      const dx = tgt.x - src.x
      const dy = tgt.y - src.y
      const dist = Math.sqrt(dx * dx + dy * dy)
      const srcSize = nodeSizeMap.get(src.slug) ?? 4
      const tgtSize = nodeSizeMap.get(tgt.slug) ?? 4
      const angle = Math.atan2(tgt.y - src.y, tgt.x - src.x)
      if (dist > 0) {
        const nx = dx / dist
        const ny = dy / dist
        ctx.beginPath()
        ctx.moveTo(src.x + nx * srcSize, src.y + ny * srcSize)
        ctx.lineTo(tgt.x - nx * tgtSize, tgt.y - ny * tgtSize)
        ctx.stroke()
        ctx.setLineDash([])
        const hasReverse = allEdges.some((l) => {
          if (l === link) return false
          if (!visible.has(l.source) || !visible.has(l.target)) return false
          return l.source === tgt.slug && l.target === src.slug
        })
        const arrowOffset = hasReverse ? 0.15 : 0
        const mx = src.x + (tgt.x - src.x) * (0.5 + arrowOffset)
        const my = src.y + (tgt.y - src.y) * (0.5 + arrowOffset)
        const arrowSize = 16 / t.k
        const height = arrowSize * 0.866
        const tipX = mx + (arrowSize / 2) * Math.cos(angle)
        const tipY = my + (arrowSize / 2) * Math.sin(angle)
        const baseX = mx - (arrowSize / 2) * Math.cos(angle)
        const baseY = my - (arrowSize / 2) * Math.sin(angle)
        const perpX = -Math.sin(angle)
        const perpY = Math.cos(angle)
        const b1x = baseX + perpX * (height / 2)
        const b1y = baseY + perpY * (height / 2)
        const b2x = baseX - perpX * (height / 2)
        const b2y = baseY - perpY * (height / 2)
        ctx.beginPath()
        ctx.moveTo(tipX, tipY)
        ctx.lineTo(b1x, b1y)
        ctx.lineTo(b2x, b2y)
        ctx.closePath()
        ctx.fillStyle = isActive ? COLOR_CONNECTED : (link.broken ? '#ef4444' : '#9ca3af')
        ctx.fill()
      } else {
        ctx.beginPath()
        ctx.moveTo(src.x, src.y)
        ctx.lineTo(tgt.x, tgt.y)
        ctx.stroke()
        ctx.setLineDash([])
      }
    }

    for (const node of nodes) {
      if (node.x == null || node.y == null) continue
      const size = nodeSizeMap.get(node.slug) ?? 4
      const isConnected = connectedSlugs.has(node.slug)
      const nodeColor = isConnected ? COLOR_CONNECTED : COLOR_ORPHANED
      const isExpanded = !baseSlugs.has(node.slug)
      const matches = highlightedSlugs === null || highlightedSlugs.has(node.slug)
      const shouldDim = !matches && !isExpanded
      ctx.globalAlpha = shouldDim ? 0.15 : 1

      ctx.fillStyle = nodeColor + '33'
      ctx.strokeStyle = nodeColor
      ctx.lineWidth = 1.5 / t.k

      if (node.boundary) {
        ctx.beginPath()
        ctx.arc(node.x, node.y, size, 0, 2 * Math.PI)
        ctx.fill()
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
        ctx.globalAlpha = shouldDim ? 0.15 : 1
      }

      if (selectedSlugRef.current === node.slug) {
        ctx.beginPath()
        ctx.arc(node.x, node.y, size + 5, 0, 2 * Math.PI)
        ctx.strokeStyle = '#06b6d4'
        ctx.lineWidth = 2.5 / t.k
        ctx.globalAlpha = 1
        ctx.stroke()
        ctx.globalAlpha = shouldDim ? 0.15 : 1
      }

      const labelAlpha = Math.max(0, Math.min(1, (t.k - 0.6) / 0.4))
      if (labelAlpha > 0) {
        const label = node.slug
        ctx.font = `${10 / t.k}px IBM Plex Mono, monospace`
        ctx.textAlign = 'center'
        ctx.textBaseline = 'top'
        ctx.fillStyle = '#9ca3af'
        ctx.globalAlpha = (shouldDim ? 0.1 : 0.8) * labelAlpha
        ctx.fillText(label, node.x, node.y + size + 3 / t.k)
      }
    }

    ctx.globalAlpha = 1
    ctx.restore()
  }, [dimensions, highlightedSlugs, nodeSizeMap, visibleSlugs])

  useEffect(() => {
    startSimulation()
    return () => {
      simulationRef.current?.stop()
    }
  }, [startSimulation])

  useEffect(() => { updateForces() }, [updateForces])

  useEffect(() => { draw() }, [draw])

  const screenToGraph = useCallback((sx: number, sy: number) => {
    const t = transformRef.current
    return { x: (sx - t.x) / t.k, y: (sy - t.y) / t.k }
  }, [])

  const findNodeAt = useCallback((sx: number, sy: number): GraphNode | null => {
    const { x, y } = screenToGraph(sx, sy)
    const nodes = nodesRef.current
    for (let i = nodes.length - 1; i >= 0; i--) {
      const node = nodes[i]
      if (node.x == null || node.y == null) continue
      const size = nodeSizeMap.get(node.slug) ?? 4
      if (Math.hypot(node.x - x, node.y - y) < size + 4) return node
    }
    return null
  }, [screenToGraph, nodeSizeMap])

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
      activeNodeSlugRef.current = node.slug
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
      activeNodeSlugRef.current = node.slug
      setTooltip({ node, x: e.clientX, y: e.clientY })
      canvasRef.current!.style.cursor = 'pointer'
    } else {
      activeNodeSlugRef.current = null
      setTooltip(null)
      canvasRef.current!.style.cursor = 'grab'
    }
    draw()
  }, [findNodeAt, draw, screenToGraph])

  const handleMouseUp = useCallback(() => {
    if (dragRef.current?.type === 'node' && dragRef.current.node) {
      dragRef.current.node.fx = null
      dragRef.current.node.fy = null
    }
    if (dragRef.current?.moved) {
      justDraggedRef.current = true
    }
    activeNodeSlugRef.current = null
    dragRef.current = null
  }, [])

  const handleNodeActivate = useCallback((slug: string) => {
    const neighborhoodSet = oneHopNeighbors(slug)
    setVisibleSlugs((prev) => {
      const next = new Set(prev)
      for (const s of neighborhoodSet) next.add(s)
      return next
    })
    simulationRef.current?.alpha(0.3).restart()
  }, [oneHopNeighbors])

  const handleOpen = useCallback(() => {
    const slug = selectedSlugRef.current
    const node = slug ? graph.nodes.find((n) => n.slug === slug) : null
    if (node) onNodeClick(node)
  }, [onNodeClick, graph.nodes])

  const handleClose = useCallback(() => {
    setSelectedSlug(null)
    draw()
  }, [draw])

  const handleClick = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (justDraggedRef.current) {
      justDraggedRef.current = false
      return
    }
    const rect = canvasRef.current!.getBoundingClientRect()
    const node = findNodeAt(e.clientX - rect.left, e.clientY - rect.top)
    if (!node) {
      setSelectedSlug(null)
      draw()
      return
    }
    setSelectedSlug(node.slug)
    handleNodeActivate(node.slug)
    draw()
  }, [findNodeAt, handleNodeActivate, draw])

  const handleReset = useCallback(() => {
    setVisibleSlugs(new Set(graph.initial_slugs))
    setSelectedSlug(null)
    simulationRef.current?.alpha(0.4).restart()
  }, [graph.initial_slugs])

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
            {tooltip.node.boundary && (
              <span style={{ fontSize: '10px', padding: '2px 6px', borderRadius: '4px', background: COLOR_BOUNDARY + '20', color: COLOR_BOUNDARY, fontWeight: 500 }}>
                other bundle: {tooltip.node.bundle}
              </span>
            )}
            {tooltip.node.tags.slice(0, 3).map((tag) => (
              <span key={tag} style={{ fontSize: '10px', padding: '2px 6px', borderRadius: '4px', background: 'var(--background)', color: 'var(--text-secondary)' }}>
                {tag}
              </span>
            ))}
          </div>
          <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
            {degreeOutInMap.out.get(tooltip.node.slug) ?? 0} outgoing · {degreeOutInMap.inc.get(tooltip.node.slug) ?? 0} incoming
          </div>
        </div>
      )}

      <div style={{ position: 'absolute', bottom: '12px', left: '12px', display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '11px', color: 'var(--text-tertiary)', background: 'var(--surface)', padding: '8px 12px', borderRadius: '8px', border: '1px solid var(--border)' }}>
        <div style={{ fontWeight: 600, marginBottom: '2px' }}>Shapes</div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ width: '10px', height: '10px', borderRadius: '2px', border: 'solid 2px #9ca3af', background: '#9ca3af33' }} /> Node
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ width: '12px', height: '12px', borderRadius: '50%', border: 'solid 2px #9ca3af', background: '#9ca3af33' }} /> Bundle / boundary
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
        <button className="btn btn-ghost" onClick={handleReset} style={{ padding: '8px', minWidth: '32px', justifyContent: 'center' }} title="Reset to initial set">
          <FaArrowRotateLeft size={14} />
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
              <span>Center Pull</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>{centerPull.toFixed(2)}</span>
            </div>
            <input type="range" min="0" max="1" step="0.01" value={centerPull} onChange={(e) => setCenterPull(Number(e.target.value))} style={{ width: '100%' }} />
          </div>
        </div>
      )}

      {selectedNode ? (
        <div
          style={{
            position: 'absolute',
            top: '12px',
            right: '12px',
            width: '300px',
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            boxShadow: 'var(--shadow-lg)',
            padding: '12px',
            zIndex: 5,
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'flex-start', gap: '8px' }}>
            <div style={{ flex: 1, fontWeight: 600, fontSize: '13px', lineHeight: 1.3, wordBreak: 'break-word' }}>
              {selectedNode.title}
            </div>
            <button
              className="btn btn-ghost"
              onClick={handleClose}
              style={{ padding: '2px 6px', minWidth: 'auto', lineHeight: 1 }}
              title="Deselect"
            >
              <FaXmark size={12} />
            </button>
          </div>
          <div style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text-tertiary)' }}>
            {selectedNode.slug}
          </div>
          <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
            {selectedNode.boundary && (
              <span
                style={{
                  fontSize: '10px',
                  padding: '2px 6px',
                  borderRadius: '4px',
                  background: COLOR_BOUNDARY + '20',
                  color: COLOR_BOUNDARY,
                  fontWeight: 500,
                }}
              >
                other bundle: {selectedNode.bundle}
              </span>
            )}
            {selectedNode.tags.slice(0, 3).map((tag) => (
              <span
                key={tag}
                style={{
                  fontSize: '10px',
                  padding: '2px 6px',
                  borderRadius: '4px',
                  background: 'var(--background)',
                  color: 'var(--text-secondary)',
                }}
              >
                {tag}
              </span>
            ))}
            {selectedNode.tags.length > 3 && (
              <span style={{ fontSize: '10px', color: 'var(--text-tertiary)' }}>
                +{selectedNode.tags.length - 3}
              </span>
            )}
          </div>
          <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
            {degreeOutInMap.out.get(selectedNode.slug) ?? 0} outgoing · {degreeOutInMap.inc.get(selectedNode.slug) ?? 0} incoming
          </div>
          <div style={{ height: '1px', background: 'var(--border)' }} />
          <button
            className="btn btn-primary"
            onClick={handleOpen}
            style={{ justifyContent: 'center' }}
          >
            Open
          </button>
          <div style={{ fontSize: '10px', color: 'var(--text-tertiary)', textAlign: 'right' }}>
            {visibleSlugs.size} visible · {graph.nodes.length} total
          </div>
        </div>
      ) : (
        <div
          style={{
            position: 'absolute',
            top: '12px',
            right: '12px',
            fontSize: '11px',
            color: 'var(--text-tertiary)',
            background: 'var(--surface)',
            padding: '4px 8px',
            borderRadius: '4px',
            border: '1px solid var(--border)',
          }}
        >
          {visibleSlugs.size} visible · {graph.nodes.length} total
        </div>
      )}
    </div>
  )
}
