import { createCollection } from '@tanstack/db'
import { queryCollectionOptions } from '@tanstack/query-db-collection'
import { queryClient } from '@/lib/query-client'
import { api } from '@/api'
import type { GitStatus } from '@/types/gitops'

interface GitStatusItem extends GitStatus {
  id: string
}

export const gitStatusCollection = createCollection(
  queryCollectionOptions({
    queryKey: ['git', 'status'],
    queryFn: async () => {
      const data = await api.git.getStatus()
      return [{ id: 'status', ...data } as GitStatusItem]
    },
    queryClient,
    getKey: (item: GitStatusItem) => item.id,
  }),
)
