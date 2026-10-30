import type { SessionFileRecord } from '../../api/frames'
import { fileSize } from '../../lib/upload'
import { FilePreviewModal, type PreviewFile } from '../shared/FilePreviewModal'

export { PREVIEW_BYTES, parseCsv } from '../shared/FilePreviewModal'

type Props = {
    files: SessionFileRecord[]
    index: number
    onIndex: (i: number) => void
    onClose: () => void
}

const toPreview = (f: SessionFileRecord): PreviewFile => ({
    key: f.id,
    name: f.filename,
    mimeType: f.mime_type,
    size: f.size,
    url: `/api/v1/files/${f.file_id}`,
    meta: `${fileSize(f.size)}${f.sender ? ` · sent by ${f.sender}` : ''}`,
})

/** A topic's files in the shared preview. */
export function FilePreviewDialog({ files, ...rest }: Props) {
    return <FilePreviewModal files={files.map(toPreview)} {...rest} />
}
