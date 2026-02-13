import {
  GitCommit,
  ArrowUp,
  ArrowDown,
  Download,
  GitMerge,
  GitBranch,
  FolderPlus,
  Check,
  X,
  Loader2,
} from 'lucide-react'
import { useGitProgressContext } from '@/contexts/GitProgressContext'
import type { GitProgressEvent, GitOperationType, GitOperationPhase } from '@/types/gitops'
import { getPhaseLabel, formatBytes, formatSpeed } from '@/types/gitops'
import { cn } from '@/lib/utils'

function getOperationIcon(type: GitOperationType) {
  switch (type) {
    case 'commit':
      return GitCommit
    case 'push':
      return ArrowUp
    case 'pull':
      return ArrowDown
    case 'fetch':
      return Download
    case 'merge':
      return GitMerge
    case 'checkout':
      return GitBranch
    case 'initialize':
      return FolderPlus
    default:
      return GitCommit
  }
}

function getOperationLabel(type: GitOperationType): string {
  switch (type) {
    case 'commit':
      return 'Commit'
    case 'push':
      return 'Push'
    case 'pull':
      return 'Pull'
    case 'fetch':
      return 'Fetch'
    case 'merge':
      return 'Merge'
    case 'checkout':
      return 'Checkout'
    case 'initialize':
      return 'Initialize'
    default:
      return type
  }
}

function getPhaseStatusIcon(phase: GitOperationPhase) {
  if (phase === 'completed') {
    return <Check className="h-4 w-4 text-green-500" />
  }
  if (phase === 'failed') {
    return <X className="h-4 w-4 text-red-500" />
  }
  return <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
}

interface ProgressItemProps {
  event: GitProgressEvent
}

function ProgressItem({ event }: ProgressItemProps) {
  const Icon = getOperationIcon(event.operationType)
  const isCompleted = event.phase === 'completed'
  const isFailed = event.phase === 'failed'
  const isFinished = isCompleted || isFailed

  return (
    <div
      className={cn(
        'rounded-lg border bg-background p-3 shadow-lg transition-all',
        isFailed && 'border-red-500/50 bg-red-500/5',
        isCompleted && 'border-green-500/50 bg-green-500/5'
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-2 mb-2">
        <div className="flex items-center gap-2">
          <Icon className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium text-sm">{getOperationLabel(event.operationType)}</span>
        </div>
        {getPhaseStatusIcon(event.phase)}
      </div>

      {/* Phase label */}
      <p className="text-sm text-muted-foreground mb-2">{getPhaseLabel(event.phase)}</p>

      {/* Progress bar */}
      {!isFinished && event.progress !== undefined && (
        <div className="mb-2">
          <div className="h-1.5 bg-muted rounded-full overflow-hidden">
            <div
              className="h-full bg-blue-500 transition-all duration-300"
              style={{ width: `${event.progress}%` }}
            />
          </div>
        </div>
      )}

      {/* Stats */}
      {!isFinished && (event.current !== undefined || event.bytesTransferred !== undefined) && (
        <div className="flex items-center gap-3 text-xs text-muted-foreground">
          {event.current !== undefined && event.total !== undefined && (
            <span>
              {event.current}/{event.total} objects
            </span>
          )}
          {event.bytesTransferred !== undefined && (
            <span>{formatBytes(event.bytesTransferred)}</span>
          )}
          {event.transferSpeed !== undefined && (
            <span>@ {formatSpeed(event.transferSpeed)}</span>
          )}
        </div>
      )}

      {/* Error message */}
      {isFailed && event.error && (
        <p className="text-xs text-red-500 mt-1 line-clamp-2">{event.error}</p>
      )}

      {/* Success message */}
      {isCompleted && <p className="text-xs text-green-600 mt-1">{event.message}</p>}
    </div>
  )
}

export function GitProgressPanel() {
  const { activeOperations, hasActiveOperations } = useGitProgressContext()

  if (!hasActiveOperations) {
    return null
  }

  // Convert map to array for rendering
  const operations = Array.from(activeOperations.values())

  return (
    <div className="fixed bottom-4 right-4 z-50 w-80 space-y-2">
      {operations.map((event) => (
        <ProgressItem key={event.operationId} event={event} />
      ))}
    </div>
  )
}
