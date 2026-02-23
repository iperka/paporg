import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { listen } from '@tauri-apps/api/event'
import type { ConfigChangeEvent } from '@/types/gitops'

/**
 * Listens for Tauri `paporg://config-changed` events and invalidates
 * relevant TanStack Query keys so collections refetch automatically.
 * Should be called once in RootLayout.
 */
export function useConfigChangeInvalidation() {
  const qc = useQueryClient()

  useEffect(() => {
    let unlisten: (() => void) | null = null

    const setup = async () => {
      unlisten = await listen<ConfigChangeEvent>(
        'paporg://config-changed',
        (event) => {
          console.log('Config changed:', event.payload)
          qc.invalidateQueries({ queryKey: ['gitops', 'file-tree'] })
          qc.invalidateQueries({ queryKey: ['git', 'status'] })
          qc.invalidateQueries({ queryKey: ['gitops', 'settings'] })
          qc.invalidateQueries({ queryKey: ['gitops', 'resources'] })
        },
      )
    }

    setup()
    return () => {
      unlisten?.()
    }
  }, [qc])
}
