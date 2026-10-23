import { motion } from 'motion/react'
import { useState, useEffect } from 'react'

interface SkeletonProps {
  className?: string
  variant?: 'text' | 'circular' | 'rectangular' | 'rounded'
  width?: string | number
  height?: string | number
}

export function Skeleton({ 
  className = '', 
  variant = 'text',
  width,
  height 
}: SkeletonProps) {
  const baseClasses = 'bg-gray-200 dark:bg-gray-700 overflow-hidden relative'
  
  const variantClasses = {
    text: 'h-4 rounded',
    circular: 'rounded-full',
    rectangular: 'rounded',
    rounded: 'rounded-lg',
  }

  const style: React.CSSProperties = {}
  if (width) style.width = typeof width === 'number' ? `${width}px` : width
  if (height && variant !== 'text') style.height = typeof height === 'number' ? `${height}px` : height

  return (
    <div 
      className={`${baseClasses} ${variantClasses[variant]} ${className} skeleton-wrapper`}
      style={style}
    >
      <motion.div
        className="absolute inset-0 bg-gradient-to-r from-transparent via-gray-300/50 dark:via-gray-600/50 to-transparent"
        animate={{ x: ['-100%', '100%'] }}
        transition={{ duration: 1.5, repeat: Infinity, ease: 'linear' }}
      />
    </div>
  )
}

export function SkeletonAgent() {
  return (
    <div className="flex flex-col items-center gap-2 p-2">
      <Skeleton variant="circular" width={48} height={48} />
      <Skeleton variant="text" width={40} />
    </div>
  )
}

export function SkeletonMessage() {
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <Skeleton variant="circular" width={32} height={32} />
        <Skeleton variant="text" width={100} />
      </div>
      <div className="space-y-2">
        <Skeleton variant="rounded" width="100%" height={16} />
        <Skeleton variant="rounded" width="85%" height={16} />
        <Skeleton variant="rounded" width="95%" height={16} />
      </div>
    </div>
  )
}

export function SkeletonMemoryCard() {
  return (
    <div className="p-4 space-y-3">
      <Skeleton variant="text" width="70%" height={20} />
      <Skeleton variant="text" width="50%" />
      <Skeleton variant="text" width="40%" />
    </div>
  )
}

export function SkeletonTopicList() {
  return (
    <div className="space-y-2 p-2">
      {[1, 2, 3, 4, 5].map((i) => (
        <div key={i} className="flex items-center gap-3 p-2">
          <Skeleton variant="circular" width={16} height={16} />
          <Skeleton variant="text" width="80%" />
        </div>
      ))}
    </div>
  )
}

// Wrapper component for smooth skeleton-to-content transition
interface SkeletonTransitionProps {
  isLoading: boolean
  children: React.ReactNode
  skeleton: React.ReactNode
  className?: string
}

export function SkeletonTransition({ 
  isLoading, 
  children, 
  skeleton,
  className = ''
}: SkeletonTransitionProps) {
  const [isLoaded, setIsLoaded] = useState(!isLoading)
  
  useEffect(() => {
    if (!isLoading) {
      // Small delay to ensure smooth transition
      const timer = setTimeout(() => {
        setIsLoaded(true)
      }, 50)
      return () => clearTimeout(timer)
    } else {
      setIsLoaded(false)
    }
  }, [isLoading])

  if (isLoading) {
    return (
      <div className={`skeleton-wrapper ${className}`}>
        {skeleton}
      </div>
    )
  }

  return (
    <div className={`skeleton-loaded ${className}`}>
      {children}
    </div>
  )
}
