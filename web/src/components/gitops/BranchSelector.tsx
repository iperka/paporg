import { useState } from 'react'
import { GitBranch, Check, Plus, Cloud } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useGitOps } from '@/contexts/GitOpsContext'
import { useToast } from '@/components/ui/use-toast'

export function BranchSelector() {
  const { gitStatus, branches, checkoutBranch, createBranch, isLoading } = useGitOps()
  const { toast } = useToast()
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false)
  const [newBranchName, setNewBranchName] = useState('')
  const [isCreating, setIsCreating] = useState(false)

  if (!gitStatus?.isRepo) {
    return null
  }

  const currentBranch = gitStatus.branch || 'main'
  const localBranches = branches.filter(b => !b.isRemote)
  const remoteBranches = branches.filter(b => b.isRemote && !localBranches.some(lb => lb.name === b.name))

  const handleCheckout = async (branch: string) => {
    const success = await checkoutBranch(branch)
    if (success) {
      toast({
        title: 'Branch switched',
        description: `Switched to branch: ${branch}`,
      })
    } else {
      toast({
        title: 'Switch failed',
        description: `Failed to switch to branch: ${branch}`,
        variant: 'destructive',
      })
    }
  }

  const handleCreateBranch = async () => {
    if (!newBranchName.trim()) return

    setIsCreating(true)
    try {
      const success = await createBranch(newBranchName.trim())
      if (success) {
        toast({
          title: 'Branch created',
          description: `Created and switched to branch: ${newBranchName}`,
        })
        setNewBranchName('')
        setIsCreateDialogOpen(false)
      } else {
        toast({
          title: 'Creation failed',
          description: `Failed to create branch: ${newBranchName}`,
          variant: 'destructive',
        })
      }
    } finally {
      setIsCreating(false)
    }
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" className="h-10 gap-2 w-full justify-start" disabled={isLoading}>
            <GitBranch className="h-5 w-5" />
            <span className="truncate">{currentBranch}</span>
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-56">
          <DropdownMenuLabel>Local Branches</DropdownMenuLabel>
          {localBranches.length === 0 ? (
            <DropdownMenuItem disabled>No local branches</DropdownMenuItem>
          ) : (
            localBranches.map((branch) => (
              <DropdownMenuItem
                key={branch.name}
                onClick={() => !branch.isCurrent && handleCheckout(branch.name)}
                className="flex items-center justify-between"
                disabled={branch.isCurrent}
              >
                <span className="truncate">{branch.name}</span>
                {branch.isCurrent && <Check className="h-4 w-4 text-green-500" />}
              </DropdownMenuItem>
            ))
          )}

          {remoteBranches.length > 0 && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuLabel className="flex items-center gap-2">
                <Cloud className="h-3 w-3" />
                Remote Branches
              </DropdownMenuLabel>
              {remoteBranches.map((branch) => (
                <DropdownMenuItem
                  key={`remote-${branch.name}`}
                  onClick={() => handleCheckout(branch.name)}
                  className="flex items-center gap-2"
                >
                  <span className="truncate text-muted-foreground">{branch.name}</span>
                </DropdownMenuItem>
              ))}
            </>
          )}

          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => setIsCreateDialogOpen(true)}>
            <Plus className="h-4 w-4 mr-2" />
            Create new branch
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <Dialog open={isCreateDialogOpen} onOpenChange={setIsCreateDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create New Branch</DialogTitle>
            <DialogDescription>
              Create a new branch from the current branch ({currentBranch}).
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="branch-name">Branch name</Label>
              <Input
                id="branch-name"
                placeholder="feature/my-new-feature"
                value={newBranchName}
                onChange={(e) => setNewBranchName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault()
                    handleCreateBranch()
                  }
                }}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setIsCreateDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleCreateBranch} disabled={!newBranchName.trim() || isCreating}>
              {isCreating ? 'Creating...' : 'Create'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
