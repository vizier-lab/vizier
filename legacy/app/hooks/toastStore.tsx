import { create } from 'zustand'

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface Toast {
  id: string
  type: ToastType
  message: string
  description?: string
}

interface ToastState {
  toasts: Toast[]
  addToast: (type: ToastType, message: string, description?: string) => void
  removeToast: (id: string) => void
}

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  addToast: (type, message, description) => {
    const id = Date.now().toString()
    set((state) => ({
      toasts: [...state.toasts, { id, type, message, description }],
    }))
    // Auto-remove after 5 seconds
    setTimeout(() => {
      get().removeToast(id)
    }, 5000)
  },
  removeToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((toast) => toast.id !== id),
    }))
  },
}))
