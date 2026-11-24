import { useEffect, useState } from 'react'
import { getGitChanges, type GitChange } from '../../api/endpoints/projects'
import { projectKey, useProjects } from '../../stores/projects'
import { ChangesDialog } from './ChangesDialog'

type Props = {
  agentId: string
  projectId: string
  topicId: string
  /** The dialog is open. */
  open: boolean
  onClose: () => void
  /** Re-reads the list when a turn ends, so the dialog shows what the agent did. */
  refreshKey?: unknown
}

/**
 * A project chat's uncommitted changes, behind the status strip's changes count:
 * the count opens this dialog, which is the same file-and-diff review the side
 * panel used to hold.
 */
export function ChangesSection({
  agentId,
  projectId,
  topicId,
  open,
  onClose,
  refreshKey,
}: Props) {
  const summary = useProjects((s) => s.git[`${projectKey(agentId, projectId)}/${topicId}`])
  const [changes, setChanges] = useState<GitChange[]>([])
  const [selected, setSelected] = useState<string | null>(null)

  useEffect(() => {
    if (!summary?.available) return
    let alive = true
    getGitChanges(agentId, projectId, topicId)
      .then((list) => alive && setChanges(list ?? []))
      .catch(() => alive && setChanges([]))
    return () => {
      alive = false
    }
  }, [agentId, projectId, topicId, summary?.available, refreshKey])

  return (
    <ChangesDialog
      agentId={agentId}
      projectId={projectId}
      topicId={topicId}
      changes={changes}
      open={open}
      onClose={onClose}
      path={selected}
      onPath={setSelected}
    />
  )
}
