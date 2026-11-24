import { createContext, useContext } from 'react'
import type { SessionFileRecord } from '../../../api/frames'

/** The files of the conversation an interface is drawn in, newest state first. */
export const MediaFiles = createContext<readonly SessionFileRecord[]>([])

export const useSessionFiles = () => useContext(MediaFiles)
