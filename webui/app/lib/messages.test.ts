import { describe, expect, it } from 'vitest'
import type { VizierAttachment, VizierRequestContent, VizierResponseContent } from '../api/frames'
import { isAudioReply, requestAttachments, requestText, replyAttachments, replyText } from './messages'

const audio: VizierAttachment = { filename: 'audio_reply_1.wav', content: { local: '/api/v1/files/1' } }
const file: VizierAttachment = { filename: 'report.pdf', content: { local: '/api/v1/files/2' } }

const spoken: VizierResponseContent = { audio_reply: [audio, 'hello there', null] }

describe('spoken replies', () => {
    it('reads the transcript as the reply text', () => {
        expect(replyText(spoken)).toBe('hello there')
    })

    it('is recognised as an audio reply, not a plain message', () => {
        expect(isAudioReply(spoken)).toBe(true)
        expect(isAudioReply({ message: { content: 'hello there' } })).toBe(false)
        expect(isAudioReply(undefined)).toBe(false)
    })

    // The audio lives in the reply's content, so the attachment list alone never draws a player.
    it('carries the audio attachment alongside any others', () => {
        expect(replyAttachments(spoken, undefined)).toEqual([audio])
        expect(replyAttachments(spoken, [file])).toEqual([audio, file])
    })

    it('leaves a plain message to its own attachments', () => {
        expect(replyAttachments({ message: { content: 'hi' } }, [file])).toEqual([file])
        expect(replyAttachments({ message: { content: 'hi' } }, undefined)).toEqual([])
    })
})

describe('voice messages', () => {
    const voice: VizierRequestContent = { audio_chat: [audio, 'what is the weather'] }

    it('draws the audio and keeps the transcript as the text', () => {
        expect(requestAttachments(voice, undefined)).toEqual([audio])
        expect(requestText(voice)).toBe('what is the weather')
    })

    it('has no text yet while the transcript is not ready', () => {
        expect(requestText({ audio_chat: [audio, null] })).toBe('')
    })
})
