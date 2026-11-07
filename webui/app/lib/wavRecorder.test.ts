import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { WORKLET_URL } from './wavRecorder'

describe('the capture worklet', () => {
    // The desktop app's CSP refuses a blob: worklet, so it has to be a file on our own origin.
    it('is a same-origin file that registers the processor the recorder asks for', () => {
        expect(WORKLET_URL.startsWith('/')).toBe(true)
        const file = join(process.cwd(), 'public', WORKLET_URL)
        expect(existsSync(file)).toBe(true)
        expect(readFileSync(file, 'utf8')).toContain("registerProcessor('vizier-capture'")
    })
})
