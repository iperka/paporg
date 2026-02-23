import { createCollection } from '@tanstack/db'
import { queryCollectionOptions } from '@tanstack/query-db-collection'
import { queryClient } from '@/lib/query-client'
import { api } from '@/api'
import type { FileTreeNode } from '@/types/gitops'

// Wrap the single FileTreeNode in an array with an id for the collection
interface FileTreeItem extends FileTreeNode {
  id: string
}

export const fileTreeCollection = createCollection(
  queryCollectionOptions({
    queryKey: ['gitops', 'file-tree'],
    queryFn: async () => {
      const data = await api.gitops.getFileTree()
      return [{ id: 'root', ...data } as FileTreeItem]
    },
    queryClient,
    getKey: (item: FileTreeItem) => item.id,
  }),
)
