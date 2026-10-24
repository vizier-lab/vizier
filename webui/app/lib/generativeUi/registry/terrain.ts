// lint-tokens-exempt: terrain stops are hex colours the palette prop takes; the default ramp is terrain-specific.
// Height-field geometry as plain arrays: no WebGL and no `three` here, so it is tested in Node and
// the 3D module only wraps it in a BufferGeometry. `flat` gives the low-poly look: every triangle
// has its own vertices and one colour from its average height, so facets show.

export type Stop = { at: number; color: string }
export type Heightfield = { size: [number, number]; resolution: number; heights: number[]; heightScale?: number; palette?: Stop[]; flat?: boolean }
export type Geometry = { positions: Float32Array; colors: Float32Array; indices: Uint32Array | null }

const DEFAULT_PALETTE: Stop[] = [
    { at: 0, color: '#2b5d7a' },
    { at: 0.3, color: '#4f8f4a' },
    { at: 0.7, color: '#8a7d63' },
    { at: 1, color: '#f2f4f7' },
]

/** `#rrggbb` as 0–1 channels; anything else is grey. */
export function rgb(color: string): [number, number, number] {
    const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(color)
    return m ? [parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255] : [0.5, 0.5, 0.5]
}

/** The palette's colour at height `t` (0–1): linear between the surrounding stops. */
export function colourAt(palette: Stop[], t: number): [number, number, number] {
    const stops = [...palette].sort((a, b) => a.at - b.at)
    const x = Math.min(1, Math.max(0, Number.isFinite(t) ? t : 0))
    if (x <= stops[0].at) return rgb(stops[0].color)
    for (let i = 1; i < stops.length; i++) {
        if (x <= stops[i].at) {
            const [a, b] = [stops[i - 1], stops[i]]
            const f = b.at === a.at ? 1 : (x - a.at) / (b.at - a.at)
            const [c1, c2] = [rgb(a.color), rgb(b.color)]
            return [c1[0] + (c2[0] - c1[0]) * f, c1[1] + (c2[1] - c1[1]) * f, c1[2] + (c2[2] - c1[2]) * f]
        }
    }
    return rgb(stops[stops.length - 1].color)
}

export function heightfieldGeometry(field: Heightfield): Geometry {
    const n = Math.max(2, Math.min(192, Math.floor(field.resolution)))
    const scale = field.heightScale ?? 1
    const palette = field.palette?.length ? field.palette : DEFAULT_PALETTE
    const height = (ix: number, iz: number) => {
        const h = field.heights[iz * n + ix]
        return typeof h === 'number' && Number.isFinite(h) ? h : 0
    }
    const px = (ix: number) => (ix / (n - 1) - 0.5) * field.size[0]
    const pz = (iz: number) => (iz / (n - 1) - 0.5) * field.size[1]

    if (!field.flat && field.flat !== undefined) {
        // Smooth: shared vertices, a colour each.
        const positions = new Float32Array(n * n * 3)
        const colors = new Float32Array(n * n * 3)
        for (let iz = 0; iz < n; iz++) {
            for (let ix = 0; ix < n; ix++) {
                const i = (iz * n + ix) * 3
                const h = height(ix, iz)
                positions.set([px(ix), h * scale, pz(iz)], i)
                colors.set(colourAt(palette, h), i)
            }
        }
        const indices = new Uint32Array((n - 1) * (n - 1) * 6)
        let k = 0
        for (let iz = 0; iz < n - 1; iz++) {
            for (let ix = 0; ix < n - 1; ix++) {
                const a = iz * n + ix
                const b = a + 1
                const c = a + n
                const d = c + 1
                indices.set([a, c, b, b, c, d], k)
                k += 6
            }
        }
        return { positions, colors, indices }
    }

    // Flat: each cell is two triangles with their own vertices and a face colour.
    const triangles = (n - 1) * (n - 1) * 2
    const positions = new Float32Array(triangles * 9)
    const colors = new Float32Array(triangles * 9)
    let t = 0
    const tri = (a: [number, number], b: [number, number], c: [number, number]) => {
        const hs = [a, b, c].map(([ix, iz]) => height(ix, iz))
        const colour = colourAt(palette, (hs[0] + hs[1] + hs[2]) / 3)
        ;[a, b, c].forEach(([ix, iz], v) => {
            positions.set([px(ix), hs[v] * scale, pz(iz)], t * 9 + v * 3)
            colors.set(colour, t * 9 + v * 3)
        })
        t++
    }
    for (let iz = 0; iz < n - 1; iz++) {
        for (let ix = 0; ix < n - 1; ix++) {
            tri([ix, iz], [ix, iz + 1], [ix + 1, iz])
            tri([ix + 1, iz], [ix, iz + 1], [ix + 1, iz + 1])
        }
    }
    return { positions, colors, indices: null }
}

/** The height (0–1, before scaling) under a local point, for a press report. */
export function heightAt(field: Pick<Heightfield, 'size' | 'resolution' | 'heights'>, x: number, z: number): number {
    const n = Math.max(2, Math.floor(field.resolution))
    const ix = Math.round((x / field.size[0] + 0.5) * (n - 1))
    const iz = Math.round((z / field.size[1] + 0.5) * (n - 1))
    const h = field.heights[Math.min(n - 1, Math.max(0, iz)) * n + Math.min(n - 1, Math.max(0, ix))]
    return typeof h === 'number' && Number.isFinite(h) ? h : 0
}
