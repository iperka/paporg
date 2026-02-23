import { useState } from 'react'
import { AlertTriangle, GitBranch, Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useSettings } from '@/queries/use-settings'
import { useGitStatus } from '@/queries/use-git-status'
import { useFileTree } from '@/queries/use-file-tree'
import { useInitializeGit } from '@/mutations/use-gitops-mutations'
import { useToast } from '@/components/ui/use-toast'
import type { InitializeResult } from '@/types/gitops'

interface GitInitializeBannerProps {
  onConflicts?: (result: InitializeResult) => void
}

export function GitInitializeBanner({ onConflicts }: GitInitializeBannerProps) {
  const { data: settings, isLoading: settingsLoading } = useSettings()
  const { data: gitStatus, isLoading: gitStatusLoading } = useGitStatus()
  const { data: fileTree } = useFileTree()
  const initializeGitMut = useInitializeGit()
  const { toast } = useToast()
  const [isInitializing, setIsInitializing] = useState(false)

  const isLoading = settingsLoading || gitStatusLoading

  // Replicate the needsInitialization logic from GitOpsContext
  const initialLoadComplete = fileTree !== null && gitStatus !== null
  const needsInitialization = Boolean(
    initialLoadComplete &&
    settings?.spec.git.enabled &&
    settings?.spec.git.repository &&
    gitStatus &&
    !gitStatus.isRepo,
  )

  if (!needsInitialization) {
    return null
  }

  const handleInitialize = async () => {
    setIsInitializing(true)

    try {
      const result = await initializeGitMut.mutateAsync()

      if (result) {
        if (result.conflictingFiles.length > 0) {
          // Has conflicts - show conflict dialog
          toast({
            title: 'Merge conflicts detected',
            description: `${result.conflictingFiles.length} file(s) have conflicts that need to be resolved.`,
            variant: 'destructive',
          })
          onConflicts?.(result)
        } else if (result.merged) {
          toast({
            title: 'Repository synced',
            description: result.message,
          })
        } else {
          toast({
            title: 'Repository initialized',
            description: result.message,
          })
        }
      }
    } catch (error) {
      toast({
        title: 'Error',
        description: error instanceof Error ? error.message : 'An unexpected error occurred',
        variant: 'destructive',
      })
    } finally {
      setIsInitializing(false)
    }
  }

  const repoUrl = settings?.spec.git.repository || 'configured remote'

  return (
    <div className="bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800 rounded-lg p-4 mb-4">
      <div className="flex items-start gap-3">
        <AlertTriangle className="h-5 w-5 text-amber-600 dark:text-amber-500 mt-0.5 flex-shrink-0" />
        <div className="flex-1 min-w-0">
          <h3 className="font-medium text-amber-900 dark:text-amber-100">
            Git sync is enabled but not initialized
          </h3>
          <p className="text-sm text-amber-700 dark:text-amber-300 mt-1">
            Your local config needs to be synced with the remote repository.
          </p>
          <p className="text-xs text-amber-600 dark:text-amber-400 mt-1 truncate">
            Remote: {repoUrl}
          </p>
        </div>
        <Button
          onClick={handleInitialize}
          disabled={isInitializing || isLoading}
          size="sm"
          className="flex-shrink-0"
        >
          {isInitializing ? (
            <>
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              Initializing...
            </>
          ) : (
            <>
              <GitBranch className="h-4 w-4 mr-2" />
              Initialize & Sync
            </>
          )}
        </Button>
      </div>
    </div>
  )
}
