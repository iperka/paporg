import { useLiveQuery } from '@tanstack/react-db'
import { branchesCollection } from '@/collections/branches'
import type { BranchInfo } from '@/types/gitops'

export function useBranches(): { data: BranchInfo[] } {
  const result = useLiveQuery((q) => q.from({ b: branchesCollection }))
  return {
    data: (result.data ?? []) as BranchInfo[],
  }
}
