import { cn } from '@/lib/utils'

interface GitStatusIndicatorProps {
  status?: 'M' | 'A' | 'D' | '?' | 'R' | string
  staged?: boolean
  className?: string
}

const statusConfig: Record<string, { label: string; color: string; title: string }> = {
  M: { label: 'M', color: 'text-yellow-500', title: 'Modified' },
  A: { label: 'A', color: 'text-green-500', title: 'Added' },
  D: { label: 'D', color: 'text-red-500', title: 'Deleted' },
  '?': { label: 'U', color: 'text-green-500', title: 'Untracked' },
  R: { label: 'R', color: 'text-blue-500', title: 'Renamed' },
}

export function GitStatusIndicator({ status, staged, className }: GitStatusIndicatorProps) {
  if (!status) return null

  const config = statusConfig[status]
  if (!config) return null

  return (
    <span
      className={cn(
        'inline-flex items-center justify-center text-[10px] font-bold w-4 h-4 rounded-sm',
        config.color,
        staged && 'ring-1 ring-current',
        className
      )}
      title={`${config.title}${staged ? ' (staged)' : ''}`}
    >
      {config.label}
    </span>
  )
}
