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
    let cancelled = false

    const setup = async () => {
      const fn = await listen<ConfigChangeEvent>(
        'paporg://config-changed',
        (_event) => {
          qc.invalidateQueries({ queryKey: ['gitops', 'file-tree'] })
          qc.invalidateQueries({ queryKey: ['git', 'status'] })
          qc.invalidateQueries({ queryKey: ['gitops', 'settings'] })
          qc.invalidateQueries({ queryKey: ['gitops', 'resources'] })
        },
      )
      if (cancelled) {
        fn()
      } else {
        unlisten = fn
      }
    }

    setup()
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [qc])
}
