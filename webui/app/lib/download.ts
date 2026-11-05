import { transport } from '../api/transport'
import { toast } from '../stores/toasts'

/** Saves a server file for the person, and tells them if it couldn't be, as nothing else on screen will. */
export async function downloadFile(path: string, filename: string): Promise<void> {
    try {
        await transport.saveFile(path, filename)
    } catch (e) {
        // The browser rejects with an ApiError, the desktop app with a plain string.
        const reason = e instanceof Error ? e.message : typeof e === 'string' ? e : undefined
        toast.error(`${filename} couldn't be downloaded`, reason)
    }
}
