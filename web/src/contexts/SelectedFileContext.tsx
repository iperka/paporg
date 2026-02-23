import React, { createContext, useContext, useState, useCallback } from 'react'
import { api } from '@/api'
import { useFileTree } from '@/queries/use-file-tree'
import type { ResourceDetail, ResourceKind, FileTreeNode } from '@/types/gitops'

interface SelectedFileContextValue {
  selectedPath: string | null
  selectedResource: ResourceDetail | null
  selectFile: (path: string) => Promise<void>
}

const SelectedFileContext = createContext<SelectedFileContextValue | null>(null)

export function useSelectedFile(): SelectedFileContextValue {
  const context = useContext(SelectedFileContext)
  if (!context) {
    throw new Error('useSelectedFile must be used within a SelectedFileProvider')
  }
  return context
}

export function SelectedFileProvider({ children }: { children: React.ReactNode }) {
  const { data: fileTree } = useFileTree()
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [selectedResource, setSelectedResource] = useState<ResourceDetail | null>(null)

  const selectFile = useCallback(
    async (path: string) => {
      setSelectedPath(path)

      const findResource = (
        node: FileTreeNode,
      ): { kind: ResourceKind; name: string } | null => {
        if (node.path === path && node.resource) {
          return { kind: node.resource.kind, name: node.resource.name }
        }
        for (const child of node.children) {
          const found = findResource(child)
          if (found) return found
        }
        return null
      }

      if (!fileTree) return

      const resourceInfo = findResource(fileTree)
      if (!resourceInfo) {
        try {
          const content = await api.files.readRaw(path)
          setSelectedResource({
            name: path.split('/').pop() || path,
            path,
            yaml: content,
          })
        } catch {
          // File not found
        }
        return
      }

      try {
        const data = await api.gitops.getResource(resourceInfo.kind, resourceInfo.name)
        setSelectedResource({ name: data.name, path: data.path, yaml: data.yaml })
      } catch {
        try {
          const content = await api.files.readRaw(path)
          setSelectedResource({ name: resourceInfo.name, path, yaml: content })
        } catch {
          // Fallback failed
        }
      }
    },
    [fileTree],
  )

  return (
    <SelectedFileContext.Provider value={{ selectedPath, selectedResource, selectFile }}>
      {children}
    </SelectedFileContext.Provider>
  )
}
