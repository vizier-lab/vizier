import { FilterTabs } from '@vizier/design'
import { useSearchParams } from 'react-router'
import { AddonsPanel } from './AddonsPanel'
import { ChannelsPanel } from './ChannelsPanel'
import { McpPanel } from './McpPanel'

const SECTIONS = [
    { value: 'channels', label: 'Channels' },
    { value: 'addons', label: 'Addons' },
    { value: 'mcp', label: 'MCP servers' },
] as const

export type IntegrationSection = (typeof SECTIONS)[number]['value']

/**
 * Everything the agent is connected to, in one place on its Config: chat channels, addon tools and
 * MCP servers. These save as you change them (an integration never restarts the agent), unlike the
 * rest of Config; each panel says so.
 */
export function IntegrationsTab({ agentId }: { agentId: string }) {
    const [params, setParams] = useSearchParams()
    const section = SECTIONS.find((s) => s.value === params.get('section'))?.value ?? 'channels'
    return (
        <div className="flex flex-col gap-24">
            <div className="flex flex-wrap items-center gap-16">
                <FilterTabs
                    aria-label="Kind of integration"
                    value={section}
                    options={SECTIONS.map((s) => ({ ...s }))}
                    onChange={(value) =>
                        setParams(
                            (p) => {
                                const next = new URLSearchParams(p)
                                next.set('section', value)
                                return next
                            },
                            { replace: true }
                        )
                    }
                />
            </div>
            {section === 'channels' && <ChannelsPanel agentId={agentId} />}
            {section === 'addons' && <AddonsPanel agentId={agentId} />}
            {section === 'mcp' && <McpPanel agentId={agentId} />}
        </div>
    )
}
