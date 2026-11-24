import type { ReactNode } from 'react'
import { motion } from 'motion/react'
import { motionDurations, motionEases } from '@vizier/design'

/**
 * Motion for the app's own composition: things that arrive (a message, a row,
 * a panel) rise into place; things that leave fade out. The design library
 * owns the surfaces; these are the pieces the app composes around them.
 *
 * Every animation here is transform and opacity only, and the app's
 * MotionProvider honours prefers-reduced-motion: the movement goes, the
 * change stays.
 */

type EnterProps = {
    /** Seconds to wait before arriving; used for sibling staggers. */
    delay?: number
    /** How far below its resting place it starts, in pixels. */
    y?: number
    className?: string
    children: ReactNode
}

/** Content that arrives: it rises and fades in. */
export function Enter({ delay = 0, y = 6, className, children }: EnterProps) {
    return (
        <motion.div
            className={className}
            initial={{ opacity: 0, y }}
            animate={{
                opacity: 1,
                y: 0,
                transition: {
                    duration: motionDurations.base,
                    ease: motionEases.arrive,
                    delay,
                },
            }}
        >
            {children}
        </motion.div>
    )
}

/** A small arrival for chips and counters: it pops into place. */
export function Pop({ delay = 0, className, children }: Omit<EnterProps, 'y'>) {
    return (
        <motion.div
            className={className}
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{
                opacity: 1,
                scale: 1,
                transition: {
                    duration: motionDurations.base,
                    ease: motionEases.arrive,
                    delay,
                },
            }}
        >
            {children}
        </motion.div>
    )
}

/**
 * The stagger for a list that arrives as a list: each item waits a beat on
 * the one before it, and the whole run is capped so a long list never keeps
 * the reader waiting.
 */
export function stagger(index: number, step = 0.03, cap = 0.24): number {
    return Math.min(index * step, cap)
}
