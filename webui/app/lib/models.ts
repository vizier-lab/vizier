import { abbreviate } from './format'
import type { ModelSummary } from '../api/endpoints/catalogue'

/** "$3" or "$0.25": no trailing zeros, cents when under a dollar. */
function price(usd: number): string {
    return `$${Number(usd.toFixed(usd < 1 ? 3 : 2))}`
}

/** What a model takes in, as a phrase: "Text, image and tool input". */
export function inputPhrase(caps: ModelSummary['caps']): string {
    const parts = ['Text']
    if (caps.image_input) parts.push('image')
    if (caps.tool_call) parts.push('tool')
    if (parts.length === 1) return 'Text input'
    return `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]} input`
}

/** The line under a model choice: "Text, image and tool input · 200k context · $3 / $15 per M tokens". */
export function capabilityLine(model: ModelSummary): string {
    const parts = [inputPhrase(model.caps)]
    if (model.limit.context) parts.push(`${abbreviate(model.limit.context)} context`)
    if (model.cost) parts.push(`${price(model.cost.input)} / ${price(model.cost.output)} per M tokens`)
    return parts.join(' · ')
}
