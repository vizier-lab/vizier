import { create } from 'zustand'

export type ToastTone = 'success' | 'error' | 'warning' | 'info'
export type ToastItem = { id: number; tone: ToastTone; title: string; message?: string }

type ToastStore = {
    items: ToastItem[]
    push: (t: Omit<ToastItem, 'id'>, ms?: number) => void
    dismiss: (id: number) => void
}

let next = 1

export const useToasts = create<ToastStore>()((set, get) => ({
    items: [],
    push: (t, ms = 5000) => {
        const id = next++
        set((s) => ({ items: [...s.items.slice(-3), { ...t, id }] }))
        if (ms > 0) setTimeout(() => get().dismiss(id), ms)
    },
    dismiss: (id) => set((s) => ({ items: s.items.filter((i) => i.id !== id) })),
}))

export const toast = {
    success: (title: string, message?: string) => useToasts.getState().push({ tone: 'success', title, message }),
    error: (title: string, message?: string) => useToasts.getState().push({ tone: 'error', title, message }, 8000),
    info: (title: string, message?: string) => useToasts.getState().push({ tone: 'info', title, message }),
}
