// The signed-in person's connected Discord and Telegram accounts (spec 069).
import { api } from '../client'
import type { components } from '../schema'

export type Platform = components['schemas']['ChannelPlatform']
export type ConnectedAccount = components['schemas']['ConnectedAccount']
export type UnverifiedAccount = components['schemas']['UnverifiedAccount']
export type Connections = components['schemas']['Connections']
export type ConnectPreview = components['schemas']['ConnectPreview']
export type ConnectResult = components['schemas']['ConnectResult']

export const listConnections = () => api.get<Connections>('/auth/users/me/connections')

/** What confirming the code would connect. The code is not used up. */
export const previewConnection = (code: string) =>
    api.post<ConnectPreview>('/auth/users/me/connections/preview', { code })

export const redeemConnection = (code: string) => api.post<ConnectResult>('/auth/users/me/connections', { code })

export const unlinkConnection = (platform: Platform) => api.delete<void>(`/auth/users/me/connections/${platform}`)

/** Remove an id typed in before accounts could be proven. */
export const removeUnverified = (platform: Platform) =>
    api.delete<void>(`/auth/users/me/connections/${platform}/unverified`)
