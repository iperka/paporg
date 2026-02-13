import { useMemo } from 'react'
import { useJobsContext } from '@/contexts/JobsContext'
import { useGitOps } from '@/contexts/GitOpsContext'
import { isUnsortedJob } from '@/utils/jobs'
import type { FileTreeNode } from '@/types/gitops'
import type { StoredJob } from '@/types/jobs'

export interface TimelineDataPoint {
  date: string
  count: number
  label: string
}

export interface SourceDataPoint {
  name: string
  value: number
  fill: string
}

export interface DashboardStats {
  totalDocuments: number
  processingCount: number
  completedCount: number
  failedCount: number
  rulesCount: number
  unsortedCount: number
  unsortedJobs: StoredJob[]
  isJobsConnected: boolean
  isGitOpsConnected: boolean
  jobsError: string | null
  timelineData: TimelineDataPoint[]
  sourceData: SourceDataPoint[]
}

/** Counts rules in a file tree using iterative DFS to avoid stack overflow on deep trees. */
function countRulesInTree(root: FileTreeNode | null): number {
  if (!root) return 0

  let count = 0
  const stack: FileTreeNode[] = [root]

  while (stack.length > 0) {
    const node = stack.pop()!
    if (node.resource?.kind === 'Rule') {
      count += 1
    }
    // Add children to stack in reverse order to maintain left-to-right traversal
    for (let i = node.children.length - 1; i >= 0; i--) {
      stack.push(node.children[i])
    }
  }

  return count
}


// Chart colors for sources
const SOURCE_COLORS = [
  'hsl(var(--chart-1))',
  'hsl(var(--chart-2))',
  'hsl(var(--chart-3))',
  'hsl(var(--chart-4))',
  'hsl(var(--chart-5))',
]

function computeTimelineData(jobs: Map<string, StoredJob>): TimelineDataPoint[] {
  const dateMap = new Map<string, number>()

  // Get last 14 days
  const today = new Date()
  for (let i = 13; i >= 0; i--) {
    const date = new Date(today)
    date.setDate(date.getDate() - i)
    const key = date.toISOString().split('T')[0]
    dateMap.set(key, 0)
  }

  // Count jobs per day
  for (const job of jobs.values()) {
    if (job.status === 'completed' || job.status === 'failed') {
      const timestamp = job.completedAt || job.startedAt
      if (!timestamp) {
        console.warn(`Job ${job.jobId} has no timestamp, skipping from timeline`)
        continue
      }
      const date = new Date(timestamp)
      if (isNaN(date.getTime())) {
        console.warn(`Invalid date "${timestamp}" for job ${job.jobId}, skipping from timeline`)
        continue
      }
      const key = date.toISOString().split('T')[0]
      if (dateMap.has(key)) {
        dateMap.set(key, (dateMap.get(key) || 0) + 1)
      }
    }
  }

  return Array.from(dateMap.entries()).map(([date, count]) => {
    const d = new Date(date)
    return {
      date,
      count,
      // Use undefined to default to browser's locale
      label: d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }),
    }
  })
}

function computeSourceData(jobs: Map<string, StoredJob>): SourceDataPoint[] {
  const sourceMap = new Map<string, number>()

  for (const job of jobs.values()) {
    const source = job.sourceName || 'Default'
    sourceMap.set(source, (sourceMap.get(source) || 0) + 1)
  }

  return Array.from(sourceMap.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([name, value], index) => ({
      name,
      value,
      fill: SOURCE_COLORS[index % SOURCE_COLORS.length],
    }))
}

export function useStats(): DashboardStats {
  const {
    jobs,
    isConnected: isJobsConnected,
    error: jobsError,
    processingJobs,
    completedJobs,
    failedJobs,
  } = useJobsContext()

  const { fileTree, isConnected: isGitOpsConnected } = useGitOps()

  const rulesCount = useMemo(() => countRulesInTree(fileTree), [fileTree])

  const unsortedJobs = useMemo(() => {
    return completedJobs.filter(
      (job) => isUnsortedJob(job) && !job.ignored
    )
  }, [completedJobs])

  const timelineData = useMemo(() => computeTimelineData(jobs), [jobs])
  const sourceData = useMemo(() => computeSourceData(jobs), [jobs])

  return {
    totalDocuments: jobs.size,
    processingCount: processingJobs.length,
    completedCount: completedJobs.length,
    failedCount: failedJobs.length,
    rulesCount,
    unsortedCount: unsortedJobs.length,
    unsortedJobs,
    isJobsConnected,
    isGitOpsConnected,
    jobsError,
    timelineData,
    sourceData,
  }
}
