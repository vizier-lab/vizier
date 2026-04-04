import { useState, FormEvent, useEffect } from 'react'
import { useNavigate } from 'react-router'
import { login } from '../services/vizier'
import { useToastStore } from '../hooks/toastStore'
import ToastContainer from '../components/Toast'

export default function Login() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()
  const { addToast } = useToastStore()

  // Check if already logged in
  useEffect(() => {
    const token = localStorage.getItem('auth_token')
    if (token) {
      navigate('/')
    }
  }, [navigate])

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError('')
    setLoading(true)

    try {
      await login(username, password)
      addToast('success', 'Welcome back!', 'Successfully logged in')
      navigate('/')
    } catch (err: any) {
      const errorMsg = err.response?.data?.message || 'Login failed. Please try again.'
      setError(errorMsg)
      addToast('error', 'Login failed', errorMsg)
    } finally {
      setLoading(false)
    }
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
        {/* Background gradient decoration */}
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
            <div style={{
              width: '64px',
              height: '64px',
              margin: '0 auto 1.5rem',
              background: 'linear-gradient(135deg, #10B981 0%, #14B8A6 100%)',
              borderRadius: '16px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              boxShadow: '0 8px 16px rgba(16, 185, 129, 0.3)',
            }}>
              <span style={{
                fontSize: '32px',
                color: 'white',
              }}>🔮</span>
            </div>
            <h1 style={{
              fontSize: '2rem',
              fontWeight: '700',
              marginBottom: '0.5rem',
              background: 'linear-gradient(135deg, var(--text-primary) 0%, var(--text-secondary) 100%)',
              WebkitBackgroundClip: 'text',
              WebkitTextFillColor: 'transparent',
              backgroundClip: 'text',
            }}>Vizier</h1>
            <p style={{
              color: 'var(--text-secondary)',
              fontSize: '14px',
            }}>Sign in to your digital steward</p>
          </div>

          {/* Login Form */}
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
                placeholder="Enter your username"
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
                placeholder="Enter your password"
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
                  Signing in...
                </span>
              ) : (
                'Sign In'
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
