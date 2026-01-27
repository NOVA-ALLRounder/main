import React from 'react'
import { cn } from '../../lib/utils'

export const Card = ({ className, children }: { className?: string, children: React.ReactNode }) => {
  return (
    <div className={cn('glass-card rounded-2xl p-6', className)}>
      {children}
    </div>
  )
}

export const CardHeader = ({ className, children }: { className?: string, children: React.ReactNode }) => (
  <div className={cn('space-y-1.5 mb-4', className)}>{children}</div>
)

export const CardTitle = ({ className, children }: { className?: string, children: React.ReactNode }) => (
  <h3 className={cn('text-lg font-semibold leading-none tracking-tight text-white/90', className)}>
    {children}
  </h3>
)

export const CardDescription = ({ className, children }: { className?: string, children: React.ReactNode }) => (
  <p className={cn('text-sm text-white/40', className)}>{children}</p>
)

export const CardContent = ({ className, children }: { className?: string, children: React.ReactNode }) => (
  <div className={cn('', className)}>{children}</div>
)
