/**
 * Shared job utility functions.
 */

import type { StoredJob } from '@/types/jobs'

/**
 * Checks if a category string represents the "unsorted" category.
 * Matches "unsorted" exactly or any path ending with "/unsorted".
 */
export function isUnsortedCategory(category: string | undefined | null): boolean {
  if (!category) return false
  const normalized = category.toLowerCase()
  return normalized === 'unsorted' || normalized.endsWith('/unsorted')
}

/**
 * Checks if a job is in the unsorted category (convenience wrapper).
 */
export function isUnsortedJob(job: StoredJob): boolean {
  return isUnsortedCategory(job.category)
}
