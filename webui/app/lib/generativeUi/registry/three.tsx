// lint-tokens-exempt: light colours default to white, a physical value rather than a UI colour.
// The 3D parts of the catalogue. This is the one module that imports `three` and
// `@react-three/fiber`, and it is only ever loaded through `React.lazy` (see `scene3d.tsx`), so
// an interface without a `Scene3D` never pays for it. Parts are the children of `Scene3D`; the
// Canvas forwards the page's React contexts, so json-render works unchanged inside it.
import { Canvas, useFrame, useThree, type ThreeEvent } from '@react-three/fiber'
import type { ComponentRenderProps } from '@json-render/react'
import { useEffect, useMemo, useRef, type ReactNode } from 'react'
import * as THREE from 'three'
import { OrbitControls as OrbitControlsImpl } from 'three/examples/jsm/controls/OrbitControls.js'
import { resolvedColour } from './colours'
import { useEmitter } from './shared'
import { heightAt, heightfieldGeometry, type Stop } from './terrain'

type P = Record<string, unknown>
type Vec3 = [number, number, number]

const vec = (value: unknown, fallback: Vec3): Vec3 =>
    Array.isArray(value) && value.length === 3 && value.every((n) => typeof n === 'number' && Number.isFinite(n)) ? (value as Vec3) : fallback
const num = (value: unknown, fallback: number) => (typeof value === 'number' && Number.isFinite(value) ? value : fallback)

export function hasWebGL(): boolean {
    try {
        return !!document.createElement('canvas').getContext('webgl2')
    } catch {
        return false
    }
}

/** position, rotation and scale props as props of a three object. */
const placement = (p: P) => ({
    position: vec(p.position, [0, 0, 0]),
    rotation: vec(p.rotation, [0, 0, 0]),
    scale: vec(p.scale, [1, 1, 1]),
})

/** Presses on a part report its id (and, for terrain, where). */
function usePress(emit: ComponentRenderProps<P>['emit']) {
    const send = useEmitter(emit)
    return (event: ThreeEvent<MouseEvent>, payload: unknown) => {
        event.stopPropagation()
        send('press', event.nativeEvent, payload)
    }
}

function Material({ p }: { p: P }) {
    const color = useMemo(() => resolvedColour(p.color, 'accent'), [p.color])
    return <meshStandardMaterial color={color} flatShading={p.flat === true} wireframe={p.wireframe === true} />
}

export function Scene3DC({ element, children }: ComponentRenderProps<P>) {
    const p = element.props
    const camera = (p.camera ?? {}) as { position?: unknown; fov?: unknown; lookAt?: unknown }
    const background = useMemo(() => resolvedColour(p.background, 'surface'), [p.background])
    if (!hasWebGL()) {
        return (
            <p role="note" className="rounded-md border border-border bg-surface px-16 py-12 type-ui text-muted">
                3D is not available in this browser.
            </p>
        )
    }
    return (
        <div role="img" aria-label="3D view" style={{ height: num(p.height, 360), width: '100%', borderRadius: 'var(--vz-radius-md)', overflow: 'hidden' }}>
            <Canvas
                shadows={p.shadows === true}
                frameloop="demand"
                dpr={[1, 2]}
                camera={{ position: vec(camera.position, [6, 5, 6]), fov: num(camera.fov, 45) }}
                gl={{ antialias: true }}
            >
                <color attach="background" args={[background]} />
                {camera.lookAt !== undefined && <LookAt target={vec(camera.lookAt, [0, 0, 0])} />}
                {children as ReactNode}
            </Canvas>
        </div>
    )
}

function LookAt({ target }: { target: Vec3 }) {
    const { camera, invalidate } = useThree()
    useEffect(() => {
        camera.lookAt(...target)
        invalidate()
    }, [camera, invalidate, target])
    return null
}

export function Group3DC({ element, children }: ComponentRenderProps<P>) {
    return <group {...placement(element.props)}>{children as ReactNode}</group>
}

export function BoxC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const press = usePress(emit)
    return (
        <mesh {...placement(p)} castShadow receiveShadow onClick={(e) => press(e, { id: p.id })}>
            <boxGeometry args={vec(p.size, [1, 1, 1])} />
            <Material p={p} />
        </mesh>
    )
}

export function SphereC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const press = usePress(emit)
    const segments = Math.round(num(p.segments, 24))
    return (
        <mesh {...placement(p)} castShadow receiveShadow onClick={(e) => press(e, { id: p.id })}>
            <sphereGeometry args={[num(p.radius, 0.5), segments, Math.max(2, Math.round(segments / 2))]} />
            <Material p={p} />
        </mesh>
    )
}

export function CylinderC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const press = usePress(emit)
    const r = num(p.radius, 0.5)
    return (
        <mesh {...placement(p)} castShadow receiveShadow onClick={(e) => press(e, { id: p.id })}>
            <cylinderGeometry args={[r, r, num(p.height, 1), Math.round(num(p.segments, 24))]} />
            <Material p={p} />
        </mesh>
    )
}

export function PlaneC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const press = usePress(emit)
    const size = Array.isArray(p.size) && p.size.length === 2 ? (p.size as [number, number]) : [10, 10]
    return (
        // Lying flat unless the interface turns it.
        <mesh {...placement(p)} rotation={p.rotation ? vec(p.rotation, [0, 0, 0]) : [-Math.PI / 2, 0, 0]} receiveShadow onClick={(e) => press(e, { id: p.id })}>
            <planeGeometry args={[num(size[0], 10), num(size[1], 10)]} />
            <Material p={p} />
        </mesh>
    )
}

export function AmbientLightC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    return <ambientLight color={resolvedColour(p.color ?? '#ffffff', 'ink')} intensity={num(p.intensity, 0.5)} />
}

export function DirectionalLightC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    return <directionalLight color={resolvedColour(p.color ?? '#ffffff', 'ink')} intensity={num(p.intensity, 1)} position={vec(p.position, [5, 10, 5])} castShadow />
}

export function PointLightC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    return <pointLight color={resolvedColour(p.color ?? '#ffffff', 'ink')} intensity={num(p.intensity, 10)} position={vec(p.position, [0, 3, 0])} castShadow />
}

export function OrbitControlsC({ element }: ComponentRenderProps<P>) {
    const p = element.props
    const { camera, gl, invalidate } = useThree()
    const controls = useRef<OrbitControlsImpl | null>(null)
    const autoRotate = p.autoRotate === true
    const target = vec(p.target, [0, 0, 0])
    useEffect(() => {
        const c = new OrbitControlsImpl(camera, gl.domElement)
        c.enableDamping = true
        const onChange = () => invalidate()
        c.addEventListener('change', onChange)
        controls.current = c
        return () => {
            c.removeEventListener('change', onChange)
            c.dispose()
            controls.current = null
        }
    }, [camera, gl, invalidate])
    useEffect(() => {
        const c = controls.current
        if (!c) return
        c.enableZoom = p.enableZoom !== false
        c.autoRotate = autoRotate
        c.target.set(...target)
        c.update()
        invalidate()
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [p.enableZoom, autoRotate, target[0], target[1], target[2], invalidate])
    // Damping and auto-rotation need frames; a still view asks for none.
    useFrame(() => {
        const c = controls.current
        if (!c) return
        c.update()
        if (autoRotate || c.enableDamping) invalidate()
    })
    return null
}

function useBufferGeometry(positions: Float32Array, colors: Float32Array | null, indices: Uint32Array | null, flatNormals: boolean) {
    const geometry = useMemo(() => {
        const g = new THREE.BufferGeometry()
        g.setAttribute('position', new THREE.BufferAttribute(positions, 3))
        if (colors) g.setAttribute('color', new THREE.BufferAttribute(colors, 3))
        if (indices) g.setIndex(new THREE.BufferAttribute(indices, 1))
        g.computeVertexNormals()
        void flatNormals
        return g
    }, [positions, colors, indices, flatNormals])
    useEffect(() => () => geometry.dispose(), [geometry])
    return geometry
}

export function HeightfieldC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const press = usePress(emit)
    const size = (Array.isArray(p.size) ? p.size : [10, 10]) as [number, number]
    const resolution = Math.round(num(p.resolution, 2))
    const heights = (Array.isArray(p.heights) ? p.heights : []) as number[]
    const scale = num(p.heightScale, 1)
    const flat = p.flat !== false
    const built = useMemo(
        () => heightfieldGeometry({ size, resolution, heights, heightScale: scale, palette: p.palette as Stop[] | undefined, flat }),
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [size[0], size[1], resolution, heights, scale, p.palette, flat]
    )
    const geometry = useBufferGeometry(built.positions, built.colors, built.indices, flat)
    return (
        <mesh
            position={vec(p.position, [0, 0, 0])}
            geometry={geometry}
            castShadow
            receiveShadow
            onClick={(e) => {
                const local = e.object.worldToLocal(e.point.clone())
                press(e, { x: Math.round(local.x * 100) / 100, z: Math.round(local.z * 100) / 100, height: heightAt({ size, resolution, heights }, local.x, local.z) })
            }}
        >
            <meshStandardMaterial vertexColors flatShading={flat} wireframe={p.wireframe === true} side={THREE.DoubleSide} />
        </mesh>
    )
}

export function MeshC({ element, emit }: ComponentRenderProps<P>) {
    const p = element.props
    const press = usePress(emit)
    const vertices = useMemo(() => new Float32Array(Array.isArray(p.vertices) ? (p.vertices as number[]) : []), [p.vertices])
    const colors = useMemo(() => (Array.isArray(p.colors) && p.colors.length >= vertices.length ? new Float32Array(p.colors as number[]) : null), [p.colors, vertices])
    const indices = useMemo(() => (Array.isArray(p.indices) ? new Uint32Array(p.indices as number[]) : null), [p.indices])
    const geometry = useBufferGeometry(vertices, colors, indices, p.flat === true)
    return (
        <mesh {...placement(p)} geometry={geometry} castShadow receiveShadow onClick={(e) => press(e, { id: p.id })}>
            <meshStandardMaterial vertexColors={!!colors} flatShading={p.flat === true} side={THREE.DoubleSide} />
        </mesh>
    )
}
