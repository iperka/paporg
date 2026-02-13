import { useEffect, useState, useCallback } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { LogEvent } from '@/types/config'

interface UseLogStreamOptions {
  maxLogs?: number
  autoScroll?: boolean
}

interface UseLogStreamResult {
  logs: LogEvent[]
  isConnected: boolean
  error: string | null
  clearLogs: () => void
  reconnect: () => void
}

/**
 * Log event from Tauri (snake_case from Rust).
 */
interface TauriLogEvent {
  timestamp: string
  level: string
  target: string
  message: string
}

export function useLogStream(options: UseLogStreamOptions = {}): UseLogStreamResult {
  const { maxLogs = 1000 } = options
  const [logs, setLogs] = useState<LogEvent[]>([])
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Handle log events
  const handleLog = useCallback(
    (event: TauriLogEvent) => {
      const logEvent: LogEvent = {
        timestamp: event.timestamp,
        level: event.level,
        target: event.target,
        message: event.message,
      }

      setLogs((prev) => {
        const newLogs = [...prev, logEvent]
        // Keep only the last maxLogs entries
        if (newLogs.length > maxLogs) {
          return newLogs.slice(-maxLogs)
        }
        return newLogs
      })
    },
    [maxLogs]
  )

  // Set up Tauri event listener
  useEffect(() => {
    let unlisten: UnlistenFn | null = null

    const setup = async () => {
      try {
        unlisten = await listen<TauriLogEvent>('paporg://log', (event) => {
          handleLog(event.payload)
        })
        setIsConnected(true)
        setError(null)
      } catch (e) {
        console.error('Failed to set up log listener:', e)
        setIsConnected(false)
        setError(e instanceof Error ? e.message : 'Failed to connect')
      }
    }

    setup()

    return () => {
      unlisten?.()
    }
  }, [handleLog])

  const clearLogs = useCallback(() => {
    setLogs([])
  }, [])

  const reconnect = useCallback(() => {
    // In Tauri mode, reconnection happens automatically via event listener
    setError(null)
  }, [])

  return {
    logs,
    isConnected,
    error,
    clearLogs,
    reconnect,
  }
}
