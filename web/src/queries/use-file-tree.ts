import { useLiveQuery } from '@tanstack/react-db'
import { fileTreeCollection } from '@/collections/file-tree'
import type { FileTreeNode } from '@/types/gitops'

export function useFileTree(): { data: FileTreeNode | null; isLoading: boolean } {
  const result = useLiveQuery((q) => q.from({ ft: fileTreeCollection }))
  const items = result.data ?? []
  return {
    data: items.length > 0 ? (items[0] as FileTreeNode) : null,
    isLoading: items.length === 0,
  }
}
