import { useLiveQuery } from '@tanstack/react-db'
import { gitStatusCollection } from '@/collections/git-status'
import type { GitStatus } from '@/types/gitops'

export function useGitStatus(): { data: GitStatus | null; isLoading: boolean } {
  const result = useLiveQuery((q) => q.from({ gs: gitStatusCollection }))
  const items = result.data ?? []
  return {
    data: items.length > 0 ? (items[0] as GitStatus) : null,
    isLoading: items.length === 0,
  }
}
