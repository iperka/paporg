import { useState, useEffect, useMemo, useCallback } from 'react'
import { GitCommit, Loader2, Check, X, ChevronDown, ChevronRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useGitOps } from '@/contexts/GitOpsContext'
import { useGitProgressContext } from '@/contexts/GitProgressContext'
import { getPhaseLabel } from '@/types/gitops'
import { cn } from '@/lib/utils'
import { api } from '@/api'

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
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set())
  const [diffFile, setDiffFile] = useState<string | null>(null)
  const [diffContent, setDiffContent] = useState<string | null>(null)
  const [isDiffLoading, setIsDiffLoading] = useState(false)

  // All files from git status
  const allFiles = useMemo(() => {
    if (!gitStatus) return []
    return [
      ...gitStatus.modifiedFiles.map((f) => ({ path: f, type: 'M' as const })),
      ...gitStatus.untrackedFiles
        .filter((f) => !gitStatus.modifiedFiles.includes(f))
        .map((f) => ({ path: f, type: 'A' as const })),
    ]
  }, [gitStatus])

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
      setDiffFile(null)
      setDiffContent(null)
      // Select all files by default
      setSelectedFiles(new Set(allFiles.map((f) => f.path)))
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  // Watch for new commit operations starting
  useEffect(() => {
    if (isLoading && !currentOperationId) {
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

  const toggleFile = useCallback((path: string) => {
    setSelectedFiles((prev) => {
      const next = new Set(prev)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })
  }, [])

  const toggleAll = useCallback(() => {
    if (selectedFiles.size === allFiles.length) {
      setSelectedFiles(new Set())
    } else {
      setSelectedFiles(new Set(allFiles.map((f) => f.path)))
    }
  }, [selectedFiles.size, allFiles])

  const showDiff = useCallback(async (path: string) => {
    if (diffFile === path) {
      setDiffFile(null)
      setDiffContent(null)
      return
    }
    setDiffFile(path)
    setIsDiffLoading(true)
    try {
      const diff = await api.git.diff(path)
      setDiffContent(diff || '(new file)')
    } catch {
      setDiffContent('(failed to load diff)')
    } finally {
      setIsDiffLoading(false)
    }
  }, [diffFile])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (!message.trim()) {
      setLocalError('Commit message is required')
      return
    }

    if (selectedFiles.size === 0) {
      setLocalError('Select at least one file to commit')
      return
    }

    setLocalError(null)
    setCurrentOperationId(null)

    // If all files selected, pass undefined (commit all)
    const files = selectedFiles.size === allFiles.length
      ? undefined
      : Array.from(selectedFiles)

    await gitCommit(message, files)
  }

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={() => !isInProgress && onOpenChange(false)}
      />

      {/* Dialog */}
      <div className="relative z-50 w-full max-w-lg bg-background border rounded-lg shadow-lg p-6">
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

        {/* File staging - hide when in progress */}
        {!currentProgress && allFiles.length > 0 && (
          <div className="mb-4 p-3 bg-muted rounded-md text-sm">
            <div className="flex items-center justify-between mb-2">
              <p className="font-medium">
                {selectedFiles.size} of {allFiles.length} file{allFiles.length !== 1 ? 's' : ''} selected
              </p>
              <button
                type="button"
                className="text-xs text-primary hover:underline"
                onClick={toggleAll}
                disabled={isInProgress}
              >
                {selectedFiles.size === allFiles.length ? 'Deselect all' : 'Select all'}
              </button>
            </div>
            <ul className="space-y-0.5 max-h-48 overflow-y-auto">
              {allFiles.map((file) => (
                <li key={file.path}>
                  <div className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={selectedFiles.has(file.path)}
                      onChange={() => toggleFile(file.path)}
                      disabled={isInProgress}
                      className="rounded border-muted-foreground/50"
                    />
                    <span className={file.type === 'M' ? 'text-yellow-600' : 'text-green-600'}>
                      {file.type === 'M' ? 'M' : 'A'}
                    </span>
                    <button
                      type="button"
                      className="truncate text-left flex-1 hover:underline text-xs font-mono"
                      onClick={() => showDiff(file.path)}
                    >
                      {file.path}
                    </button>
                    <button
                      type="button"
                      className="text-muted-foreground hover:text-foreground shrink-0"
                      onClick={() => showDiff(file.path)}
                    >
                      {diffFile === file.path ? (
                        <ChevronDown className="h-3.5 w-3.5" />
                      ) : (
                        <ChevronRight className="h-3.5 w-3.5" />
                      )}
                    </button>
                  </div>
                  {/* Inline diff */}
                  {diffFile === file.path && (
                    <div className="mt-1 ml-6 mb-2">
                      {isDiffLoading ? (
                        <div className="flex items-center gap-1 text-xs text-muted-foreground py-2">
                          <Loader2 className="h-3 w-3 animate-spin" />
                          Loading diff...
                        </div>
                      ) : (
                        <pre className="text-xs font-mono overflow-x-auto max-h-48 overflow-y-auto rounded bg-background p-2 border">
                          {diffContent?.split('\n').map((line, i) => {
                            let color = ''
                            if (line.startsWith('+') && !line.startsWith('+++')) color = 'text-green-600'
                            else if (line.startsWith('-') && !line.startsWith('---')) color = 'text-red-600'
                            else if (line.startsWith('@@')) color = 'text-blue-500'
                            return (
                              <div key={i} className={color}>
                                {line}
                              </div>
                            )
                          })}
                        </pre>
                      )}
                    </div>
                  )}
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
              <Button type="submit" disabled={isLoading || isInProgress || !message.trim() || selectedFiles.size === 0}>
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
