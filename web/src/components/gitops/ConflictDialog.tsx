import { useState } from 'react'
import { AlertTriangle, GitBranch, Loader2, FileWarning, ExternalLink } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useGitOps } from '@/contexts/GitOpsContext'
import { useToast } from '@/components/ui/use-toast'
import type { InitializeResult } from '@/types/gitops'

interface ConflictDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  result: InitializeResult | null
}

export function ConflictDialog({ open, onOpenChange, result }: ConflictDialogProps) {
  const { createBranch, gitCommit, settings, isLoading } = useGitOps()
  const { toast } = useToast()
  const [branchName, setBranchName] = useState(() => {
    const timestamp = new Date().toISOString().slice(0, 10).replace(/-/g, '')
    return `local-changes-${timestamp}`
  })
  const [isCreating, setIsCreating] = useState(false)

  if (!open || !result) return null

  const handleCreateBranch = async () => {
    if (!branchName.trim()) {
      toast({
        title: 'Branch name required',
        description: 'Please enter a name for the new branch.',
        variant: 'destructive',
      })
      return
    }

    setIsCreating(true)

    try {
      // First, commit any local changes
      const commitSuccess = await gitCommit(`Save local changes before merge (branch: ${branchName})`)

      if (!commitSuccess) {
        // No changes to commit is fine, continue
      }

      // Create new branch with local changes
      const success = await createBranch(branchName)

      if (success) {
        toast({
          title: 'Branch created',
          description: `Your local changes have been saved to branch "${branchName}".`,
        })
        onOpenChange(false)
      } else {
        toast({
          title: 'Failed to create branch',
          description: 'Could not create the branch. Check the logs for details.',
          variant: 'destructive',
        })
      }
    } catch (error) {
      toast({
        title: 'Error',
        description: error instanceof Error ? error.message : 'An unexpected error occurred',
        variant: 'destructive',
      })
    } finally {
      setIsCreating(false)
    }
  }

  const repoUrl = settings?.spec.git.repository || ''
  const isGitHub = repoUrl.includes('github.com')

  // Extract owner/repo for GitHub URL
  let githubUrl = ''
  if (isGitHub) {
    const match = repoUrl.match(/github\.com[:/]([^/]+\/[^/]+?)(?:\.git)?$/)
    if (match) {
      githubUrl = `https://github.com/${match[1]}`
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={() => onOpenChange(false)}
      />

      {/* Dialog */}
      <div className="relative z-50 w-full max-w-lg bg-background border rounded-lg shadow-lg p-6">
        <div className="flex items-center gap-2 mb-4">
          <AlertTriangle className="h-5 w-5 text-amber-500" />
          <h2 className="text-lg font-semibold">Merge Conflicts Detected</h2>
        </div>

        <p className="text-sm text-muted-foreground mb-4">
          The following files have conflicts between your local changes and the remote repository:
        </p>

        {/* Conflicting files list */}
        <div className="mb-4 p-3 bg-muted rounded-md max-h-40 overflow-y-auto">
          <ul className="space-y-1 text-sm">
            {result.conflictingFiles.map((file) => (
              <li key={file} className="flex items-center gap-2">
                <FileWarning className="h-4 w-4 text-amber-500 flex-shrink-0" />
                <span className="font-mono truncate">{file}</span>
              </li>
            ))}
          </ul>
        </div>

        {/* Recommendation */}
        <div className="mb-4 p-3 bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800 rounded-md">
          <p className="text-sm text-blue-800 dark:text-blue-200">
            <strong>Recommended:</strong> Create a new branch for your local changes, then resolve
            conflicts using {isGitHub ? 'GitHub' : 'a git client'}.
          </p>
        </div>

        {/* Branch name input */}
        <div className="space-y-2 mb-4">
          <Label htmlFor="branch-name">New branch name</Label>
          <Input
            id="branch-name"
            value={branchName}
            onChange={(e) => setBranchName(e.target.value)}
            placeholder="local-changes"
          />
        </div>

        {/* Actions */}
        <div className="flex flex-col sm:flex-row gap-2 pt-2">
          {githubUrl && (
            <Button
              variant="outline"
              asChild
              className="flex-1"
            >
              <a href={githubUrl} target="_blank" rel="noopener noreferrer">
                <ExternalLink className="h-4 w-4 mr-2" />
                Open in GitHub
              </a>
            </Button>
          )}

          <div className="flex gap-2 flex-1 sm:justify-end">
            <Button
              variant="ghost"
              onClick={() => onOpenChange(false)}
              disabled={isCreating || isLoading}
            >
              Cancel
            </Button>
            <Button
              onClick={handleCreateBranch}
              disabled={isCreating || isLoading || !branchName.trim()}
            >
              {isCreating ? (
                <>
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  Creating...
                </>
              ) : (
                <>
                  <GitBranch className="h-4 w-4 mr-2" />
                  Create Branch
                </>
              )}
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
