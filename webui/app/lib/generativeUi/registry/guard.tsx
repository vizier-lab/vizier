// A component that throws while drawing shows an inline chip for itself only; the rest of the
// interface keeps working (contracts/generative-ui-tools.md §6).
import type { ComponentRegistry, ComponentRenderProps } from '@json-render/react'
import { Component, type ErrorInfo, type ReactNode } from 'react'

class Boundary extends Component<{ name: string; children: ReactNode }, { failed: boolean }> {
    state = { failed: false }

    static getDerivedStateFromError() {
        return { failed: true }
    }

    componentDidCatch(error: Error, info: ErrorInfo) {
        console.warn(`generative UI: ${this.props.name} failed to draw`, error, info.componentStack)
    }

    render() {
        if (!this.state.failed) return this.props.children
        return (
            <span role="alert" className="inline-flex items-center rounded-xs bg-failed-bg px-6 py-2 type-mono-xs text-failed-text">
                {this.props.name} could not be drawn
            </span>
        )
    }
}

/** Every renderer of `registry`, wrapped so one broken component does not take the page down. */
export function guarded(registry: ComponentRegistry): ComponentRegistry {
    return Object.fromEntries(
        Object.entries(registry).map(([name, Inner]) => [
            name,
            (props: ComponentRenderProps) => (
                <Boundary name={name}>
                    <Inner {...props} />
                </Boundary>
            ),
        ])
    )
}
