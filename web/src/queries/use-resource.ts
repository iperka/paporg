import { useQuery } from '@tanstack/react-query'
import { api, type ResourceInfo } from '@/api'

export function useResource(kind: string, name: string, enabled = true): {
  data: ResourceInfo | null
  isLoading: boolean
  refetch: () => void
} {
  const result = useQuery({
    queryKey: ['gitops', 'resource', kind, name],
    queryFn: () => api.gitops.getResource(kind, name),
    enabled: enabled && !!kind && !!name,
  })

  return {
    data: result.data ?? null,
    isLoading: result.isLoading,
    refetch: result.refetch,
  }
}
