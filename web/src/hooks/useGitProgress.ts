import { useState, useEffect, useCallback, useRef } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { GitProgressEvent } from '@/types/gitops'

interface UseGitProgressReturn {
  /** Map of active operations by operationId */
  activeOperations: Map<string, GitProgressEvent>
  /** Whether there are any active operations */
  hasActiveOperations: boolean
  /** Whether event connection is established */
  isConnected: boolean
  /** Connection error if any */
  error: string | null
  /** Manually reconnect */
  reconnect: () => void
  /** Clear completed/failed operations */
  clearCompleted: () => void
}

const COMPLETION_DISPLAY_TIME = 3000 // How long to show completed operations (ms)

// Valid operation types and phases for validation
const VALID_OPERATION_TYPES = new Set([
  'commit', 'push', 'pull', 'fetch', 'merge', 'checkout', 'initialize'
])
const VALID_PHASES = new Set([
  'starting', 'staging_files', 'committing', 'counting', 'compressing',
  'writing', 'receiving', 'resolving', 'unpacking', 'pushing', 'pulling',
  'fetching', 'merging', 'checking_out', 'completed', 'failed'
])

/**
 * Git progress event from Tauri backend.
 * The Rust struct uses #[serde(rename_all = "camelCase")] so fields are already camelCase.
 */
interface TauriGitProgressEvent {
  operationId: string
  operationType: string
  phase: string
  message: string
  progress?: number
  current?: number
  total?: number
  bytesTransferred?: number
  transferSpeed?: number
  rawOutput?: string
  error?: string
  timestamp: string
}

/**
 * Validate and convert Tauri GitProgressEvent to frontend type.
 * Uses fallback values for unrecognized operation types or phases.
 */
function convertTauriEvent(event: TauriGitProgressEvent): GitProgressEvent {
  // Validate operationType with fallback
  const operationType = VALID_OPERATION_TYPES.has(event.operationType)
    ? (event.operationType as GitProgressEvent['operationType'])
    : 'commit' // Fallback to a safe default

  // Validate phase with fallback
  const phase = VALID_PHASES.has(event.phase)
    ? (event.phase as GitProgressEvent['phase'])
    : 'starting' // Fallback to a safe default

  return {
    operationId: event.operationId,
    operationType,
    phase,
    message: event.message,
    progress: event.progress,
    total: event.total,
    // Use backend timestamp instead of synthesizing client-side
    timestamp: event.timestamp || new Date().toISOString(),
  }
}

export function useGitProgress(): UseGitProgressReturn {
  const [activeOperations, setActiveOperations] = useState<Map<string, GitProgressEvent>>(
    new Map()
  )
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Counter to trigger effect re-run on reconnect
  const [retryCount, setRetryCount] = useState(0)

  const completionTimersRef = useRef<Map<string, NodeJS.Timeout>>(new Map())
  const unlistenRef = useRef<UnlistenFn | null>(null)

  const handleGitProgress = useCallback((event: GitProgressEvent) => {
    setActiveOperations((prev) => {
      const next = new Map(prev)
      next.set(event.operationId, event)
      return next
    })

    // Auto-remove completed/failed operations after delay
    if (event.phase === 'completed' || event.phase === 'failed') {
      // Clear any existing timer for this operation
      const existingTimer = completionTimersRef.current.get(event.operationId)
      if (existingTimer) {
        clearTimeout(existingTimer)
      }

      // Set new timer to remove operation
      const timer = setTimeout(() => {
        setActiveOperations((prev) => {
          const next = new Map(prev)
          next.delete(event.operationId)
          return next
        })
        completionTimersRef.current.delete(event.operationId)
      }, COMPLETION_DISPLAY_TIME)

      completionTimersRef.current.set(event.operationId, timer)
    }
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
        const unlisten = await listen<TauriGitProgressEvent>('paporg://git-progress', (event) => {
          if (isMounted) {
            handleGitProgress(convertTauriEvent(event.payload))
          }
        })
        unlistenRef.current = unlisten
        if (isMounted) {
          setIsConnected(true)
          setError(null)
        }
      } catch (e) {
        console.error('Failed to set up git progress listener:', e)
        if (isMounted) {
          setIsConnected(false)
          setError(e instanceof Error ? e.message : 'Failed to connect')
        }
      }
    }

    setup()

    // Copy ref values for cleanup
    const completionTimers = completionTimersRef.current

    return () => {
      isMounted = false
      unlistenRef.current?.()
      unlistenRef.current = null

      // Clear all completion timers
      completionTimers.forEach((timer) => clearTimeout(timer))
      completionTimers.clear()
    }
  }, [handleGitProgress, retryCount])

  const reconnect = useCallback(() => {
    // Clear error and trigger effect re-run to re-establish listener
    setError(null)
    setRetryCount((c) => c + 1)
  }, [])

  const clearCompleted = useCallback(() => {
    setActiveOperations((prev) => {
      const next = new Map(prev)
      for (const [id, op] of next) {
        if (op.phase === 'completed' || op.phase === 'failed') {
          next.delete(id)
          // Clear any pending timer
          const timer = completionTimersRef.current.get(id)
          if (timer) {
            clearTimeout(timer)
            completionTimersRef.current.delete(id)
          }
        }
      }
      return next
    })
  }, [])

  return {
    activeOperations,
    hasActiveOperations: activeOperations.size > 0,
    isConnected,
    error,
    reconnect,
    clearCompleted,
  }
}
