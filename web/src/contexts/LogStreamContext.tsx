import { createContext, useContext, ReactNode } from 'react'
import { useLogStream } from '@/api/logs'
import type { LogEvent } from '@/types/config'

interface LogStreamContextValue {
  logs: LogEvent[]
  isConnected: boolean
  error: string | null
  clearLogs: () => void
  reconnect: () => void
}

const LogStreamContext = createContext<LogStreamContextValue | null>(null)

export function LogStreamProvider({ children }: { children: ReactNode }) {
  const sseState = useLogStream({ maxLogs: 1000 })
  return <LogStreamContext.Provider value={sseState}>{children}</LogStreamContext.Provider>
}

export function useLogs() {
  const context = useContext(LogStreamContext)
  if (!context) throw new Error('useLogs must be used within LogStreamProvider')
  return context
}
