/**
 * Encode an AudioBuffer to a WAV file (PCM 16-bit).
 * WAV = 44-byte header + raw PCM samples.
 */
export function encodeWav(buffer: AudioBuffer): Blob {
  const numChannels = 1 // mono
  const sampleRate = buffer.sampleRate
  const format = 1 // PCM

  // Mix down to mono if stereo
  let samples: Float32Array
  if (buffer.numberOfChannels === 1) {
    samples = buffer.getChannelData(0)
  } else {
    const left = buffer.getChannelData(0)
    const right = buffer.getChannelData(1)
    samples = new Float32Array(left.length)
    for (let i = 0; i < left.length; i++) {
      samples[i] = (left[i] + right[i]) * 0.5
    }
  }

  const bitsPerSample = 16
  const bytesPerSample = bitsPerSample / 8
  const dataSize = samples.length * bytesPerSample
  const headerSize = 44
  const totalSize = headerSize + dataSize

  const arrayBuffer = new ArrayBuffer(totalSize)
  const view = new DataView(arrayBuffer)

  // RIFF header
  writeString(view, 0, 'RIFF')
  view.setUint32(4, totalSize - 8, true)
  writeString(view, 8, 'WAVE')

  // fmt subchunk
  writeString(view, 12, 'fmt ')
  view.setUint32(16, 16, true) // subchunk size
  view.setUint16(20, format, true)
  view.setUint16(22, numChannels, true)
  view.setUint32(24, sampleRate, true)
  view.setUint32(28, sampleRate * numChannels * bytesPerSample, true)
  view.setUint16(32, numChannels * bytesPerSample, true)
  view.setUint16(34, bitsPerSample, true)

  // data subchunk
  writeString(view, 36, 'data')
  view.setUint32(40, dataSize, true)

  // PCM samples (float32 → int16)
  let offset = 44
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]))
    view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7fff, true)
    offset += 2
  }

  return new Blob([arrayBuffer], { type: 'audio/wav' })
}

/**
 * Decode a Blob (any browser-supported format) to AudioBuffer,
 * then encode as WAV. Returns a File ready for upload.
 */
export async function blobToWavFile(blob: Blob, filename = 'recording.wav'): Promise<File> {
  const audioCtx = new AudioContext()
  try {
    const arrayBuffer = await blob.arrayBuffer()
    const audioBuffer = await audioCtx.decodeAudioData(arrayBuffer)
    const wavBlob = encodeWav(audioBuffer)
    return new File([wavBlob], filename, { type: 'audio/wav' })
  } finally {
    await audioCtx.close()
  }
}

function writeString(view: DataView, offset: number, str: string) {
  for (let i = 0; i < str.length; i++) {
    view.setUint8(offset + i, str.charCodeAt(i))
  }
}
