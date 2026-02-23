import { GitBranch, GitCommit, Check, AlertCircle, ArrowDown, ArrowUp, RefreshCw } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { useGitStatus } from '@/queries/use-git-status'
import { useGitPull, GIT_STATUS_KEYS } from '@/mutations/use-gitops-mutations'
import { cn } from '@/lib/utils'

interface GitStatusProps {
  onCommitClick: () => void
}

export function GitStatus({ onCommitClick }: GitStatusProps) {
  const { data: gitStatus } = useGitStatus()
  const gitPullMut = useGitPull()
  const qc = useQueryClient()

  const isLoading = gitPullMut.isPending

  if (!gitStatus) {
    return null
  }

  if (!gitStatus.isRepo) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground text-sm">
        <AlertCircle className="h-4 w-4" />
        <span>Not a git repository</span>
      </div>
    )
  }

  const hasChanges = !gitStatus.isClean
  const totalChanges = gitStatus.modifiedFiles.length + gitStatus.untrackedFiles.length

  return (
    <div className="flex items-center gap-3">
      {/* Branch */}
      <div className="flex items-center gap-1.5 text-sm">
        <GitBranch className="h-4 w-4" />
        <span className="font-medium">{gitStatus.branch || 'unknown'}</span>
      </div>

      {/* Sync status */}
      {(gitStatus.ahead > 0 || gitStatus.behind > 0) && (
        <div className="flex items-center gap-1 text-sm text-muted-foreground">
          {gitStatus.ahead > 0 && (
            <span className="flex items-center gap-0.5">
              <ArrowUp className="h-3 w-3" />
              {gitStatus.ahead}
            </span>
          )}
          {gitStatus.behind > 0 && (
            <span className="flex items-center gap-0.5">
              <ArrowDown className="h-3 w-3" />
              {gitStatus.behind}
            </span>
          )}
        </div>
      )}

      {/* Status badge */}
      {hasChanges ? (
        <Badge variant="secondary" className="gap-1">
          <span className="h-2 w-2 rounded-full bg-yellow-500" />
          {totalChanges} change{totalChanges !== 1 ? 's' : ''}
        </Badge>
      ) : (
        <Badge variant="outline" className="gap-1 text-green-600">
          <Check className="h-3 w-3" />
          Clean
        </Badge>
      )}

      {/* Actions */}
      <div className="flex items-center gap-1">
        {gitStatus.behind > 0 && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => gitPullMut.mutate()}
            disabled={isLoading}
            className="h-7"
          >
            <ArrowDown className="h-4 w-4 mr-1" />
            Pull
          </Button>
        )}

        {hasChanges && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onCommitClick}
            disabled={isLoading}
            className="h-7"
          >
            <GitCommit className="h-4 w-4 mr-1" />
            Commit
          </Button>
        )}

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })}
          disabled={isLoading}
          title="Refresh git status"
        >
          <RefreshCw className={cn('h-4 w-4', isLoading && 'animate-spin')} />
        </Button>
      </div>
    </div>
  )
}
