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
      try {
        const resources = await api.gitops.listResources('Settings')
        if (!resources?.length || !resources[0]?.name) return []
        const resource = await api.gitops.getResource('Settings', resources[0].name)
        const parsed = yaml.load(resource.yaml)
        if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return []
        return [{ id: 'settings', ...(parsed as SettingsResource) } as SettingsItem]
      } catch (err) {
        console.error('Failed to load settings:', err)
        return []
      }
    },
    queryClient,
    getKey: (item: SettingsItem) => item.id,
  }),
)
