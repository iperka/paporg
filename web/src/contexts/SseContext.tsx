import { createContext, useContext, ReactNode } from 'react'
import { useLogStream } from '@/api/logs'
import type { LogEvent } from '@/types/config'

interface SseContextValue {
  logs: LogEvent[]
  isConnected: boolean
  error: string | null
  clearLogs: () => void
  reconnect: () => void
}

const SseContext = createContext<SseContextValue | null>(null)

export function SseProvider({ children }: { children: ReactNode }) {
  const sseState = useLogStream({ maxLogs: 1000 })
  return <SseContext.Provider value={sseState}>{children}</SseContext.Provider>
}

export function useSse() {
  const context = useContext(SseContext)
  if (!context) throw new Error('useSse must be used within SseProvider')
  return context
}
