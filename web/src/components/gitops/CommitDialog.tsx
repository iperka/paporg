import { useState, useEffect, useMemo } from 'react'
import { GitCommit, Loader2, Check, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useGitOps } from '@/contexts/GitOpsContext'
import { useGitProgressContext } from '@/contexts/GitProgressContext'
import { getPhaseLabel } from '@/types/gitops'
import { cn } from '@/lib/utils'

interface CommitDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function CommitDialog({ open, onOpenChange }: CommitDialogProps) {
  const { gitStatus, gitCommit, isLoading, error: contextError } = useGitOps()
  const { activeOperations } = useGitProgressContext()

  const [message, setMessage] = useState('')
  const [localError, setLocalError] = useState<string | null>(null)
  const [currentOperationId, setCurrentOperationId] = useState<string | null>(null)

  // Get the current operation progress if we have one
  const currentProgress = useMemo(() => {
    if (!currentOperationId) return null
    return activeOperations.get(currentOperationId) || null
  }, [currentOperationId, activeOperations])

  // Check if operation is complete
  const isCompleted = currentProgress?.phase === 'completed'
  const isFailed = currentProgress?.phase === 'failed'
  const isInProgress = Boolean(currentProgress && !isCompleted && !isFailed)

  // Combine local validation errors with context errors
  const error = localError || contextError || (isFailed ? currentProgress?.error : null)

  // Reset form when dialog opens
  useEffect(() => {
    if (open) {
      setMessage('')
      setLocalError(null)
      setCurrentOperationId(null)
    }
  }, [open])

  // Watch for new commit operations starting
  useEffect(() => {
    if (isLoading && !currentOperationId) {
      // Find any commit operation that started recently
      for (const [id, op] of activeOperations) {
        if (op.operationType === 'commit' && !currentOperationId) {
          setCurrentOperationId(id)
          break
        }
      }
    }
  }, [isLoading, activeOperations, currentOperationId])

  // Auto-close on success after a delay
  useEffect(() => {
    if (isCompleted) {
      const timer = setTimeout(() => {
        onOpenChange(false)
      }, 1500)
      return () => clearTimeout(timer)
    }
  }, [isCompleted, onOpenChange])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (!message.trim()) {
      setLocalError('Commit message is required')
      return
    }

    setLocalError(null)
    setCurrentOperationId(null) // Clear so we pick up the new one
    await gitCommit(message)
    // Result handling is done via progress context
  }

  if (!open) return null

  const totalChanges = gitStatus
    ? gitStatus.modifiedFiles.length + gitStatus.untrackedFiles.length
    : 0

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={() => !isInProgress && onOpenChange(false)}
      />

      {/* Dialog */}
      <div className="relative z-50 w-full max-w-md bg-background border rounded-lg shadow-lg p-6">
        <div className="flex items-center gap-2 mb-4">
          <GitCommit className="h-5 w-5" />
          <h2 className="text-lg font-semibold">Commit Changes</h2>
        </div>

        {/* Progress display */}
        {currentProgress && (
          <div
            className={cn(
              'mb-4 p-3 rounded-md text-sm',
              isFailed && 'bg-red-500/10 border border-red-500/30',
              isCompleted && 'bg-green-500/10 border border-green-500/30',
              isInProgress && 'bg-blue-500/10 border border-blue-500/30'
            )}
          >
            <div className="flex items-center gap-2 mb-2">
              {isInProgress && <Loader2 className="h-4 w-4 animate-spin text-blue-500" />}
              {isCompleted && <Check className="h-4 w-4 text-green-500" />}
              {isFailed && <X className="h-4 w-4 text-red-500" />}
              <span className="font-medium">{getPhaseLabel(currentProgress.phase)}</span>
            </div>

            {/* Progress bar */}
            {isInProgress && currentProgress.progress !== undefined && (
              <div className="mt-2">
                <div className="h-1.5 bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-full bg-blue-500 transition-all duration-300"
                    style={{ width: `${currentProgress.progress}%` }}
                  />
                </div>
                {currentProgress.current !== undefined && currentProgress.total !== undefined && (
                  <p className="text-xs text-muted-foreground mt-1">
                    {currentProgress.current}/{currentProgress.total} objects
                  </p>
                )}
              </div>
            )}

            {/* Completion message */}
            {isCompleted && (
              <p className="text-green-600">{currentProgress.message}</p>
            )}

            {/* Error message */}
            {isFailed && currentProgress.error && (
              <p className="text-red-500">{currentProgress.error}</p>
            )}
          </div>
        )}

        {/* Changed files summary - hide when in progress */}
        {!currentProgress && gitStatus && totalChanges > 0 && (
          <div className="mb-4 p-3 bg-muted rounded-md text-sm">
            <p className="font-medium mb-2">
              {totalChanges} file{totalChanges !== 1 ? 's' : ''} to commit:
            </p>
            <ul className="space-y-1 max-h-32 overflow-y-auto">
              {gitStatus.modifiedFiles.map((file) => (
                <li key={file} className="flex items-center gap-2">
                  <span className="text-yellow-600">M</span>
                  <span className="truncate">{file}</span>
                </li>
              ))}
              {gitStatus.untrackedFiles.map((file) => (
                <li key={file} className="flex items-center gap-2">
                  <span className="text-green-600">A</span>
                  <span className="truncate">{file}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="message">Commit Message</Label>
            <Input
              id="message"
              value={message}
              onChange={(e) => {
                setMessage(e.target.value)
                setLocalError(null)
              }}
              placeholder="Describe your changes..."
              className={error && !currentProgress ? 'border-destructive' : ''}
              autoFocus
              disabled={isInProgress}
            />
            {error && !currentProgress && <p className="text-sm text-destructive">{error}</p>}
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={isInProgress}
            >
              {isCompleted ? 'Close' : 'Cancel'}
            </Button>
            {!isCompleted && (
              <Button type="submit" disabled={isLoading || isInProgress || !message.trim()}>
                {isInProgress ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    Working...
                  </>
                ) : (
                  'Commit & Push'
                )}
              </Button>
            )}
          </div>
        </form>
      </div>
    </div>
  )
}
