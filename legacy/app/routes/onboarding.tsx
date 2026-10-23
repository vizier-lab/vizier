import { useState, useEffect } from 'react'
import type { FormEvent } from 'react'
import { useNavigate } from 'react-router'
import { setupFirstUser, getSetupStatus } from '../services/vizier'
import { useToastStore } from '../hooks/toastStore'
import { useThemeStore } from '../hooks/themeStore'
import ToastContainer from '../components/Toast'
import ThemeToggle from '../components/ThemeToggle'

export default function Onboarding() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const [checking, setChecking] = useState(true)
  const navigate = useNavigate()
  const { addToast } = useToastStore()
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme)

  // Check if setup is needed
  useEffect(() => {
    const checkSetup = async () => {
      try {
        const res = await getSetupStatus()
        if (!res.data?.needs_setup) {
          // Setup already completed, redirect to login
          navigate('/login')
        }
      } catch (err) {
        // If we can't check, assume setup is needed
        console.error('Failed to check setup status:', err)
      } finally {
        setChecking(false)
      }
    }
    checkSetup()
  }, [navigate])

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError('')

    if (password !== confirmPassword) {
      setError('Passwords do not match')
      return
    }

    if (password.length < 8) {
      setError('Password must be at least 8 characters')
      return
    }

    setLoading(true)

    try {
      await setupFirstUser(username, password)
      addToast('success', 'Welcome!', 'Your account has been created successfully')
      navigate('/')
    } catch (err: any) {
      const errorMsg = err.response?.data?.message || 'Setup failed. Please try again.'
      setError(errorMsg)
      addToast('error', 'Setup failed', errorMsg)
    } finally {
      setLoading(false)
    }
  }

  if (checking) {
    return (
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100vh',
        background: 'var(--background)',
      }}>
        <div className="thinking-dots">
          <span>.</span><span>.</span><span>.</span>
        </div>
      </div>
    )
  }

  return (
    <>
      <ToastContainer />
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100vh',
        background: 'var(--background)',
        position: 'relative',
        overflow: 'hidden',
      }}>
        <div style={{
          position: 'absolute',
          top: '1rem',
          right: '1rem',
          zIndex: 2,
        }}>
          <ThemeToggle />
        </div>

        <div style={{
          position: 'absolute',
          top: '-50%',
          right: '-20%',
          width: '800px',
          height: '800px',
          background: 'radial-gradient(circle, rgba(16, 185, 129, 0.1) 0%, transparent 70%)',
          borderRadius: '50%',
          pointerEvents: 'none',
        }} />
        <div style={{
          position: 'absolute',
          bottom: '-30%',
          left: '-10%',
          width: '600px',
          height: '600px',
          background: 'radial-gradient(circle, rgba(20, 184, 166, 0.08) 0%, transparent 70%)',
          borderRadius: '50%',
          pointerEvents: 'none',
        }} />

        <div style={{
          width: '100%',
          maxWidth: '420px',
          padding: '2.5rem',
          position: 'relative',
          zIndex: 1,
        }}>
          {/* Logo/Header */}
          <div style={{
            textAlign: 'center',
            marginBottom: '2.5rem',
          }}>
            <img
              src={`/vizier-logo-${resolvedTheme}.svg`}
              alt="Vizier"
              style={{
                width: '64px',
                height: '64px',
                margin: '0 auto 1.5rem',
                display: 'block',
              }}
            />
            <h1 style={{
              fontSize: '2rem',
              fontWeight: '700',
              marginBottom: '0.5rem',
              background: 'linear-gradient(135deg, var(--text-primary) 0%, var(--text-secondary) 100%)',
              WebkitBackgroundClip: 'text',
              WebkitTextFillColor: 'transparent',
              backgroundClip: 'text',
            }}>Welcome to Vizier</h1>
            <p style={{
              color: 'var(--text-secondary)',
              fontSize: '14px',
            }}>Create your admin account to get started</p>
          </div>

          {/* Onboarding Form */}
          <form onSubmit={handleSubmit} style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '1.25rem',
          }}>
            {error && (
              <div style={{
                padding: '12px 16px',
                background: 'rgba(239, 68, 68, 0.1)',
                border: '1px solid rgba(239, 68, 68, 0.3)',
                borderRadius: '8px',
                color: '#ef4444',
                fontSize: '14px',
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
              }}>
                <span>⚠️</span>
                {error}
              </div>
            )}

            <div className="input-group">
              <label htmlFor="username" style={{
                fontSize: '14px',
                fontWeight: '600',
                color: 'var(--text-primary)',
              }}>Username</label>
              <input
                id="username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                required
                autoFocus
                disabled={loading}
                placeholder="Choose a username"
                style={{
                  padding: '12px 16px',
                  fontSize: '15px',
                }}
              />
            </div>

            <div className="input-group">
              <label htmlFor="password" style={{
                fontSize: '14px',
                fontWeight: '600',
                color: 'var(--text-primary)',
              }}>Password</label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                disabled={loading}
                placeholder="Create a password (min 8 characters)"
                style={{
                  padding: '12px 16px',
                  fontSize: '15px',
                }}
              />
            </div>

            <div className="input-group">
              <label htmlFor="confirmPassword" style={{
                fontSize: '14px',
                fontWeight: '600',
                color: 'var(--text-primary)',
              }}>Confirm Password</label>
              <input
                id="confirmPassword"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                required
                disabled={loading}
                placeholder="Confirm your password"
                style={{
                  padding: '12px 16px',
                  fontSize: '15px',
                }}
              />
            </div>

            <button
              type="submit"
              className="btn btn-primary"
              disabled={loading}
              style={{
                width: '100%',
                justifyContent: 'center',
                padding: '14px',
                fontSize: '15px',
                fontWeight: '600',
                marginTop: '0.5rem',
              }}
            >
              {loading ? (
                <span style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                }}>
                  <span className="thinking-dots">
                    <span>.</span><span>.</span><span>.</span>
                  </span>
                  Creating account...
                </span>
              ) : (
                'Create Account'
              )}
            </button>
          </form>

          {/* Footer */}
          <div style={{
            marginTop: '2rem',
            textAlign: 'center',
            fontSize: '12px',
            color: 'var(--text-tertiary)',
          }}>
            <p>21st Century Digital Steward</p>
          </div>
        </div>
      </div>
    </>
  )
}
