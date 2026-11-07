import { beforeEach, describe, expect, it, vi } from 'vitest'

const saveFile = vi.fn()
const error = vi.fn()
vi.mock('../api/transport', () => ({ transport: { saveFile: (...a: unknown[]) => saveFile(...a) } }))
vi.mock('../stores/toasts', () => ({ toast: { error: (...a: unknown[]) => error(...a) } }))

import { downloadFile } from './download'

describe('downloadFile', () => {
    beforeEach(() => {
        saveFile.mockReset()
        error.mockReset()
    })

    it('says nothing when the file is saved', async () => {
        saveFile.mockResolvedValue(undefined)
        await downloadFile('/files/abc', 'notes.txt')
        expect(saveFile).toHaveBeenCalledWith('/files/abc', 'notes.txt')
        expect(error).not.toHaveBeenCalled()
    })

    it('names the file and the reason from the browser', async () => {
        saveFile.mockRejectedValue(new Error('File not found'))
        await downloadFile('/files/abc', 'notes.txt')
        expect(error).toHaveBeenCalledWith("notes.txt couldn't be downloaded", 'File not found')
    })

    it('names the reason from the desktop app, which rejects with a string', async () => {
        saveFile.mockRejectedValue('the file could not be read (status 404)')
        await downloadFile('/files/abc', 'notes.txt')
        expect(error).toHaveBeenCalledWith("notes.txt couldn't be downloaded", 'the file could not be read (status 404)')
    })
})
