import { useState } from 'react'
import { FaChevronLeft, FaChevronRight } from 'react-icons/fa6'

interface DatePickerProps {
  value: string
  onChange: (value: string) => void
  label?: string
}

const DAYS = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa']
const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December'
]

function formatDateTime(year: number, month: number, day: number, hour: number, minute: number): string {
  const pad = (n: number) => n.toString().padStart(2, '0')
  return `${year}-${pad(month + 1)}-${pad(day)}T${pad(hour)}:${pad(minute)}:00Z`
}

function parseValue(value: string): { year: number; month: number; day: number; hour: number; minute: number } | null {
  if (!value) return null
  const d = new Date(value)
  if (isNaN(d.getTime())) return null
  return {
    year: d.getUTCFullYear(),
    month: d.getUTCMonth(),
    day: d.getUTCDate(),
    hour: d.getUTCHours(),
    minute: d.getUTCMinutes()
  }
}

export default function DatePicker({ value, onChange, label }: DatePickerProps) {
  const now = new Date()
  const initial = parseValue(value)

  const [selectedDate, setSelectedDate] = useState(() => {
    if (initial) return initial
    return {
      year: now.getUTCFullYear(),
      month: now.getUTCMonth(),
      day: now.getUTCDate(),
      hour: 9,
      minute: 0
    }
  })

  const [isOpen, setIsOpen] = useState(false)

  const currentMonthDays = new Date(selectedDate.year, selectedDate.month + 1, 0).getDate()
  const firstDayOfMonth = new Date(selectedDate.year, selectedDate.month, 1).getDay()

  const prevMonth = () => {
    setSelectedDate(prev => {
      const newMonth = prev.month - 1
      if (newMonth < 0) {
        return { ...prev, month: 11, year: prev.year - 1 }
      }
      return { ...prev, month: newMonth }
    })
  }

  const nextMonth = () => {
    setSelectedDate(prev => {
      const newMonth = prev.month + 1
      if (newMonth > 11) {
        return { ...prev, month: 0, year: prev.year + 1 }
      }
      return { ...prev, month: newMonth }
    })
  }

  const selectDay = (day: number) => {
    const newDate = { ...selectedDate, day }
    setSelectedDate(newDate)
    onChange(formatDateTime(newDate.year, newDate.month, newDate.day, newDate.hour, newDate.minute))
  }

  const handleTimeChange = (field: 'hour' | 'minute', rawValue: string) => {
    const num = parseInt(rawValue, 10)
    if (isNaN(num)) return
    
    const newDate = { ...selectedDate }
    if (field === 'hour') {
      newDate.hour = Math.max(0, Math.min(23, num))
    } else {
      newDate.minute = Math.max(0, Math.min(59, num))
    }
    setSelectedDate(newDate)
    onChange(formatDateTime(newDate.year, newDate.month, newDate.day, newDate.hour, newDate.minute))
  }

  const displayValue = value ? (() => {
    const d = new Date(value)
    return d.toLocaleString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      hour12: false,
      timeZone: 'UTC'
    }) + ' UTC'
  })() : 'Select date and time'

  return (
    <div className="input-group">
      {label && <label>{label}</label>}
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        style={{
          width: '100%',
          padding: '8px 16px',
          borderRadius: '4px',
          border: '1px solid var(--border)',
          background: 'var(--background)',
          color: value ? 'var(--text)' : 'var(--text-tertiary)',
          textAlign: 'left',
          cursor: 'pointer',
          fontSize: '14px',
        }}
      >
        {displayValue}
      </button>

      {isOpen && (
        <div
          style={{
            position: 'absolute',
            marginTop: '4px',
            padding: '16px',
            background: 'var(--background)',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
            zIndex: 1000,
          }}
        >
          <div style={{ display: 'flex', gap: '24px' }}>
            <div>
              <div style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                marginBottom: '8px',
                width: '252px'
              }}>
                <button
                  type="button"
                  onClick={prevMonth}
                  style={{
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    padding: '4px',
                    color: 'var(--text-secondary)',
                  }}
                >
                  <FaChevronLeft size={20} />
                </button>
                <span style={{ fontWeight: '600', color: 'var(--text)' }}>
                  {MONTHS[selectedDate.month]} {selectedDate.year}
                </span>
                <button
                  type="button"
                  onClick={nextMonth}
                  style={{
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    padding: '4px',
                    color: 'var(--text-secondary)',
                  }}
                >
                  <FaChevronRight size={20} />
                </button>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(7, 36px)', gap: '0' }}>
                {DAYS.map(day => (
                  <div
                    key={day}
                    style={{
                      textAlign: 'center',
                      padding: '4px',
                      fontSize: '12px',
                      fontWeight: '600',
                      color: 'var(--text-tertiary)',
                    }}
                  >
                    {day}
                  </div>
                ))}

                {Array.from({ length: firstDayOfMonth }, (_, i) => (
                  <div key={`empty-${i}`} style={{ width: '36px', height: '36px' }} />
                ))}

                {Array.from({ length: currentMonthDays }, (_, i) => {
                  const day = i + 1
                  const isSelected = day === selectedDate.day
                  return (
                    <button
                      key={day}
                      type="button"
                      onClick={() => selectDay(day)}
                      style={{
                        width: '36px',
                        height: '36px',
                        border: 'none',
                        borderRadius: '4px',
                        cursor: 'pointer',
                        fontSize: '14px',
                        background: isSelected ? 'var(--primary)' : 'transparent',
                        color: isSelected ? 'white' : 'var(--text)',
                      }}
                    >
                      {day}
                    </button>
                  )
                })}
              </div>
            </div>

            <div style={{
              borderLeft: '1px solid var(--border)',
              paddingLeft: '24px',
            }}>
              <div style={{
                fontSize: '12px',
                fontWeight: '600',
                color: 'var(--text-secondary)',
                marginBottom: '8px'
              }}>
                Time (UTC)
              </div>
              <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                <input
                  type="number"
                  min="0"
                  max="23"
                  value={selectedDate.hour.toString().padStart(2, '0')}
                  onChange={(e) => handleTimeChange('hour', e.target.value)}
                  style={{
                    width: '50px',
                    padding: '8px',
                    borderRadius: '4px',
                    border: '1px solid var(--border)',
                    background: 'var(--background)',
                    color: 'var(--text)',
                    fontSize: '14px',
                    textAlign: 'center',
                  }}
                />
                <span style={{ color: 'var(--text)' }}>:</span>
                <input
                  type="number"
                  min="0"
                  max="59"
                  value={selectedDate.minute.toString().padStart(2, '0')}
                  onChange={(e) => handleTimeChange('minute', e.target.value)}
                  style={{
                    width: '50px',
                    padding: '8px',
                    borderRadius: '4px',
                    border: '1px solid var(--border)',
                    background: 'var(--background)',
                    color: 'var(--text)',
                    fontSize: '14px',
                    textAlign: 'center',
                  }}
                />
              </div>

              <div style={{ marginTop: '16px' }}>
                <div style={{
                  fontSize: '11px',
                  color: 'var(--text-tertiary)',
                  marginBottom: '8px'
                }}>
                  Quick select
                </div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                  {[
                    { label: '09:00', hour: 9, minute: 0 },
                    { label: '12:00', hour: 12, minute: 0 },
                    { label: '18:00', hour: 18, minute: 0 },
                    { label: '00:00', hour: 0, minute: 0 },
                  ].map(({ label, hour, minute }) => (
                    <button
                      key={label}
                      type="button"
                      onClick={() => {
                        const newDate = { ...selectedDate, hour, minute }
                        setSelectedDate(newDate)
                        onChange(formatDateTime(newDate.year, newDate.month, newDate.day, newDate.hour, newDate.minute))
                      }}
                      style={{
                        padding: '4px 8px',
                        borderRadius: '4px',
                        border: '1px solid var(--border)',
                        background: 'transparent',
                        color: 'var(--text-secondary)',
                        cursor: 'pointer',
                        fontSize: '12px',
                      }}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>

          <div style={{
            marginTop: '12px',
            paddingTop: '12px',
            borderTop: '1px solid var(--border)',
            display: 'flex',
            justifyContent: 'flex-end'
          }}>
            <button
              type="button"
              onClick={() => setIsOpen(false)}
              style={{
                padding: '6px 16px',
                borderRadius: '4px',
                border: 'none',
                background: 'var(--primary)',
                color: 'white',
                cursor: 'pointer',
                fontSize: '14px',
              }}
            >
              Done
            </button>
          </div>
        </div>
      )}
    </div>
  )
}