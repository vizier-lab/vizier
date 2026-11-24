// The 3D entries of the registry: loaded on demand, so the main bundle carries no `three`.
import type { ComponentRenderProps, ComponentRegistry } from '@json-render/react'
import { lazy, Suspense, type ComponentType } from 'react'

type Three = typeof import('./three')

const NAMES = {
    Scene3D: 'Scene3DC',
    Group3D: 'Group3DC',
    Box: 'BoxC',
    Sphere: 'SphereC',
    Cylinder: 'CylinderC',
    Plane: 'PlaneC',
    AmbientLight: 'AmbientLightC',
    DirectionalLight: 'DirectionalLightC',
    PointLight: 'PointLightC',
    OrbitControls: 'OrbitControlsC',
    Heightfield: 'HeightfieldC',
    Mesh: 'MeshC',
} as const satisfies Record<string, keyof Three>

function load(name: keyof Three) {
    return lazy(() => import('./three').then((m) => ({ default: m[name] as ComponentType<ComponentRenderProps> })))
}

function entry(exportName: keyof Three, scene: boolean) {
    const Lazy = load(exportName)
    // Only the scene shows a placeholder; its parts load with it, inside its canvas.
    return function Part(props: ComponentRenderProps) {
        return (
            <Suspense fallback={scene ? <p className="type-small text-faint">Loading 3D…</p> : null}>
                <Lazy {...props} />
            </Suspense>
        )
    }
}

export const scene3dRegistry: ComponentRegistry = Object.fromEntries(
    Object.entries(NAMES).map(([type, exportName]) => [type, entry(exportName, type === 'Scene3D')])
)
