import { createCollection } from '@tanstack/db'
import { queryCollectionOptions } from '@tanstack/query-db-collection'
import { queryClient } from '@/lib/query-client'
import { api } from '@/api'
import type { SettingsResource } from '@/types/gitops'
import yaml from 'js-yaml'

interface SettingsItem extends SettingsResource {
  id: string
}

export const settingsCollection = createCollection(
  queryCollectionOptions({
    queryKey: ['gitops', 'settings'],
    queryFn: async () => {
      const resources = await api.gitops.listResources('Settings')
      if (resources.length === 0) return []
      const resource = await api.gitops.getResource('Settings', resources[0].name)
      try {
        const parsed = yaml.load(resource.yaml) as SettingsResource | undefined
        if (!parsed) return []
        return [{ id: 'settings', ...parsed } as SettingsItem]
      } catch {
        return []
      }
    },
    queryClient,
    getKey: (item: SettingsItem) => item.id,
  }),
)
