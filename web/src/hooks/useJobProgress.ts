import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { StoredJob } from '@/types/jobs'
import { api, type StoredJob as ApiStoredJob } from '@/api'

/**
 * Job progress event from Tauri backend.
 * The Rust struct uses #[serde(rename_all = "camelCase")] so fields are already camelCase.
 */
interface TauriJobProgressEvent {
  jobId: string
  sourcePath: string
  filename: string
  status: string
  phase: string
  message: string
  category?: string
  outputPath?: string
  archivePath?: string
  symlinks?: string[]
  error?: string
  ocrText?: string
  timestamp?: string
}

interface UseJobProgressReturn {
  /** Map of all jobs by jobId */
  jobs: Map<string, StoredJob>
  /** Whether event connection is established */
  isConnected: boolean
  /** Connection error if any */
  error: string | null
  /** Manually reconnect */
  reconnect: () => void
  /** All processing jobs */
  processingJobs: StoredJob[]
  /** All completed jobs */
  completedJobs: StoredJob[]
  /** All failed jobs */
  failedJobs: StoredJob[]
}

/**
 * Convert Tauri JobProgressEvent to StoredJob.
 * Uses backend timestamp if available, falls back to client time.
 */
function storedJobFromTauriEvent(event: TauriJobProgressEvent): StoredJob {
  const timestamp = event.timestamp || new Date().toISOString()

  return {
    jobId: event.jobId,
    filename: event.filename,
    status: event.status as StoredJob['status'],
    currentPhase: event.phase as StoredJob['currentPhase'],
    startedAt: timestamp,
    completedAt: event.status === 'completed' || event.status === 'failed' ? timestamp : undefined,
    outputPath: event.outputPath,
    archivePath: event.archivePath,
    category: event.category,
    error: event.error,
    message: event.message,
    symlinks: event.symlinks || [],
    sourcePath: event.sourcePath,
    ocrText: event.ocrText,
  }
}

function updateJobFromTauriEvent(existing: StoredJob, event: TauriJobProgressEvent): StoredJob {
  const timestamp = event.timestamp || new Date().toISOString()
  return {
    ...existing,
    status: event.status as StoredJob['status'],
    currentPhase: event.phase as StoredJob['currentPhase'],
    message: event.message,
    completedAt: event.status === 'completed' || event.status === 'failed' ? timestamp : existing.completedAt,
    outputPath: event.outputPath ?? existing.outputPath,
    archivePath: event.archivePath ?? existing.archivePath,
    symlinks: event.symlinks?.length ? event.symlinks : existing.symlinks,
    category: event.category ?? existing.category,
    error: event.error ?? existing.error,
    ocrText: event.ocrText ?? existing.ocrText,
  }
}

/**
 * Convert API StoredJob to hook StoredJob type.
 * Maps API field names to the hook's expected format.
 */
function storedJobFromApi(apiJob: ApiStoredJob): StoredJob {
  return {
    jobId: apiJob.id,
    filename: apiJob.filename,
    status: apiJob.status as StoredJob['status'],
    currentPhase: apiJob.status as StoredJob['currentPhase'], // Use status as phase for persisted jobs
    startedAt: apiJob.createdAt,
    completedAt: apiJob.updatedAt,
    outputPath: apiJob.outputPath ?? undefined,
    archivePath: apiJob.archivePath ?? undefined,
    category: apiJob.category ?? undefined,
    error: apiJob.errorMessage ?? undefined,
    message: apiJob.errorMessage || `Status: ${apiJob.status}`,
    symlinks: apiJob.symlinks || [],
    sourcePath: apiJob.sourcePath,
    ocrText: apiJob.ocrText ?? undefined,
    sourceName: apiJob.sourceName ?? undefined,
    mimeType: apiJob.mimeType ?? undefined,
  }
}

export function useJobProgress(): UseJobProgressReturn {
  const [jobs, setJobs] = useState<Map<string, StoredJob>>(new Map())
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Counter to trigger effect re-run on reconnect
  const [retryCount, setRetryCount] = useState(0)

  const unlistenRef = useRef<UnlistenFn | null>(null)

  // Handle Tauri job progress events
  const handleJobProgress = useCallback((event: TauriJobProgressEvent) => {
    setJobs((prev) => {
      const next = new Map(prev)
      const existing = next.get(event.jobId)

      if (existing) {
        next.set(event.jobId, updateJobFromTauriEvent(existing, event))
      } else {
        next.set(event.jobId, storedJobFromTauriEvent(event))
      }

      return next
    })
  }, [])

  // Set up Tauri event listener
  useEffect(() => {
    let isMounted = true

    const setup = async () => {
      // Cleanup previous listener if exists
      if (unlistenRef.current) {
        unlistenRef.current()
        unlistenRef.current = null
      }

      try {
        const unlisten = await listen<TauriJobProgressEvent>('paporg://job-progress', (event) => {
          if (isMounted) {
            handleJobProgress(event.payload)
          }
        })
        unlistenRef.current = unlisten
        if (isMounted) {
          setIsConnected(true)
          setError(null)
        }
      } catch (e) {
        console.error('Failed to set up job progress listener:', e)
        if (isMounted) {
          setIsConnected(false)
          setError(e instanceof Error ? e.message : 'Failed to connect')
        }
      }
    }

    setup()

    return () => {
      isMounted = false
      unlistenRef.current?.()
      unlistenRef.current = null
    }
  }, [handleJobProgress, retryCount])

  // Load initial jobs from database on mount
  useEffect(() => {
    let isMounted = true

    const loadInitialJobs = async () => {
      try {
        const storedJobs = await api.jobs.getAll()
        if (isMounted && storedJobs.length > 0) {
          setJobs((prev) => {
            const next = new Map(prev)
            for (const job of storedJobs) {
              // Only add if not already present (real-time events take priority)
              if (!next.has(job.id)) {
                next.set(job.id, storedJobFromApi(job))
              }
            }
            return next
          })
        }
      } catch (error) {
        console.error('Failed to load initial jobs:', error)
      }
    }

    loadInitialJobs()

    return () => {
      isMounted = false
    }
  }, [])

  const reconnect = useCallback(() => {
    // Clear error and trigger effect re-run to re-establish listener
    setError(null)
    setRetryCount((c) => c + 1)
  }, [])

  // Computed values
  const { processingJobs, completedJobs, failedJobs } = useMemo(() => {
    const processing: StoredJob[] = []
    const completed: StoredJob[] = []
    const failed: StoredJob[] = []

    for (const job of jobs.values()) {
      switch (job.status) {
        case 'processing':
          processing.push(job)
          break
        case 'completed':
          completed.push(job)
          break
        case 'failed':
          failed.push(job)
          break
      }
    }

    // Sort by startedAt (newest first)
    const sortByDate = (a: StoredJob, b: StoredJob) =>
      new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime()

    return {
      processingJobs: processing.sort(sortByDate),
      completedJobs: completed.sort(sortByDate),
      failedJobs: failed.sort(sortByDate),
    }
  }, [jobs])

  return {
    jobs,
    isConnected,
    error,
    reconnect,
    processingJobs,
    completedJobs,
    failedJobs,
  }
}
