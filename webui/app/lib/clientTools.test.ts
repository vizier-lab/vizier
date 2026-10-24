import { describe, expect, it } from 'vitest'
import { declaredClientTools, findClientTool, registerClientTool } from './clientTools'

describe('retired client tools', () => {
    it('are not declared to the agent but still resolve for drawing saved calls', () => {
        const stop = registerClientTool({
            declare: false,
            definition: { name: 'old_tool', category: 'legacy_ui', description: 'Retired', input_schema: { type: 'object' } },
            handle: () => null,
        })
        expect(declaredClientTools().client_tools.some((t) => t.name === 'old_tool')).toBe(false)
        expect(findClientTool('legacy_ui', 'old_tool')).toBeDefined()
        stop()
    })

    it('declares an ordinary tool', () => {
        const stop = registerClientTool({
            definition: { name: 'new_tool', category: 'legacy_ui', description: 'Live', input_schema: { type: 'object' }, ends_turn: true },
            handle: () => null,
        })
        const declared = declaredClientTools().client_tools.find((t) => t.name === 'new_tool')
        expect(declared?.ends_turn).toBe(true)
        stop()
    })
})
