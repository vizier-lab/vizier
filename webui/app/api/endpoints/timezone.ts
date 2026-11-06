import { api } from '../client'
import type { InstanceZone } from '../../lib/timezone'

/** The zone the instance runs in; every time on screen is shown in it. */
export const getInstanceZone = () => api.get<InstanceZone>('/utils/timezone')
