import { useEffect, useState } from 'react'
import { useParams } from 'react-router'
import { listTasks, getTask, createTask, updateTask, deleteTask } from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import { FaPlus, FaTrash, FaClock } from 'react-icons/fa6'
import type { Task } from '../interfaces/types'
import DatePicker from '../components/DatePicker'

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
  const [tasks, setTasks] = useState<Task[]>([])
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [loading, setLoading] = useState(true)
  const [modalMode, setModalMode] = useState<ModalMode>(null)
  const [filterActive, setFilterActive] = useState<boolean | undefined>(undefined)

  // Form state
  const [formSlug, setFormSlug] = useState('')
  const [formUser, setFormUser] = useState('user')
  const [formTitle, setFormTitle] = useState('')
  const [formInstruction, setFormInstruction] = useState('')
  const [formScheduleType, setFormScheduleType] = useState<ScheduleType>('Cron')
  const [formScheduleValue, setFormScheduleValue] = useState('')
  const [submitting, setSubmitting] = useState(false)

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
    } catch (error) {
      console.error('Failed to load task:', error)
    }
  }

  const handleEditTask = (task: Task) => {
    setSelectedTask(task)
    setFormSlug(task.slug)
    setFormUser(task.user)
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
    setFormUser('user')
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
      // Apply strict validation to finalize slug
      const finalSlug = autoCorrectSlugStrict(formSlug)
      if (!finalSlug) return

      const taskData = {
        slug: finalSlug,
        user: formUser,
        title: formTitle,
        instruction: formInstruction,
        schedule: formScheduleType === 'Cron'
          ? { type: 'Cron' as const, expression: formScheduleValue }
          : { type: 'OneTime' as const, datetime: formScheduleValue }
      }

      if (modalMode === 'create') {
        await createTask(agentId, taskData)
      } else if (modalMode === 'edit' && selectedTask) {
        await updateTask(agentId, selectedTask.slug, taskData)
      }

      await loadTasks()
      closeModal()
    } catch (error) {
      console.error('Failed to save task:', error)
      alert('Failed to save task')
    } finally {
      setSubmitting(false)
    }
  }

  const handleDeleteTask = async (slug: string) => {
    if (!agentId) return
    if (!confirm('Are you sure you want to delete this task?')) return

    try {
      await deleteTask(agentId, slug)
      await loadTasks()
      closeModal()
    } catch (error) {
      console.error('Failed to delete task:', error)
      alert('Failed to delete task')
    }
  }

  const closeModal = () => {
    setModalMode(null)
    setSelectedTask(null)
    setFormSlug('')
    setFormUser('user')
    setFormTitle('')
    setFormInstruction('')
    setFormScheduleType('Cron')
    setFormScheduleValue('')
  }

  const getScheduleDisplay = (schedule: Task['schedule']) => {
    if ('CronTask' in schedule) {
      return `Cron: ${schedule.CronTask}`
    } else if ('OneTimeTask' in schedule) {
      return `One-time: ${new Date(schedule.OneTimeTask).toLocaleString()}`
    }
    return 'Unknown'
  }

  if (loading) {
    return (
      <div className="main-body" style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}>
        Loading tasks...
      </div>
    )
  }

  return (
    <>
      {/* Header */}
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
            style={{
              padding: '8px 16px',
              borderRadius: '4px',
              border: '1px solid var(--border)',
              background: 'var(--background)',
            }}
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

      {/* Task List */}
      <div className="main-body">
        {tasks.length === 0 ? (
          <div style={{
            textAlign: 'center',
            color: 'var(--text-tertiary)',
            padding: '3rem',
          }}>
            <p>No tasks yet.</p>
            <button
              className="btn btn-primary"
              onClick={handleCreateTask}
              style={{ marginTop: '1rem' }}
            >
              Create your first task
            </button>
          </div>
        ) : (
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '1rem',
          }}>
            {tasks.map((task) => (
              <div
                key={task.slug}
                className="card"
                style={{
                  cursor: 'pointer',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'flex-start',
                }}
                onClick={() => handleViewTask(task.slug)}
              >
                <div style={{ flex: 1 }}>
                  <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '8px',
                    marginBottom: '0.5rem',
                  }}>
                    <h4 style={{ margin: 0 }}>{task.title}</h4>
                    <span style={{
                      padding: '2px 8px',
                      borderRadius: '12px',
                      fontSize: '11px',
                      fontWeight: '600',
                      background: task.is_active ? '#e8f5e9' : '#ffebee',
                      color: task.is_active ? '#2e7d32' : '#c62828',
                    }}>
                      {task.is_active ? 'Active' : 'Inactive'}
                    </span>
                  </div>
                  <p style={{
                    fontSize: '14px',
                    color: 'var(--text-secondary)',
                    marginBottom: '0.5rem',
                  }}>
                    {task.instruction}
                  </p>
                  <div style={{
                    fontSize: '12px',
                    color: 'var(--text-tertiary)',
                  }}>
                    <div>{getScheduleDisplay(task.schedule)}</div>
                    {task.last_executed_at && (
                      <div>Last executed: {new Date(task.last_executed_at).toLocaleString()}</div>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Modal */}
      {modalMode && (
        <>
          {/* Backdrop */}
          <div
            style={{
              position: 'fixed',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: 'rgba(0, 0, 0, 0.5)',
              zIndex: 1000,
            }}
            onClick={closeModal}
          />

          {/* Modal Content */}
          <div
            style={{
              position: 'fixed',
              top: '50%',
              left: '50%',
              transform: 'translate(-50%, -50%)',
              background: 'var(--background)',
              borderRadius: '8px',
              padding: '2rem',
              maxWidth: '700px',
              width: '90%',
              maxHeight: '80vh',
              overflow: 'auto',
              zIndex: 1001,
              border: '1px solid var(--border)',
            }}
          >
            {modalMode === 'view' && selectedTask && (
              <>
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'flex-start',
                  marginBottom: '1.5rem',
                }}>
                  <div style={{ flex: 1 }}>
                    <div style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      marginBottom: '0.5rem',
                    }}>
                      <h2 style={{ margin: 0 }}>{selectedTask.title}</h2>
                      <span style={{
                        padding: '4px 12px',
                        borderRadius: '12px',
                        fontSize: '12px',
                        fontWeight: '600',
                        background: selectedTask.is_active ? '#e8f5e9' : '#ffebee',
                        color: selectedTask.is_active ? '#2e7d32' : '#c62828',
                      }}>
                        {selectedTask.is_active ? 'Active' : 'Inactive'}
                      </span>
                    </div>
                    <p style={{
                      fontSize: '12px',
                      color: 'var(--text-tertiary)',
                    }}>
                      {selectedTask.slug}
                    </p>
                  </div>
                  <button className="btn btn-ghost" onClick={closeModal}>✕</button>
                </div>

                <div style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '1rem',
                  marginBottom: '1.5rem',
                }}>
                  <div>
                    <div style={{
                      fontSize: '12px',
                      fontWeight: '600',
                      color: 'var(--text-secondary)',
                      marginBottom: '4px',
                    }}>
                      Instruction
                    </div>
                    <div className="prose" style={{
                      whiteSpace: 'pre-wrap',
                    }}>
                      {selectedTask.instruction}
                    </div>
                  </div>

                  <div>
                    <div style={{
                      fontSize: '12px',
                      fontWeight: '600',
                      color: 'var(--text-secondary)',
                      marginBottom: '4px',
                    }}>
                      Schedule
                    </div>
                    <div>{getScheduleDisplay(selectedTask.schedule)}</div>
                  </div>

                  <div>
                    <div style={{
                      fontSize: '12px',
                      fontWeight: '600',
                      color: 'var(--text-secondary)',
                      marginBottom: '4px',
                    }}>
                      Details
                    </div>
                    <div style={{ fontSize: '14px', color: 'var(--text-secondary)' }}>
                      <div>User: {selectedTask.user}</div>
                      <div>Created: {new Date(selectedTask.timestamp).toLocaleString()}</div>
                      {selectedTask.last_executed_at && (
                        <div>Last executed: {new Date(selectedTask.last_executed_at).toLocaleString()}</div>
                      )}
                    </div>
                  </div>
                </div>

                <div style={{ display: 'flex', gap: '8px' }}>
                  <button
                    className="btn btn-secondary"
                    onClick={() => handleEditTask(selectedTask)}
                  >
                    Edit
                  </button>
                  <button
                    className="btn btn-ghost"
                    onClick={() => handleDeleteTask(selectedTask.slug)}
                    style={{ color: '#c00' }}
                  >
                    <FaTrash size={16} />
                    <span>Delete</span>
                  </button>
                </div>
              </>
            )}

            {(modalMode === 'create' || modalMode === 'edit') && (
              <>
                <div style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: '1.5rem',
                }}>
                  <h2>{modalMode === 'create' ? 'Create Task' : 'Edit Task'}</h2>
                  <button className="btn btn-ghost" onClick={closeModal}>✕</button>
                </div>

                <div style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '1rem',
                }}>
                  <div className="input-group">
                    <label htmlFor="slug">Slug</label>
                    <input
                      id="slug"
                      type="text"
                      value={formSlug}
                      onChange={(e) => setFormSlug(autoCorrectSlug(e.target.value))}
                      required
                      disabled={modalMode === 'edit'}
                      placeholder="my-task-slug"
                    />
                    {formSlug && (
                      <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                        Slug: {formSlug}
                      </div>
                    )}
                  </div>

                  <div className="input-group">
                    <label htmlFor="user">User</label>
                    <input
                      id="user"
                      type="text"
                      value={formUser}
                      onChange={(e) => setFormUser(e.target.value)}
                      required
                    />
                  </div>

                  <div className="input-group">
                    <label htmlFor="title">Title</label>
                    <input
                      id="title"
                      type="text"
                      value={formTitle}
                      onChange={(e) => setFormTitle(e.target.value)}
                      required
                    />
                  </div>

                  <div className="input-group">
                    <label htmlFor="instruction">Instruction</label>
                    <textarea
                      id="instruction"
                      value={formInstruction}
                      onChange={(e) => setFormInstruction(e.target.value)}
                      required
                      rows={5}
                    />
                  </div>

                  <div className="input-group">
                    <label htmlFor="schedule-type">Schedule Type</label>
                    <select
                      id="schedule-type"
                      value={formScheduleType}
                      onChange={(e) => setFormScheduleType(e.target.value as ScheduleType)}
                      style={{
                        padding: '8px 16px',
                        borderRadius: '4px',
                        border: '1px solid var(--border)',
                        background: 'var(--background)',
                      }}
                    >
                      <option value="Cron">Cron (Recurring)</option>
                      <option value="OneTime">One-Time</option>
                    </select>
                  </div>

                  {formScheduleType === 'Cron' ? (
                    <>
                      <div className="input-group">
                        <label htmlFor="cron-template">Template</label>
                        <select
                          id="cron-template"
                          value={CRON_TEMPLATES.find(t => t.value === formScheduleValue)?.value ?? ''}
                          onChange={(e) => {
                            if (e.target.value) {
                              setFormScheduleValue(e.target.value)
                            }
                          }}
                          style={{
                            padding: '8px 16px',
                            borderRadius: '4px',
                            border: '1px solid var(--border)',
                            background: 'var(--background)',
                          }}
                        >
                          {CRON_TEMPLATES.map(t => (
                            <option key={t.value} value={t.value}>{t.label}</option>
                          ))}
                        </select>
                      </div>

                      <div className="input-group">
                        <label htmlFor="schedule-value">Cron Expression</label>
                        <input
                          id="schedule-value"
                          type="text"
                          value={formScheduleValue}
                          onChange={(e) => setFormScheduleValue(e.target.value)}
                          required
                          placeholder="0 0 * * *"
                        />
                        <p style={{
                          fontSize: '12px',
                          color: 'var(--text-tertiary)',
                          marginTop: '4px',
                        }}>
                          {formScheduleType === 'Cron'
                            ? 'Example: "0 0 * * *" (daily at midnight)'
                            : 'Example: "2026-04-04T14:00:00Z"'
                          }
                        </p>
                      </div>
                    </>
                  ) : (
                    <DatePicker
                      label="Datetime (UTC)"
                      value={formScheduleValue}
                      onChange={setFormScheduleValue}
                    />
                  )}

                  <div style={{ display: 'flex', gap: '8px', marginTop: '0.5rem' }}>
                    <button
                      className="btn btn-primary"
                      onClick={handleSubmit}
                      disabled={!formSlug.trim() || !formTitle.trim() || !formInstruction.trim() || !formScheduleValue.trim() || submitting}
                    >
                      {submitting ? 'Saving...' : 'Save'}
                    </button>
                    <button
                      className="btn btn-secondary"
                      onClick={closeModal}
                      disabled={submitting}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </>
            )}
          </div>
        </>
      )}
    </>
  )
}
