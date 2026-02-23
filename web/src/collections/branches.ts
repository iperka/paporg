import { createCollection } from '@tanstack/db'
import { queryCollectionOptions } from '@tanstack/query-db-collection'
import { queryClient } from '@/lib/query-client'
import { api } from '@/api'
import type { BranchInfo } from '@/types/gitops'

interface BranchItem extends BranchInfo {
  id: string
}

export const branchesCollection = createCollection(
  queryCollectionOptions({
    queryKey: ['git', 'branches'],
    queryFn: async () => {
      const data = await api.git.getBranches()
      return data.map((b) => ({ id: b.name, ...b }) as BranchItem)
    },
    queryClient,
    getKey: (item: BranchItem) => item.id,
  }),
)
