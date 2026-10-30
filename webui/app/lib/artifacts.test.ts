import { describe, expect, it } from 'vitest'
import {
    actionLabel,
    bytesLabel,
    canPreview,
    kindLabel,
    plural,
    previewNote,
    referenceLabel,
    unavailableLabel,
} from './artifacts'
import type { ArtifactEvent } from '../api/endpoints/artifacts'

describe('artifacts', () => {
    it('labels kinds and previewability', () => {
        expect(kindLabel('pdf')).toBe('PDFs')
        expect(canPreview('image')).toBe(true)
        expect(canPreview('audio')).toBe(false)
        expect(previewNote('audio')).toContain('No preview')
        expect(previewNote('text')).toBe('')
    })

    it('names an unavailable artifact rather than only colouring it', () => {
        expect(unavailableLabel({ name: 'Q3.pdf' })).toBe('Q3.pdf — artifact unavailable')
    })

    it('labels events and references', () => {
        const event = { action: 'deleted' } as Pick<ArtifactEvent, 'action'>
        expect(actionLabel(event)).toBe('Deleted')
        expect(referenceLabel({ kind: 'memory', label: 'Q3 planning' })).toBe('Memory · Q3 planning')
        expect(referenceLabel({ kind: 'message', label: 'WebUI conversation' })).toBe('Message · WebUI conversation')
    })

    it('formats counts and sizes', () => {
        expect(plural(1, 'file')).toBe('1 file')
        expect(plural(4, 'file')).toBe('4 files')
        expect(bytesLabel(512)).toBe('512 B')
        expect(bytesLabel(2048)).toBe('2.0 KB')
    })
})
