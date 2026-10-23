/**
 * Decode JWT token to extract user information
 * Note: This does NOT validate the token - validation happens on the backend
 */
export function decodeJWT(token: string): { sub: string; username: string; permissions: string[] } | null {
  try {
    const base64Url = token.split('.')[1]
    if (!base64Url) return null
    
    const base64 = base64Url.replace(/-/g, '+').replace(/_/g, '/')
    const jsonPayload = decodeURIComponent(
      atob(base64)
        .split('')
        .map((c) => '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2))
        .join('')
    )
    
    return JSON.parse(jsonPayload)
  } catch (error) {
    console.error('Failed to decode JWT:', error)
    return null
  }
}

/**
 * Get the current username from the JWT token or return default
 */
export function getCurrentUsername(): string {
  const token = localStorage.getItem('auth_token')
  if (!token) return 'user'
  
  const decoded = decodeJWT(token)
  return decoded?.username || 'user'
}

/**
 * Check if the current user has a specific permission
 */
export function hasPermission(permission: string): boolean {
  const token = localStorage.getItem('auth_token')
  if (!token) return false
  
  const decoded = decodeJWT(token)
  if (!decoded?.permissions) return false
  
  return decoded.permissions.includes(permission)
}

/**
 * Check if the current user has any of the specified permissions
 */
export function hasAnyPermission(permissions: string[]): boolean {
  return permissions.some(p => hasPermission(p))
}

/**
 * Get all permissions for the current user
 */
export function getCurrentPermissions(): string[] {
  const token = localStorage.getItem('auth_token')
  if (!token) return []
  
  const decoded = decodeJWT(token)
  return decoded?.permissions || []
}
