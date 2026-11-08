// Helpers shared by the share tests (not shipped: only tests import this file).
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ChatMessage, SessionFileRecord } from '../../api/frames'
import type { MediaDeps } from './media'

export interface ConversationFixture {
    agent_id: string
    topic_id: string
    agent_name: string
    pngBase64: string
    entries: ChatMessage[]
    files: SessionFileRecord[]
    seeds: { secrets: string[]; paths: string[]; recall: string[]; reasoning: string[]; sensitive: string[]; checkpoint: string[]; command: string[]; answerer: string[] }
}

export const loadConversation = (): ConversationFixture => JSON.parse(readFileSync(join(__dirname, 'fixtures', 'conversation.json'), 'utf8'))

/** Serves the fixture's one-pixel PNG for every file, with no re-encoder. */
export const pngDeps = (b64: string): MediaDeps => ({
    fetchFile: async () => ({ bytes: Uint8Array.from(Buffer.from(b64, 'base64')) }),
})
