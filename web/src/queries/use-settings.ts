import { useLiveQuery } from '@tanstack/react-db'
import { settingsCollection } from '@/collections/settings'
import type { SettingsResource } from '@/types/gitops'

export function useSettings(): { data: SettingsResource | null; isLoading: boolean } {
  const result = useLiveQuery((q) => q.from({ s: settingsCollection }))
  const items = result.data ?? []
  return {
    data: items.length > 0 ? (items[0] as SettingsResource) : null,
    isLoading: result.isLoading,
  }
}
