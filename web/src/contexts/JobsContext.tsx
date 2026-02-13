import React, { createContext, useContext } from 'react'
import { useJobProgress } from '@/hooks/useJobProgress'
import type { StoredJob } from '@/types/jobs'

interface JobsContextValue {
  /** Map of all jobs by jobId */
  jobs: Map<string, StoredJob>
  /** Whether SSE connection is established */
  isConnected: boolean
  /** Connection error if any */
  error: string | null
  /** Manually reconnect to SSE */
  reconnect: () => void
  /** All processing jobs */
  processingJobs: StoredJob[]
  /** All completed jobs */
  completedJobs: StoredJob[]
  /** All failed jobs */
  failedJobs: StoredJob[]
}

const JobsContext = createContext<JobsContextValue | null>(null)

export function useJobsContext(): JobsContextValue {
  const context = useContext(JobsContext)
  if (!context) {
    throw new Error('useJobsContext must be used within a JobsProvider')
  }
  return context
}

interface JobsProviderProps {
  children: React.ReactNode
}

export function JobsProvider({ children }: JobsProviderProps) {
  const jobsState = useJobProgress()

  return <JobsContext.Provider value={jobsState}>{children}</JobsContext.Provider>
}
