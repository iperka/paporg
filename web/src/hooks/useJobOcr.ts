import { useState, useCallback, useRef, useEffect } from 'react'
import { api } from '@/api'

interface UseJobOcrReturn {
  /** OCR text content */
  ocrText: string | null
  /** Whether OCR is being fetched */
  loading: boolean
  /** Error message if any */
  error: string | null
  /** Fetch OCR text for a job. Returns cleanup function. */
  fetchOcr: (jobId: string) => void
  /** Clear OCR text and error */
  clear: () => void
}

/**
 * Hook to fetch OCR text on-demand for a job.
 * OCR text is not stored in the database - it's computed from archive files.
 */
export function useJobOcr(): UseJobOcrReturn {
  const [ocrText, setOcrText] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Track current job ID to ignore stale responses
  const currentJobIdRef = useRef<string | null>(null)
  // AbortController for cancelling in-flight requests
  const abortControllerRef = useRef<AbortController | null>(null)

  // Clean up abort controller on unmount
  useEffect(() => {
    return () => {
      abortControllerRef.current?.abort()
    }
  }, [])

  const fetchOcr = useCallback((jobId: string) => {
    if (!jobId) return

    // Abort any previous in-flight request
    abortControllerRef.current?.abort()

    // Create new abort controller for this request
    const abortController = new AbortController()
    abortControllerRef.current = abortController

    // Track current job ID to ignore stale responses
    currentJobIdRef.current = jobId

    setLoading(true)
    setError(null)
    setOcrText(null)

    api.jobs.getOcr(jobId)
      .then((response) => {
        // Ignore response if job ID changed or aborted
        if (currentJobIdRef.current !== jobId || abortController.signal.aborted) return
        setOcrText(response.text)
      })
      .catch((err: unknown) => {
        // Ignore if aborted or job ID changed
        if (abortController.signal.aborted) return
        if (currentJobIdRef.current !== jobId) return

        // Handle error safely (err could be anything)
        const message = err instanceof Error ? err.message : String(err)
        setError(message || 'Failed to fetch OCR text')
      })
      .finally(() => {
        // Only clear loading if still for the same job and not aborted
        if (currentJobIdRef.current === jobId && !abortController.signal.aborted) {
          setLoading(false)
        }
      })
  }, [])

  const clear = useCallback(() => {
    // Abort any in-flight request
    abortControllerRef.current?.abort()
    abortControllerRef.current = null
    currentJobIdRef.current = null
    setOcrText(null)
    setError(null)
    setLoading(false)
  }, [])

  return { ocrText, loading, error, fetchOcr, clear }
}
