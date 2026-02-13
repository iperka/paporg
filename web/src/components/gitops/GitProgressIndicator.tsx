import { useMemo } from 'react'
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
import { cn } from '@/lib/utils'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

function getOperationIcon(type: GitOperationType) {
  const props = { className: 'h-3.5 w-3.5' }
  switch (type) {
    case 'commit':
      return <GitCommit {...props} />
    case 'push':
      return <ArrowUp {...props} />
    case 'pull':
      return <ArrowDown {...props} />
    case 'fetch':
      return <Download {...props} />
    case 'merge':
      return <GitMerge {...props} />
    case 'checkout':
      return <GitBranch {...props} />
    case 'initialize':
      return <FolderPlus {...props} />
    default:
      return <GitCommit {...props} />
  }
}

function getOperationLabel(type: GitOperationType): string {
  switch (type) {
    case 'commit':
      return 'Committing'
    case 'push':
      return 'Pushing'
    case 'pull':
      return 'Pulling'
    case 'fetch':
      return 'Fetching'
    case 'merge':
      return 'Merging'
    case 'checkout':
      return 'Switching'
    case 'initialize':
      return 'Initializing'
    default:
      return type
  }
}

function getPhaseLabel(phase: GitOperationPhase): string {
  switch (phase) {
    case 'starting':
      return 'Starting'
    case 'staging_files':
      return 'Staging'
    case 'committing':
      return 'Committing'
    case 'counting':
      return 'Counting'
    case 'compressing':
      return 'Compressing'
    case 'writing':
      return 'Writing'
    case 'receiving':
      return 'Receiving'
    case 'resolving':
      return 'Resolving'
    case 'unpacking':
      return 'Unpacking'
    case 'pushing':
      return 'Pushing'
    case 'pulling':
      return 'Pulling'
    case 'fetching':
      return 'Fetching'
    case 'merging':
      return 'Merging'
    case 'checking_out':
      return 'Switching'
    case 'completed':
      return 'Done'
    case 'failed':
      return 'Failed'
    default:
      return phase
  }
}

interface ProgressItemProps {
  event: GitProgressEvent
}

function ProgressItem({ event }: ProgressItemProps) {
  const isCompleted = event.phase === 'completed'
  const isFailed = event.phase === 'failed'
  const isInProgress = !isCompleted && !isFailed

  const statusIcon = isCompleted ? (
    <Check className="h-3 w-3 text-green-500" />
  ) : isFailed ? (
    <X className="h-3 w-3 text-red-500" />
  ) : (
    <Loader2 className="h-3 w-3 animate-spin" />
  )

  const tooltipContent = (
    <div className="text-xs space-y-1">
      <div className="font-medium">{getOperationLabel(event.operationType)}</div>
      <div className="text-muted-foreground">{getPhaseLabel(event.phase)}</div>
      {event.progress !== undefined && isInProgress && (
        <div className="text-muted-foreground">{event.progress}%</div>
      )}
      {event.current !== undefined && event.total !== undefined && isInProgress && (
        <div className="text-muted-foreground">
          {event.current}/{event.total} objects
        </div>
      )}
      {isFailed && event.error && (
        <div className="text-red-500 max-w-48 break-words">{event.error}</div>
      )}
      {isCompleted && <div className="text-green-500">{event.message}</div>}
    </div>
  )

  return (
    <TooltipProvider delayDuration={0}>
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            className={cn(
              'flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-colors cursor-default',
              isInProgress && 'bg-blue-500/10 text-blue-600 dark:text-blue-400',
              isCompleted && 'bg-green-500/10 text-green-600 dark:text-green-400',
              isFailed && 'bg-red-500/10 text-red-600 dark:text-red-400'
            )}
          >
            {getOperationIcon(event.operationType)}
            <span className="hidden sm:inline">{getPhaseLabel(event.phase)}</span>
            {event.progress !== undefined && isInProgress && (
              <span className="tabular-nums">{event.progress}%</span>
            )}
            {statusIcon}
          </div>
        </TooltipTrigger>
        <TooltipContent side="bottom" align="end">
          {tooltipContent}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

export function GitProgressIndicator() {
  const { activeOperations, hasActiveOperations } = useGitProgressContext()

  // Get the most recent/important operation to display
  const currentOperation = useMemo(() => {
    if (!hasActiveOperations) return null

    const ops = Array.from(activeOperations.values())

    // Prioritize: in-progress > failed > completed
    const inProgress = ops.find((op) => op.phase !== 'completed' && op.phase !== 'failed')
    if (inProgress) return inProgress

    const failed = ops.find((op) => op.phase === 'failed')
    if (failed) return failed

    // Return most recent completed
    return ops[ops.length - 1]
  }, [activeOperations, hasActiveOperations])

  if (!currentOperation) {
    return null
  }

  return <ProgressItem event={currentOperation} />
}
