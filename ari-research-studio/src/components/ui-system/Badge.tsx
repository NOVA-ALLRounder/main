import React from 'react'
import { cn } from '../../lib/utils'

interface BadgeProps {
  children: React.ReactNode
  variant?: 'default' | 'secondary' | 'outline' | 'destructive' | 'success'
  className?: string
}

export const Badge = ({ children, variant = 'default', className }: BadgeProps) => {
  const variants = {
    default: 'bg-primary-500/10 text-primary-400 border-primary-500/20',
    secondary: 'bg-white/5 text-white/60 border-white/10',
    outline: 'bg-transparent text-white/40 border-white/10',
    destructive: 'bg-red-500/10 text-red-400 border-red-500/20',
    success: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
  }

  return (
    <span className={cn('inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium transition-colors', variants[variant], className)}>
      {children}
    </span>
  )
}
