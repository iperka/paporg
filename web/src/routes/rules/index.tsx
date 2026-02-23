import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { useFileTree } from '@/queries/use-file-tree'
import { useCreateDirectory } from '@/mutations/use-gitops-mutations'
import { FileText, Plus, FolderPlus } from 'lucide-react'
import { FolderTreeView } from '@/components/organization/FolderTreeView'
import { CreateFolderDialog } from '@/components/organization/CreateFolderDialog'
import { useToast } from '@/components/ui/use-toast'
import type { FileTreeNode } from '@/types/gitops'

export function RulesPage() {
  const { data: fileTree, isLoading: isTreeLoading } = useFileTree()
  const createDirectoryMut = useCreateDirectory()
  const { toast } = useToast()
  const [showFolderDialog, setShowFolderDialog] = useState(false)

  // Extract rules from file tree
  const getRules = (): { name: string; path: string }[] => {
    if (isTreeLoading || !fileTree) return []
    const rules: { name: string; path: string }[] = []

    const traverse = (node: FileTreeNode | null) => {
      if (!node) return
      if (node.resource?.kind === 'Rule') {
        rules.push({ name: node.resource.name, path: node.path })
      }
      node.children.forEach(traverse)
    }

    traverse(fileTree)
    return rules.sort((a, b) => a.name.localeCompare(b.name))
  }

  const rules = getRules()

  const handleCreateFolder = async (name: string) => {
    const path = `rules/${name}`
    try {
      await createDirectoryMut.mutateAsync({ path })
      toast({
        title: 'Folder created',
        description: `Created folder "${name}"`,
      })
    } catch (err) {
      throw err instanceof Error ? err : new Error('Failed to create folder')
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-3">
          <FileText className="h-8 w-8 shrink-0" />
          <div>
            <h1 className="text-2xl sm:text-3xl font-bold tracking-tight">Rules</h1>
            <p className="text-sm text-muted-foreground">
              Manage document classification and routing rules
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setShowFolderDialog(true)}>
            <FolderPlus className="h-4 w-4 sm:mr-2" />
            <span className="hidden sm:inline">New Folder</span>
          </Button>
          <Link to="/rules/$name" params={{ name: 'new' }}>
            <Button>
              <Plus className="h-4 w-4 sm:mr-2" />
              <span className="hidden sm:inline">New Rule</span>
            </Button>
          </Link>
        </div>
      </div>

      {rules.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <FileText className="h-12 w-12 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold mb-2">No Rules</h3>
            <p className="text-muted-foreground text-center mb-4 max-w-md">
              Rules define how documents are classified and where they are stored.
            </p>
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setShowFolderDialog(true)}>
                <FolderPlus className="h-4 w-4 mr-2" />
                Create Folder
              </Button>
              <Link to="/rules/$name" params={{ name: 'new' }}>
                <Button>
                  <Plus className="h-4 w-4 mr-2" />
                  Create Rule
                </Button>
              </Link>
            </div>
          </CardContent>
        </Card>
      ) : (
        <FolderTreeView
          items={rules}
          resourceType="rules"
          basePath="rules"
        />
      )}

      <CreateFolderDialog
        open={showFolderDialog}
        onOpenChange={setShowFolderDialog}
        onCreateFolder={handleCreateFolder}
        basePath="rules"
      />
    </div>
  )
}
