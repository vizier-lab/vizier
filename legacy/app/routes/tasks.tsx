import { useEffect, useState } from 'react'
import { useParams } from 'react-router'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import {
  listTasks,
  getTask,
  createTask,
  updateTask,
  deleteTask,
  listTaskRuns,
  getTaskRunHistory,
} from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import { FaPlus, FaTrash, FaPenToSquare, FaChevronDown, FaChevronRight } from 'react-icons/fa6'
import { Skeleton } from '../components/Skeleton'
import { useToastStore } from '../hooks/toastStore'
import type { ChatMessage, Requester, Task, TaskRun, TaskRunState } from '../interfaces/types'
import DatePicker from '../components/DatePicker'
import MarkdownEditor from '../components/MarkdownEditor'
import SlideOver from '../components/SlideOver'
import ActivityTrail from '../components/ActivityTrail'
import { groupHistory, outcomeDurationMs } from '../lib/trail'
import { formatToolChoice } from './chat'

// How many runs a page of the past-runs list holds.
const RUNS_PAGE_SIZE = 10

// A run's state, as a person reads it. `no_response` and `interrupted` are the two that
// need saying plainly: one means the run finished having produced nothing to read, the
// other that it died with the process and was swept at the next startup.
const RUN_STATE_LABEL: Record<TaskRunState, string> = {
  running: 'Running',
  answered: 'Answered',
  no_response: 'No response',
  interrupted: 'Interrupted',
}

const RUN_STATE_COLOR: Record<TaskRunState, { background: string; color: string }> = {
  running: { background: '#e3f2fd', color: '#1565c0' },
  answered: { background: '#e8f5e9', color: '#2e7d32' },
  no_response: { background: '#fff8e1', color: '#ef6c00' },
  interrupted: { background: '#ffebee', color: '#c62828' },
}

function RunStateBadge({ state }: { state: TaskRunState }) {
  const palette = RUN_STATE_COLOR[state]
  return (
    <span style={{
      padding: '2px 8px', borderRadius: '12px', fontSize: '11px', fontWeight: 600,
      background: palette.background, color: palette.color,
    }}>
      {RUN_STATE_LABEL[state]}
    </span>
  )
}

// A person who can no longer be resolved still renders as recorded — the string is what was
// stored when the task was created, and nothing looks it up.
function requesterLabel(requester: Requester): string {
  if ('agent' in requester) return 'the agent\u2019s own initiative'
  return requester.user
}

// A turn's final message, where it produced one. Only `message` content is drawn: the rest
// of the response kinds are trail events, which `ActivityTrail` has already rendered.
function runOutcomeText(outcome?: ChatMessage): string | null {
  // `VizierResponseContent` includes bare string variants (`'thinking_start'` and the
  // like), so the object check is what lets `in` narrow at all.
  const content = outcome?.content.Response?.content
  if (typeof content !== 'object' || content === null) return null
  return 'message' in content ? content.message.content : null
}

function runDuration(run: TaskRun): string | null {
  if (!run.finished_at) return null
  const ms = new Date(run.finished_at).getTime() - new Date(run.ran_at).getTime()
  if (!Number.isFinite(ms) || ms < 0) return null
  if (ms < 1000) return `${ms}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  const minutes = Math.floor(ms / 60_000)
  const seconds = Math.round((ms % 60_000) / 1000)
  return `${minutes}m ${seconds}s`
}

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

type ModalMode = 'create' | 'edit' | 'view' | null
type ScheduleType = 'Cron' | 'OneTime'

const CRON_TEMPLATES = [
  { label: 'Custom', value: '' },
  { label: 'Every 15 minutes', value: '*/15 * * * *' },
  { label: 'Every hour', value: '0 * * * *' },
  { label: 'Daily at midnight', value: '0 0 * * *' },
  { label: 'Daily at noon', value: '0 12 * * *' },
  { label: 'Weekly on Sunday 6pm', value: '0 18 * * 0' },
  { label: 'Weekly on Monday 9am', value: '0 9 * * 1' },
  { label: 'Weekdays at 9am', value: '0 9 * * 1-5' },
  { label: 'Monthly on 1st', value: '0 0 1 * *' },
  { label: 'Monthly on 15th', value: '0 0 15 * *' },
  { label: 'Quarterly', value: '0 0 1 1,4,7,10 *' },
  { label: 'Yearly on Jan 1st', value: '0 0 1 1 *' },
]

export default function TaskManagement() {
  const { agentId } = useParams()
  const addToast = useToastStore((s) => s.addToast)
  const [tasks, setTasks] = useState<Task[]>([])
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [loading, setLoading] = useState(true)
  const [modalMode, setModalMode] = useState<ModalMode>(null)
  const [filterActive, setFilterActive] = useState<boolean | undefined>(undefined)

  const [formSlug, setFormSlug] = useState('')
  const [formTitle, setFormTitle] = useState('')
  const [formInstruction, setFormInstruction] = useState('')
  const [formScheduleType, setFormScheduleType] = useState<ScheduleType>('Cron')
  const [formScheduleValue, setFormScheduleValue] = useState('')
  const [submitting, setSubmitting] = useState(false)

  // Past runs of the open task, oldest-page-last. The cursor is the last row of the newest
  // page already loaded, so paging further back cannot shift under a run landing at the head.
  const [runs, setRuns] = useState<TaskRun[]>([])
  const [runsHasMore, setRunsHasMore] = useState(false)
  const [runsLoading, setRunsLoading] = useState(false)
  // run_id of the row expanded in place, and the history it rendered.
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null)
  const [runHistory, setRunHistory] = useState<ChatMessage[]>([])
  const [runHistoryLoading, setRunHistoryLoading] = useState(false)

  useEffect(() => {
    loadTasks()
  }, [agentId, filterActive])

  const loadTasks = async () => {
    if (!agentId) return
    try {
      setLoading(true)
      const response = await listTasks(agentId, filterActive)
      setTasks(response.data || [])
    } catch (error) {
      console.error('Failed to load tasks:', error)
    } finally {
      setLoading(false)
    }
  }

  const handleViewTask = async (slug: string) => {
    if (!agentId) return
    try {
      const response = await getTask(agentId, slug)
      setSelectedTask(response.data)
      setModalMode('view')
      setRuns([])
      setRunsHasMore(false)
      setExpandedRunId(null)
      setRunHistory([])
      await loadRuns(slug, null)
    } catch (error) {
      console.error('Failed to load task:', error)
    }
  }

  // `cursor` is the oldest run already shown; `null` loads the first page. Both halves of
  // the cursor travel together, which is what keeps runs sharing a millisecond from
  // straddling a page boundary.
  const loadRuns = async (slug: string, cursor: TaskRun | null) => {
    if (!agentId) return
    try {
      setRunsLoading(true)
      const response = await listTaskRuns(
        agentId,
        slug,
        cursor?.ran_at,
        cursor?.id,
        RUNS_PAGE_SIZE
      )
      const page: TaskRun[] = response.data?.runs ?? []
      setRuns((previous) => (cursor ? [...previous, ...page] : page))
      setRunsHasMore(Boolean(response.data?.has_more))
    } catch (error) {
      console.error('Failed to load task runs:', error)
      addToast('error', 'Failed to load past runs', getErrorMessage(error))
    } finally {
      setRunsLoading(false)
    }
  }

  // A run expands in place rather than navigating to the chat view, which has no way to
  // address a task's session at all.
  const handleToggleRun = async (run: TaskRun) => {
    if (!agentId || !selectedTask) return
    if (expandedRunId === run.run_id) {
      setExpandedRunId(null)
      setRunHistory([])
      return
    }

    setExpandedRunId(run.run_id)
    setRunHistory([])
    try {
      setRunHistoryLoading(true)
      const response = await getTaskRunHistory(agentId, selectedTask.slug, run.run_id)
      setRunHistory(response.data ?? [])
    } catch (error) {
      console.error('Failed to load run history:', error)
      addToast('error', 'Failed to load that run', getErrorMessage(error))
    } finally {
      setRunHistoryLoading(false)
    }
  }

  const handleEditTask = (task: Task) => {
    setSelectedTask(task)
    setFormSlug(task.slug)
    setFormTitle(task.title)
    setFormInstruction(task.instruction)
    if ('CronTask' in task.schedule) {
      setFormScheduleType('Cron')
      setFormScheduleValue(task.schedule.CronTask)
    } else if ('OneTimeTask' in task.schedule) {
      setFormScheduleType('OneTime')
      setFormScheduleValue(task.schedule.OneTimeTask)
    }
    setModalMode('edit')
  }

  const handleCreateTask = () => {
    setFormSlug('')
    setFormTitle('')
    setFormInstruction('')
    setFormScheduleType('Cron')
    setFormScheduleValue('0 0 * * *')
    setModalMode('create')
  }

  const handleSubmit = async () => {
    if (!agentId || !formSlug.trim() || !formTitle.trim() || !formInstruction.trim() || !formScheduleValue.trim()) return
    setSubmitting(true)
    try {
      const finalSlug = autoCorrectSlugStrict(formSlug)
      if (!finalSlug) return
      const taskData = {
        slug: finalSlug,
        title: formTitle,
        instruction: formInstruction,
        schedule: formScheduleType === 'Cron'
          ? { type: 'Cron' as const, expression: formScheduleValue }
          : { type: 'OneTime' as const, datetime: formScheduleValue },
      }
      if (modalMode === 'create') {
        await createTask(agentId, taskData)
        addToast('success', 'Task created')
      } else if (modalMode === 'edit' && selectedTask) {
        await updateTask(agentId, selectedTask.slug, taskData)
        addToast('success', 'Task updated')
      }
      await loadTasks()
      closeModal()
    } catch (error: unknown) {
      console.error('Failed to save task:', error)
      addToast('error', 'Failed to save task', getErrorMessage(error))
    } finally {
      setSubmitting(false)
    }
  }

  const handleDeleteTask = async (slug: string, e: React.MouseEvent) => {
    e.stopPropagation()
    if (!agentId) return
    if (!confirm('Are you sure you want to delete this task?')) return
    try {
      await deleteTask(agentId, slug)
      addToast('success', 'Task deleted')
      await loadTasks()
      closeModal()
    } catch (error: unknown) {
      console.error('Failed to delete task:', error)
      addToast('error', 'Failed to delete task', getErrorMessage(error))
    }
  }

  const closeModal = () => {
    setModalMode(null)
    setSelectedTask(null)
    setRuns([])
    setRunsHasMore(false)
    setExpandedRunId(null)
    setRunHistory([])
    setFormSlug('')
    setFormTitle('')
    setFormInstruction('')
    setFormScheduleType('Cron')
    setFormScheduleValue('')
  }

  const getScheduleDisplay = (schedule: Task['schedule']) => {
    if ('CronTask' in schedule) return schedule.CronTask
    if ('OneTimeTask' in schedule) return new Date(schedule.OneTimeTask).toLocaleString()
    return 'Unknown'
  }

  return (
    <>
      <div className="main-header">
        <div style={{ flex: 1 }}>
          <h3 style={{ margin: 0 }}>Task Management</h3>
        </div>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <select
            value={filterActive === undefined ? 'all' : filterActive ? 'active' : 'inactive'}
            onChange={(e) => {
              if (e.target.value === 'all') setFilterActive(undefined)
              else setFilterActive(e.target.value === 'active')
            }}
            style={{ padding: '8px 16px', borderRadius: '4px', border: '1px solid var(--border)', background: 'var(--background)' }}
          >
            <option value="all">All Tasks</option>
            <option value="active">Active</option>
            <option value="inactive">Inactive</option>
          </select>
          <button className="btn btn-primary" onClick={handleCreateTask}>
            <FaPlus size={16} />
            <span>New Task</span>
          </button>
        </div>
      </div>

      <div className="main-body">
        {loading ? (
          <table className="data-table">
            <thead>
              <tr>
                <th>Title</th>
                <th>Slug</th>
                <th>Schedule</th>
                <th>Requested by</th>
                <th>Status</th>
                <th>Last run</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {[1, 2, 3, 4, 5].map((i) => (
                <tr key={i} style={{ cursor: 'default' }}>
                  <td><Skeleton variant="text" width="60%" /></td>
                  <td><Skeleton variant="text" width="40%" /></td>
                  <td><Skeleton variant="text" width="50%" /></td>
                  <td><Skeleton variant="text" width="50%" /></td>
                  <td><Skeleton variant="text" width="50px" /></td>
                  <td><Skeleton variant="text" width="60%" /></td>
                  <td><Skeleton variant="text" width="60px" /></td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : tasks.length === 0 ? (
          <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', padding: '3rem' }}>
            <p>No tasks yet.</p>
            <button className="btn btn-primary" onClick={handleCreateTask} style={{ marginTop: '1rem' }}>
              Create your first task
            </button>
          </div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Title</th>
                <th>Slug</th>
                <th>Schedule</th>
                <th>Requested by</th>
                <th>Status</th>
                <th>Last run</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {tasks.map((task) => (
                <tr key={task.slug} onClick={() => handleViewTask(task.slug)}>
                  <td style={{ fontWeight: 500 }}>{task.title}</td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>{task.slug}</td>
                  <td style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>{getScheduleDisplay(task.schedule)}</td>
                  <td style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                    {'agent' in task.requester
                      ? <em style={{ color: 'var(--text-tertiary)' }}>own initiative</em>
                      : task.requester.user}
                  </td>
                  <td>
                    <span style={{
                      padding: '2px 8px',
                      borderRadius: '12px',
                      fontSize: '11px',
                      fontWeight: 600,
                      background: task.is_active ? '#e8f5e9' : '#ffebee',
                      color: task.is_active ? '#2e7d32' : '#c62828',
                    }}>
                      {task.is_active ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                  <td style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                    {task.last_run ? (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', alignItems: 'flex-start' }}>
                        <span>{new Date(task.last_run.ran_at).toLocaleString()}</span>
                        <RunStateBadge state={task.last_run.state} />
                      </div>
                    ) : (
                      // Never run is the absence of any run, not a state — and it reads as
                      // itself rather than as a blank cell or an error.
                      <em style={{ color: 'var(--text-tertiary)' }}>not yet run</em>
                    )}
                  </td>
                  <td>
                    <div style={{ display: 'flex', gap: '4px' }}>
                      <button
                        className="btn btn-ghost"
                        style={{ padding: '4px 6px' }}
                        onClick={(e) => { e.stopPropagation(); handleEditTask(task) }}
                      >
                        <FaPenToSquare size={14} />
                      </button>
                      <button
                        className="btn btn-ghost"
                        style={{ padding: '4px 6px', color: '#ef4444' }}
                        onClick={(e) => handleDeleteTask(task.slug, e)}
                      >
                        <FaTrash size={14} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* SlideOver */}
      <SlideOver
        open={modalMode !== null}
        onClose={closeModal}
        title={
          modalMode === 'view' ? selectedTask?.title ?? '' :
            modalMode === 'create' ? 'Create Task' :
              'Edit Task'
        }
      >
        {modalMode === 'view' && selectedTask && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem', flex: 1 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span style={{
                padding: '4px 12px', borderRadius: '12px', fontSize: '12px', fontWeight: 600,
                background: selectedTask.is_active ? '#e8f5e9' : '#ffebee',
                color: selectedTask.is_active ? '#2e7d32' : '#c62828',
              }}>
                {selectedTask.is_active ? 'Active' : 'Inactive'}
              </span>
              <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>{selectedTask.slug}</span>
            </div>
            <div>
              <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '4px' }}>Instruction</div>
              <div className="prose" style={{ whiteSpace: 'pre-wrap' }}>{selectedTask.instruction}</div>
            </div>
            <div>
              <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '4px' }}>Schedule</div>
              <div>{getScheduleDisplay(selectedTask.schedule)}</div>
            </div>
            <div>
              <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '4px' }}>Latest run</div>
              {!selectedTask.last_run ? (
                <div style={{ fontSize: '14px', color: 'var(--text-tertiary)', fontStyle: 'italic' }}>
                  This task has not run yet.
                </div>
              ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                    <RunStateBadge state={selectedTask.last_run.state} />
                    <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                      {new Date(selectedTask.last_run.ran_at).toLocaleString()}
                    </span>
                    {runDuration(selectedTask.last_run) && (
                      <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
                        took {runDuration(selectedTask.last_run)}
                      </span>
                    )}
                  </div>
                  {/* Three non-results, each reading as itself: still in flight, finished
                      having produced nothing, or stopped with the process. */}
                  {selectedTask.last_run.state === 'running' ? (
                    <div style={{ fontSize: '14px', color: 'var(--text-tertiary)', fontStyle: 'italic' }}>
                      Still running \u2014 its report will appear here when it finishes.
                    </div>
                  ) : selectedTask.last_run.state === 'interrupted' ? (
                    <div style={{ fontSize: '14px', color: 'var(--text-tertiary)', fontStyle: 'italic' }}>
                      Stopped before it finished, so there is no report. The task is free to run again.
                    </div>
                  ) : !selectedTask.last_run.response ? (
                    <div style={{ fontSize: '14px', color: 'var(--text-tertiary)', fontStyle: 'italic' }}>
                      This run produced no report. Expand it below to see what happened.
                    </div>
                  ) : (
                    <div className="prose" style={{ fontSize: '14px' }}>
                      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                        {selectedTask.last_run.response}
                      </ReactMarkdown>
                    </div>
                  )}
                </div>
              )}
            </div>

            <div>
              <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '4px' }}>
                Past runs
              </div>
              {runs.length === 0 && !runsLoading ? (
                <div style={{ fontSize: '14px', color: 'var(--text-tertiary)', fontStyle: 'italic' }}>
                  No runs recorded yet.
                </div>
              ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                  {runs.map((run) => {
                    const expanded = expandedRunId === run.run_id
                    return (
                      <div key={run.run_id} style={{ border: '1px solid var(--border)', borderRadius: '4px' }}>
                        <button
                          className="btn btn-ghost"
                          onClick={() => handleToggleRun(run)}
                          style={{
                            width: '100%', display: 'flex', alignItems: 'center', gap: '8px',
                            justifyContent: 'flex-start', padding: '8px', textAlign: 'left',
                          }}
                        >
                          {expanded ? <FaChevronDown size={11} /> : <FaChevronRight size={11} />}
                          <span style={{ fontSize: '13px' }}>{new Date(run.ran_at).toLocaleString()}</span>
                          <RunStateBadge state={run.state} />
                          {runDuration(run) && (
                            <span style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
                              {runDuration(run)}
                            </span>
                          )}
                        </button>
                        {expanded && (
                          <div style={{ padding: '0 8px 8px', borderTop: '1px solid var(--border)' }}>
                            {runHistoryLoading ? (
                              <Skeleton variant="text" width="80%" />
                            ) : runHistory.length === 0 ? (
                              <div style={{ fontSize: '13px', color: 'var(--text-tertiary)', fontStyle: 'italic', paddingTop: '8px' }}>
                                Nothing was recorded for this run.
                              </div>
                            ) : (
                              // Rendered through the same grouping the chat view uses. The
                              // run's opening request is not drawn as a bubble: the task's
                              // instruction is already above, and a second nameless copy of
                              // it would be the only thing that bubble could say.
                              groupHistory(runHistory).map((turn) => (
                                <div key={turn.key} style={{ paddingTop: '8px' }}>
                                  {turn.trail.length > 0 && (
                                    <ActivityTrail
                                      trail={turn.trail}
                                      live={false}
                                      durationMs={outcomeDurationMs(turn.outcome)}
                                      label={(name, args) => formatToolChoice(name, args, {})}
                                      variant="block"
                                    />
                                  )}
                                  {runOutcomeText(turn.outcome) && (
                                    <div className="prose" style={{ fontSize: '14px' }}>
                                      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                                        {runOutcomeText(turn.outcome)!}
                                      </ReactMarkdown>
                                    </div>
                                  )}
                                </div>
                              ))
                            )}
                          </div>
                        )}
                      </div>
                    )
                  })}
                  {/* Hidden once there is no further page, so a caller is never offered one
                      that does not exist. */}
                  {runsHasMore && (
                    <button
                      className="btn btn-secondary"
                      disabled={runsLoading}
                      onClick={() => loadRuns(selectedTask.slug, runs[runs.length - 1] ?? null)}
                      style={{ justifyContent: 'center' }}
                    >
                      {runsLoading ? 'Loading\u2026' : 'Load older runs'}
                    </button>
                  )}
                </div>
              )}
            </div>

            <div>
              <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '4px' }}>Details</div>
              <div style={{ fontSize: '14px', color: 'var(--text-secondary)' }}>
                <div>Requested by: {requesterLabel(selectedTask.requester)}</div>
                <div>Created: {new Date(selectedTask.timestamp).toLocaleString()}</div>
              </div>
            </div>
            <div style={{ display: 'flex', gap: '8px' }}>
              <button className="btn btn-secondary" onClick={() => handleEditTask(selectedTask)}>Edit</button>
              <button className="btn btn-ghost" onClick={(e) => handleDeleteTask(selectedTask.slug, e)} style={{ color: '#c00' }}>
                <FaTrash size={16} />
                <span>Delete</span>
              </button>
            </div>
          </div>
        )}

        {(modalMode === 'create' || modalMode === 'edit') && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', flex: 1, height: '100%' }}>
            <div className="input-group" style={{ marginBottom: 0 }}>
              <label htmlFor="slug">Slug</label>
              <input id="slug" type="text" value={formSlug} onChange={(e) => setFormSlug(autoCorrectSlug(e.target.value))} required disabled={modalMode === 'edit'} placeholder="my-task-slug" />
              {formSlug && <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px' }}>Slug: {formSlug}</div>}
            </div>
            <div className="input-group" style={{ marginBottom: 0 }}>
              <label htmlFor="title">Title</label>
              <input id="title" type="text" value={formTitle} onChange={(e) => setFormTitle(e.target.value)} required />
            </div>
            <div className="input-group" style={{ marginBottom: 0 }}>
              <label htmlFor="schedule-type">Schedule Type</label>
              <select
                id="schedule-type"
                value={formScheduleType}
                onChange={(e) => setFormScheduleType(e.target.value as ScheduleType)}
                style={{ padding: '8px 16px', borderRadius: '4px', border: '1px solid var(--border)', background: 'var(--background)' }}
              >
                <option value="Cron">Cron (Recurring)</option>
                <option value="OneTime">One-Time</option>
              </select>
            </div>
            {formScheduleType === 'Cron' ? (
              <>
                <div className="input-group" style={{ marginBottom: 0 }}>
                  <label htmlFor="cron-template">Template</label>
                  <select
                    id="cron-template"
                    value={CRON_TEMPLATES.find(t => t.value === formScheduleValue)?.value ?? ''}
                    onChange={(e) => { if (e.target.value) setFormScheduleValue(e.target.value) }}
                    style={{ padding: '8px 16px', borderRadius: '4px', border: '1px solid var(--border)', background: 'var(--background)' }}
                  >
                    {CRON_TEMPLATES.map(t => <option key={t.value} value={t.value}>{t.label}</option>)}
                  </select>
                </div>
                <div className="input-group" style={{ marginBottom: 0 }}>
                  <label htmlFor="schedule-value">Cron Expression</label>
                  <input id="schedule-value" type="text" value={formScheduleValue} onChange={(e) => setFormScheduleValue(e.target.value)} required placeholder="0 0 * * *" />
                  <p style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                    Example: "0 0 * * *" (daily at midnight)
                  </p>
                </div>
              </>
            ) : (
              <DatePicker label="Datetime (UTC)" value={formScheduleValue} onChange={setFormScheduleValue} />
            )}
            <div className="input-group" style={{ marginBottom: 0, height: '100%', overflow: 'hidden' }}>
              <label htmlFor="instruction">Instruction</label>
              <div style={{ height: '100%', overflowY: 'hidden' }}>
                <MarkdownEditor value={formInstruction} onChange={setFormInstruction} placeholder="Enter task instruction..." className="modal-mdx-editor" />
              </div>
            </div>
            <div style={{ display: 'flex', gap: '8px', marginTop: '0.5rem' }}>
              <button className="btn btn-primary" onClick={handleSubmit} disabled={!formSlug.trim() || !formTitle.trim() || !formInstruction.trim() || !formScheduleValue.trim() || submitting} style={{ flex: 1, justifyContent: 'center' }}>
                {submitting ? 'Saving...' : 'Save'}
              </button>
              <button className="btn btn-secondary" onClick={closeModal} disabled={submitting}>Cancel</button>
            </div>
          </div>
        )}
      </SlideOver>
    </>
  )
}
