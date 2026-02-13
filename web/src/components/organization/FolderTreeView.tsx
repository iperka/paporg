import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import { ChevronRight, Folder, FolderOpen, FolderInput, Variable, FileText } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'

export interface FolderNode {
  name: string
  path: string
  type: 'folder'
  children: (FolderNode | ItemNode)[]
}

export interface ItemNode {
  name: string
  path: string
  type: 'item'
}

interface FolderTreeViewProps {
  items: { name: string; path: string }[]
  resourceType: 'variables' | 'rules' | 'sources'
  basePath: string
}

function buildTree(
  items: { name: string; path: string }[],
  basePath: string
): (FolderNode | ItemNode)[] {
  const root: (FolderNode | ItemNode)[] = []
  const folderMap = new Map<string, FolderNode>()

  items.forEach((item) => {
    // Get relative path from basePath
    const relativePath = item.path.startsWith(basePath + '/')
      ? item.path.slice(basePath.length + 1)
      : item.path

    const parts = relativePath.split('/')

    if (parts.length === 1) {
      // Item at root level (it's a file like "item.yaml")
      root.push({
        name: item.name,
        path: item.path,
        type: 'item',
      })
    } else {
      // Item is in a subfolder
      let currentPath = basePath
      let currentChildren = root

      // Create/navigate folder structure
      for (let i = 0; i < parts.length - 1; i++) {
        const folderName = parts[i]
        currentPath = currentPath ? `${currentPath}/${folderName}` : folderName

        let folder = folderMap.get(currentPath)
        if (!folder) {
          folder = {
            name: folderName,
            path: currentPath,
            type: 'folder',
            children: [],
          }
          folderMap.set(currentPath, folder)
          currentChildren.push(folder)
        }
        currentChildren = folder.children
      }

      // Add the item to the deepest folder
      currentChildren.push({
        name: item.name,
        path: item.path,
        type: 'item',
      })
    }
  })

  // Sort: folders first, then items, both alphabetically
  const sortNodes = (nodes: (FolderNode | ItemNode)[]) => {
    nodes.sort((a, b) => {
      if (a.type === 'folder' && b.type !== 'folder') return -1
      if (a.type !== 'folder' && b.type === 'folder') return 1
      return a.name.localeCompare(b.name)
    })
    nodes.forEach((node) => {
      if (node.type === 'folder') {
        sortNodes(node.children)
      }
    })
  }

  sortNodes(root)
  return root
}

function getResourceIcon(resourceType: 'variables' | 'rules' | 'sources') {
  switch (resourceType) {
    case 'variables':
      return Variable
    case 'rules':
      return FileText
    case 'sources':
      return FolderInput
  }
}

function FolderItem({
  folder,
  resourceType,
  depth = 0,
}: {
  folder: FolderNode
  resourceType: 'variables' | 'rules' | 'sources'
  depth?: number
}) {
  const [isOpen, setIsOpen] = useState(true)
  const Icon = getResourceIcon(resourceType)

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <CollapsibleTrigger className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium hover:bg-muted transition-colors">
        <ChevronRight
          className={cn(
            'h-4 w-4 shrink-0 transition-transform',
            isOpen && 'rotate-90'
          )}
        />
        {isOpen ? (
          <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
        ) : (
          <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
        )}
        <span className="truncate">{folder.name}</span>
        <span className="ml-auto text-xs text-muted-foreground">
          {folder.children.filter((c) => c.type === 'item').length}
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="ml-4 border-l pl-2 mt-1 space-y-1">
          {folder.children.map((child) =>
            child.type === 'folder' ? (
              <FolderItem
                key={child.path}
                folder={child}
                resourceType={resourceType}
                depth={depth + 1}
              />
            ) : (
              <Link
                key={child.name}
                to={`/${resourceType}/$name`}
                params={{ name: child.name }}
              >
                <Card className="hover:border-primary/50 hover:shadow transition-all cursor-pointer">
                  <CardHeader className="p-4 pb-2">
                    <CardTitle className="text-base flex items-center gap-2">
                      <Icon className="h-4 w-4 shrink-0" />
                      <span className="truncate">{child.name}</span>
                    </CardTitle>
                  </CardHeader>
                  <CardContent className="p-4 pt-0">
                    <p className="text-xs text-muted-foreground truncate">
                      {child.path}
                    </p>
                  </CardContent>
                </Card>
              </Link>
            )
          )}
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}

export function FolderTreeView({ items, resourceType, basePath }: FolderTreeViewProps) {
  const tree = buildTree(items, basePath)
  const Icon = getResourceIcon(resourceType)

  // Separate root items and folders
  const rootFolders = tree.filter((node): node is FolderNode => node.type === 'folder')
  const rootItems = tree.filter((node): node is ItemNode => node.type === 'item')

  return (
    <div className="space-y-4">
      {/* Folders */}
      {rootFolders.map((folder) => (
        <FolderItem
          key={folder.path}
          folder={folder}
          resourceType={resourceType}
        />
      ))}

      {/* Root level items */}
      {rootItems.length > 0 && (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {rootItems.map((item) => (
            <Link
              key={item.name}
              to={`/${resourceType}/$name`}
              params={{ name: item.name }}
            >
              <Card className="hover:border-primary/50 hover:shadow transition-all cursor-pointer h-full">
                <CardHeader className="p-4 pb-2">
                  <CardTitle className="text-base flex items-center gap-2">
                    <Icon className="h-4 w-4 shrink-0" />
                    <span className="truncate">{item.name}</span>
                  </CardTitle>
                </CardHeader>
                <CardContent className="p-4 pt-0">
                  <p className="text-xs text-muted-foreground truncate">
                    {item.path}
                  </p>
                </CardContent>
              </Card>
            </Link>
          ))}
        </div>
      )}
    </div>
  )
}
