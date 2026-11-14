import {
  BackgroundWork,
  EmptyState,
  FileRow,
  FileTypeTag,
  FilterTabs,
  SearchField,
  StatusPanel,
  StatusStrip,
  TextLink,
  type StatusStripItem,
  type StatusStripItemId,
} from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import type { DetachedScriptDetail, DetachedScriptSnapshot, SessionFileRecord, SubagentSnapshot } from '../../api/frames'
import { detachedScript } from '../../api/endpoints/chat'
import { detachedReport, scriptElapsed } from '../../lib/detached'
import { Run } from './Trail'
import { clearTopicFiles, promoteTopicFile } from '../../api/endpoints/artifacts'
import { time } from '../../lib/messages'
import { fileSize } from '../../lib/upload'
import {
  PAGE,
  fileKinds,
  filterFiles,
  filterWork,
  workCounts,
  type WorkTab,
} from '../../lib/statusFilters'
import { readyCount, stripLabel, worstDot } from '../../lib/lsp'
import { useProjects } from '../../stores/projects'
import { toast } from '../../stores/toasts'
import { LspPanel } from './LspPanel'
import { SubagentsPanel } from './SubagentsPanel'
import { workerStrip } from '../../lib/subagents'
import { FilePreviewDialog } from './FilePreviewDialog'
import { ConfirmDialog } from '../shared/ConfirmDialog'

type Props = {
  agentId: string
  topicId: string
  /** The topic's detached scripts (spec 058). */
  scripts: DetachedScriptSnapshot[]
  /** The workers the topic hired (spec 059): their own item and panel, apart from Background work. */
  workers: SubagentSnapshot[]
  /** The agent's name, for "Waiting for <name>". */
  agentName: string
  /** The conversation's channel, for the worker reads. */
  channel?: string
  files: SessionFileRecord[]
  canPromote: boolean
  onFilesCleared: () => void
  /** A project chat's uncommitted changes: the count joins the strip, and the item opens the changes dialog. */
  changes?: { count: number; open: () => void }
  /** A project chat's language servers (spec 054): the item opens the panel. Other chats have none. */
  lsp?: { projectId: string; enabled: boolean }
  /** How far above the strip the panel sits: the composer's height, so it clears the composer. */
  panelGap?: number
  /** The panel's width in px: the composer's, since the strip itself is only as wide as its counts. */
  panelWidth?: number
  className?: string
}

/**
 * One detached script in the Background work panel: collapsed to a line, expanded to the
 * activity trail's run detail. The detail is read on first expand and again whenever a frame
 * says the script changed. No cancel control: only the agent cancels (spec 058).
 */
function WorkEntry({
  agentId,
  topicId,
  script,
  now,
}: {
  agentId: string
  topicId: string
  script: DetachedScriptSnapshot
  now: number
}) {
  const [expanded, setExpanded] = useState(false)
  const [detail, setDetail] = useState<DetachedScriptDetail | null>(null)
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    if (!expanded) return
    let live = true
    detachedScript(agentId, topicId, script.id)
      .then((d) => live && (setDetail(d), setFailed(false)))
      .catch(() => live && setFailed(true))
    return () => {
      live = false
    }
  }, [expanded, agentId, topicId, script.id, script.state, script.tool_call_count, script.delivered])
  return (
    <BackgroundWork
      intent={script.intent}
      handle={script.id}
      state={script.state}
      elapsed={scriptElapsed(script, now)}
      callCount={script.tool_call_count}
      delivered={script.delivered}
      expanded={expanded}
      onExpandedChange={setExpanded}
    >
      {detail ? (
        <Run report={detachedReport(detail)} code={detail.code} intent={null} />
      ) : (
        <p className="type-small text-faint">{failed ? "Couldn't load this script." : 'Loading...'}</p>
      )}
    </BackgroundWork>
  )
}

/**
 * The chat's status strip (D62): the counts of what the agent is still working on and
 * what it left in the conversation. Each count opens a panel
 * that hovers above the composer; a project's changes open their own dialog.
 */
export function ChatStatusStrip({
  agentId,
  topicId,
  scripts,
  workers,
  agentName,
  channel,
  files,
  canPromote,
  onFilesCleared,
  changes,
  lsp,
  panelGap,
  panelWidth,
  className,
}: Props) {
  const [now, setNow] = useState(() => Date.now())
  // The strip owns which panel is open, so the panel's x can close it.
  const [open, setOpen] = useState<StatusStripItemId | null>(null)
  const [query, setQuery] = useState('')
  const [tab, setTab] = useState<string | null>(null)
  const [preview, setPreview] = useState<number | null>(null)
  const [confirmClear, setConfirmClear] = useState(false)
  const [clearing, setClearing] = useState(false)

  const running = scripts.filter((s) => s.state === 'running')
  const counts = workCounts(scripts)
  const atWork = workerStrip(workers)

  // The elapsed times tick while work runs, and stop when nothing does.
  useEffect(() => {
    if (running.length === 0 && atWork.count === 0) return
    const t = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(t)
  }, [running.length, atWork.count])

  const items: StatusStripItem[] = [
    {
      id: 'work',
      icon: 'layers',
      label: 'background work running',
      count: running.length,
      busy: running.length > 0,
    },
    {
      id: 'subagents',
      icon: 'users',
      label: atWork.dot === 'warn' ? 'subagents at work, one is asking' : 'subagents at work',
      count: atWork.count,
      dot: atWork.dot,
    },
    { id: 'files', icon: 'file', label: 'session files', count: files.length },
  ]
  if (changes) {
    items.push({
      id: 'changes',
      icon: 'git-branch',
      label: 'changed files',
      count: changes.count,
      onSelect: changes.open,
    })
  }

  const lspView = useProjects((s) => (lsp ? s.lsp[lsp.projectId] : undefined))
  // The status is read as soon as the chat shows, so the strip's number is right before the panel
  // is ever opened. The inbox stream keeps it current after that.
  useEffect(() => {
    if (!lsp) return
    return useProjects.getState().watchLsp(agentId, lsp.projectId)
  }, [agentId, lsp?.projectId])
  if (lsp) {
    const snapshot = lspView ?? { enabled: lsp.enabled, servers: [] }
    items.push({
      id: 'lsp',
      icon: 'code',
      label: stripLabel(snapshot),
      count: readyCount(snapshot),
      display: snapshot.enabled ? undefined : 'off',
      dot: worstDot(snapshot),
    })
  }

  const kinds = useMemo(() => fileKinds(files), [files])

  const workRows = useMemo(() => filterWork(scripts, query, (tab ?? 'running') as WorkTab), [scripts, query, tab])
  const fileRows = useMemo(() => filterFiles(files, query, tab), [files, query, tab])

  const doClear = async () => {
    setClearing(true)
    try {
      const result = await clearTopicFiles(agentId, 'vizier-webui', topicId)
      toast.success(`Cleared ${result.removed} session files.`)
      setConfirmClear(false)
      onFilesCleared()
    } catch (e) {
      toast.error("Couldn't clear session files", e instanceof Error ? e.message : undefined)
    } finally {
      setClearing(false)
    }
  }

  const doPromote = async (filename: string) => {
    try {
      await promoteTopicFile(agentId, 'vizier-webui', topicId, { filename })
      toast.success(`Promoted "${filename}" to artifacts.`)
    } catch (e) {
      toast.error("Couldn't promote the file", e instanceof Error ? e.message : undefined)
    }
  }

  const WORK_EMPTY: Record<WorkTab, string> = { running: 'running', done: 'done yet', stopped: 'stopped' }

  const workPanel = (close: () => void) => (
    <StatusPanel
      title="Background work"
      meta={counts.running ? `${counts.running} running` : `${counts.total}`}
      onClose={close}
      search={
        <SearchField
          value={query}
          onValueChange={setQuery}
          aria-label="Search background work"
          placeholder="Search calls..."
          count={workRows.length}
        />
      }
      tabs={
        <FilterTabs
          aria-label="Filter background work by state"
          value={tab ?? 'running'}
          onChange={(v) => setTab(v === 'running' ? null : v)}
          options={[
            { value: 'running', label: 'Running', count: counts.running, tone: 'live' },
            { value: 'done', label: 'Done', count: counts.done },
            { value: 'stopped', label: 'Stopped', count: counts.stopped },
          ]}
        />
      }
      footer={<span className="type-small text-faint">Newest first</span>}
    >
      {workRows.length === 0 ? (
        <EmptyState
          kind="empty"
          icon="layers"
          title={
            scripts.length === 0 ? 'No background work' : `Nothing ${WORK_EMPTY[(tab ?? 'running') as WorkTab]}`
          }
          message="Scripts the agent runs in the background appear here."
        />
      ) : (
        <ul className="flex flex-col gap-6 py-6">
          {workRows.slice(0, PAGE).map((row) =>
            row.kind === 'script' ? (
              <li key={row.key}>
                <WorkEntry agentId={agentId} topicId={topicId} script={row.script} now={now} />
              </li>
            ) : null
          )}
        </ul>
      )}
    </StatusPanel>
  )

  const filesPanel = (close: () => void) => (
    <StatusPanel
      title="Session files"
      meta={`${files.length}`}
      onClose={close}
      search={
        <SearchField
          value={query}
          onValueChange={setQuery}
          aria-label="Search session files"
          placeholder="Search files..."
          count={fileRows.length}
        />
      }
      tabs={
        <FilterTabs
          aria-label="Filter session files by kind"
          value={tab ?? 'all'}
          onChange={(v) => setTab(v === 'all' ? null : v)}
          options={[
            { value: 'all', label: 'All', count: files.length },
            ...kinds.slice(0, 4).map((k) => ({ value: k.value, label: k.label, count: k.count })),
          ]}
        />
      }
      footer={
        <>
          <span className="type-small text-faint">Newest first</span>
          {files.length > 0 && (
            <TextLink tone="danger" onClick={() => setConfirmClear(true)}>
              Clear all files...
            </TextLink>
          )}
        </>
      }
    >
      {fileRows.length === 0 ? (
        <EmptyState
          kind="empty"
          icon="file"
          title="No files yet"
          message="Files the agent makes or you attach appear here."
        />
      ) : (
        <ul className="flex flex-col py-6">
          {fileRows.slice(0, PAGE).map((row) =>
            row.kind === 'file' ? (
              <li key={row.key}>
                <FileRow
                  name={row.file.filename}
                  size={fileSize(row.file.size)}
                  subtitle={`${row.file.sender ? `${row.file.sender} - ` : ''}${time(row.file.added_at)}`}
                  onSelect={() => setPreview(files.indexOf(row.file))}
                  actions={
                    <span className="flex items-center gap-8">
                      {canPromote && (
                        <TextLink onClick={() => void doPromote(row.file.filename)}>Promote</TextLink>
                      )}
                      <FileTypeTag file={row.file.filename} />
                    </span>
                  }
                />
              </li>
            ) : null
          )}
        </ul>
      )}
      {preview !== null && files[preview] && (
        <FilePreviewDialog files={files} index={preview} onIndex={setPreview} onClose={() => setPreview(null)} />
      )}
      <ConfirmDialog
        open={confirmClear}
        title="Clear session files?"
        removes={files.map((f) => f.filename)}
        message={`This removes ${files.length} files from this conversation. Artifacts are unaffected. Older messages or memories that reference these files will no longer open them.`}
        confirmLabel="Clear session files"
        busy={clearing}
        onConfirm={() => void doClear()}
        onCancel={() => setConfirmClear(false)}
      />
    </StatusPanel>
  )

  const panelFor = (id: StatusStripItem['id'], close: () => void) =>
    id === 'work' ? (
      workPanel(close)
    ) : id === 'subagents' ? (
      <SubagentsPanel
        agentId={agentId}
        topicId={topicId}
        channel={channel}
        agentName={agentName}
        workers={workers}
        now={now}
        onClose={close}
      />
    ) : id === 'lsp' && lsp ? (
      <LspPanel agentId={agentId} projectId={lsp.projectId} onClose={close} />
    ) : (
      filesPanel(close)
    )

  return (
    <StatusStrip
      items={items}
      className={className}
      fullWidth={false}
      open={open}
      onOpenChange={setOpen}
      panelGap={panelGap}
      panelWidth={panelWidth}
      // The language-server and subagents items are left out of the key: their status changes under
      // an open panel, and remounting would drop an override form's text or close an expanded worker.
      key={`${items.filter((i) => i.id !== 'lsp' && i.id !== 'subagents').map((i) => i.count).join('-')}`}
    >
      {(item, close) => panelFor(item.id, close)}
    </StatusStrip>
  )
}
