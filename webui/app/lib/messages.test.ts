import { describe, expect, it } from 'vitest'
import type { VizierAttachment, VizierRequestContent, VizierResponseContent } from '../api/frames'
import { isAudioReply, replyAttachments, replySpoken, replyText, requestAttachments, requestSpoken, requestText } from './messages'

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

describe('the words of a recording', () => {
    it('pairs a spoken reply with the recording they were said in', () => {
        expect(replySpoken(spoken)).toEqual({ attachment: audio, transcript: 'hello there' })
    })

    // The bug: a written reply's prose was read back as the transcript of an audio it carried.
    it('leaves a written reply without words', () => {
        expect(replySpoken({ message: { content: 'here is the recording' } })).toBeNull()
        expect(replySpoken(undefined)).toBeNull()
    })

    it('pairs a voice message with the recording they were heard in', () => {
        expect(requestSpoken({ audio_chat: [audio, 'what is the weather'] })).toEqual({
            attachment: audio,
            transcript: 'what is the weather',
        })
    })

    it('leaves a typed message without words', () => {
        expect(requestSpoken({ chat: 'hello' })).toBeNull()
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
