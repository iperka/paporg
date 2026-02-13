import React, { useState } from 'react'
import { Plus, RefreshCw, FolderPlus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { FileTreeItem } from './FileTreeItem'
import { CreateResourceDialog } from './CreateResourceDialog'
import { useGitOps } from '@/contexts/GitOpsContext'
import type { FileTreeNode, ResourceKind } from '@/types/gitops'

interface ContextMenuState {
  x: number
  y: number
  node: FileTreeNode | null
}

export function FileTree() {
  const {
    fileTree,
    selectedPath,
    selectFile,
    refreshTree,
    createDirectory,
    deleteFile,
    isLoading,
  } = useGitOps()

  const [contextMenu, setContextMenu] = useState<ContextMenuState>({ x: 0, y: 0, node: null })
  const [showCreateDialog, setShowCreateDialog] = useState(false)
  const [createKind, setCreateKind] = useState<ResourceKind>('Rule')
  const [createPath, setCreatePath] = useState<string | undefined>()

  const handleContextMenu = (e: React.MouseEvent, node: FileTreeNode) => {
    setContextMenu({ x: e.clientX, y: e.clientY, node })
  }

  const closeContextMenu = () => {
    setContextMenu({ x: 0, y: 0, node: null })
  }

  const handleCreateResource = (kind: ResourceKind, basePath?: string) => {
    setCreateKind(kind)
    setCreatePath(basePath)
    setShowCreateDialog(true)
    closeContextMenu()
  }

  const handleCreateDirectory = async () => {
    const name = prompt('Enter directory name:')
    if (name) {
      const basePath = contextMenu.node?.isDirectory
        ? contextMenu.node.path
        : contextMenu.node?.path.split('/').slice(0, -1).join('/') || ''
      const path = basePath ? `${basePath}/${name}` : name
      await createDirectory(path)
    }
    closeContextMenu()
  }

  const handleDelete = async () => {
    if (!contextMenu.node) return

    const confirmed = confirm(
      `Are you sure you want to delete "${contextMenu.node.name}"?${
        contextMenu.node.isDirectory ? ' This will delete all contents.' : ''
      }`
    )

    if (confirmed) {
      await deleteFile(contextMenu.node.path)
    }
    closeContextMenu()
  }

  // Close context menu when clicking elsewhere
  React.useEffect(() => {
    const handleClick = () => closeContextMenu()
    if (contextMenu.node) {
      document.addEventListener('click', handleClick)
      return () => document.removeEventListener('click', handleClick)
    }
  }, [contextMenu.node])

  if (!fileTree) {
    return (
      <div className="p-4 text-center text-muted-foreground">
        {isLoading ? 'Loading...' : 'No configuration loaded'}
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col">
      <div className="p-2 border-b flex items-center gap-2">
        <span className="text-sm font-medium flex-1">Files</span>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => handleCreateResource('Rule')}
          title="New Rule"
        >
          <Plus className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={refreshTree}
          title="Refresh"
        >
          <RefreshCw className={`h-4 w-4 ${isLoading ? 'animate-spin' : ''}`} />
        </Button>
      </div>

      <ScrollArea className="flex-1">
        <div className="py-1">
          {fileTree.children.map((child) => (
            <FileTreeItem
              key={child.path}
              node={child}
              level={0}
              selectedPath={selectedPath}
              onSelect={selectFile}
              onContextMenu={handleContextMenu}
            />
          ))}
        </div>
      </ScrollArea>

      {/* Context Menu */}
      {contextMenu.node && (
        <div
          className="fixed z-50 min-w-[160px] bg-popover text-popover-foreground rounded-md border shadow-md py-1"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          {contextMenu.node.isDirectory && (
            <>
              <button
                className="w-full px-3 py-1.5 text-sm text-left hover:bg-accent flex items-center gap-2"
                onClick={() => handleCreateResource('Rule', contextMenu.node?.path)}
              >
                <Plus className="h-4 w-4" />
                New Rule
              </button>
              <button
                className="w-full px-3 py-1.5 text-sm text-left hover:bg-accent flex items-center gap-2"
                onClick={() => handleCreateResource('Variable', contextMenu.node?.path)}
              >
                <Plus className="h-4 w-4" />
                New Variable
              </button>
              <button
                className="w-full px-3 py-1.5 text-sm text-left hover:bg-accent flex items-center gap-2"
                onClick={handleCreateDirectory}
              >
                <FolderPlus className="h-4 w-4" />
                New Folder
              </button>
              <div className="border-t my-1" />
            </>
          )}

          {contextMenu.node.path !== 'settings.yaml' && (
            <button
              className="w-full px-3 py-1.5 text-sm text-left hover:bg-accent text-destructive"
              onClick={handleDelete}
            >
              Delete
            </button>
          )}
        </div>
      )}

      {/* Create Resource Dialog */}
      <CreateResourceDialog
        open={showCreateDialog}
        onOpenChange={setShowCreateDialog}
        defaultKind={createKind}
        basePath={createPath}
      />
    </div>
  )
}
