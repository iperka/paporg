import { createCollection } from '@tanstack/db'
import { queryCollectionOptions } from '@tanstack/query-db-collection'
import { queryClient } from '@/lib/query-client'
import { api, type ResourceInfo } from '@/api'

interface ResourceItem extends ResourceInfo {
  id: string
}

export const resourcesCollection = createCollection(
  queryCollectionOptions({
    queryKey: ['gitops', 'resources'],
    queryFn: async () => {
      const data = await api.gitops.listResources()
      return data.map((r) => ({ id: `${r.kind}:${r.name}`, ...r }) as ResourceItem)
    },
    queryClient,
    getKey: (item: ResourceItem) => item.id,
  }),
)
