import { api } from '../api/client'
import type { VizierAttachment } from '../api/frames'

export const MAX_UPLOAD_BYTES = 25 * 1024 * 1024

/** The file's bytes as base64, which is how `/files/upload` takes them. */
export function fileToBase64(file: Blob): Promise<string> {
    return new Promise((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(String(reader.result).split(',')[1] ?? '')
        reader.onerror = () => reject(reader.error ?? new Error('could not read the file'))
        reader.readAsDataURL(file)
    })
}

export async function uploadFile(file: File): Promise<VizierAttachment> {
    const res = await api.post<{ url: string }>('/files/upload', { file: await fileToBase64(file), filename: file.name })
    return { filename: file.name, content: { local: res.url } }
}

/** "12 KB", "3.4 MB" */
export function fileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}
