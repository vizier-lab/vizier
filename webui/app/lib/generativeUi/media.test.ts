import { describe, expect, it } from 'vitest'
import type { SessionFileRecord } from '../../api/frames'
import { availableFiles, fileReferences, fileUrl, findFile } from './media'

const file = (filename: string, extra: Partial<SessionFileRecord> = {}): SessionFileRecord => ({
    id: filename,
    session_slug: 's',
    agent_id: 'a',
    filename,
    mime_type: 'image/png',
    size: 1,
    file_id: `id-${filename}`,
    added_at: '2026-10-24T10:00:00Z',
    ...extra,
})

describe('session file lookup', () => {
    it('finds a file by name, the newest when the name repeats', () => {
        const files = [file('a.png', { file_id: 'old', added_at: '2026-10-24T09:00:00Z' }), file('a.png', { file_id: 'new' }), file('b.png')]
        expect(findFile(files, 'a.png')?.file_id).toBe('new')
        expect(findFile(files, 'nope.png')).toBeUndefined()
    })

    it('builds a serving address through the transport', () => {
        expect(fileUrl(file('a.png'))).toBe('/api/v1/files/id-a.png')
    })

    it('lists the available files for an error, once each', () => {
        expect(availableFiles([file('a.png'), file('r.pdf', { mime_type: 'application/pdf' }), file('a.png')])).toBe('Available: a.png (image/png), r.pdf (application/pdf).')
        expect(availableFiles([])).toBe('This conversation has no files yet.')
    })

    it('collects the file names an interface refers to, with their paths', () => {
        const refs = fileReferences({
            a: { type: 'Image', props: { file: 'x.png', alt: 'x' } },
            b: { type: 'Gallery', props: { files: ['p.png', 'q.png'] } },
            c: { type: 'Video', props: { file: 'v.mp4', poster: 'v.png' } },
            d: { type: 'Canvas2D', props: { shapes: [{ kind: 'rect' }, { kind: 'image', file: 'm.png' }] } },
            e: { type: 'Text', props: { text: 'hi' } },
        })
        expect(refs.map((r) => r.name)).toEqual(['x.png', 'p.png', 'q.png', 'v.mp4', 'v.png', 'm.png'])
        expect(refs[1].path).toBe('elements.b.props.files[0]')
        expect(refs[5].path).toBe('elements.d.props.shapes[1].file')
    })
})
