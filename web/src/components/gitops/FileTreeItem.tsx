import React, { useState } from 'react'
import { ChevronDown, ChevronRight, File, Folder, FolderOpen, Settings, Code, FileText } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { FileTreeNode, ResourceKind } from '@/types/gitops'

interface FileTreeItemProps {
  node: FileTreeNode
  level: number
  selectedPath: string | null
  onSelect: (path: string) => void
  onContextMenu?: (e: React.MouseEvent, node: FileTreeNode) => void
}

function getResourceIcon(kind: ResourceKind): React.ReactNode {
  switch (kind) {
    case 'Settings':
      return <Settings className="h-4 w-4 text-blue-500" />
    case 'Variable':
      return <Code className="h-4 w-4 text-purple-500" />
    case 'Rule':
      return <FileText className="h-4 w-4 text-green-500" />
  }
}

function getFileIcon(node: FileTreeNode, isOpen: boolean): React.ReactNode {
  if (node.isDirectory) {
    return isOpen ? (
      <FolderOpen className="h-4 w-4 text-yellow-500" />
    ) : (
      <Folder className="h-4 w-4 text-yellow-500" />
    )
  }

  if (node.resource) {
    return getResourceIcon(node.resource.kind)
  }

  return <File className="h-4 w-4 text-gray-400" />
}

export function FileTreeItem({
  node,
  level,
  selectedPath,
  onSelect,
  onContextMenu,
}: FileTreeItemProps) {
  const [isOpen, setIsOpen] = useState(level < 2) // Auto-expand first two levels
  const isSelected = selectedPath === node.path
  const hasChildren = node.children.length > 0

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation()

    if (node.isDirectory) {
      setIsOpen(!isOpen)
    } else {
      onSelect(node.path)
    }
  }

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    onContextMenu?.(e, node)
  }

  return (
    <div>
      <div
        className={cn(
          'flex items-center gap-1 py-1 px-2 cursor-pointer rounded-sm hover:bg-accent transition-colors',
          isSelected && 'bg-accent'
        )}
        style={{ paddingLeft: `${level * 16 + 8}px` }}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        {node.isDirectory ? (
          <span className="w-4 h-4 flex items-center justify-center">
            {hasChildren ? (
              isOpen ? (
                <ChevronDown className="h-3 w-3" />
              ) : (
                <ChevronRight className="h-3 w-3" />
              )
            ) : null}
          </span>
        ) : (
          <span className="w-4" />
        )}

        {getFileIcon(node, isOpen)}

        <span className="ml-1 text-sm truncate flex-1">{node.name}</span>

        {node.resource && (
          <span className="text-xs text-muted-foreground px-1.5 py-0.5 bg-muted rounded">
            {node.resource.kind}
          </span>
        )}
      </div>

      {node.isDirectory && isOpen && (
        <div>
          {node.children.map((child) => (
            <FileTreeItem
              key={child.path}
              node={child}
              level={level + 1}
              selectedPath={selectedPath}
              onSelect={onSelect}
              onContextMenu={onContextMenu}
            />
          ))}
        </div>
      )}
    </div>
  )
}
