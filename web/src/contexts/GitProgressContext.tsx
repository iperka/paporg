import React, { createContext, useContext } from 'react'
import { useGitProgress } from '@/hooks/useGitProgress'
import type { GitProgressEvent } from '@/types/gitops'

interface GitProgressContextValue {
  /** Map of active operations by operationId */
  activeOperations: Map<string, GitProgressEvent>
  /** Whether there are any active operations */
  hasActiveOperations: boolean
  /** Whether SSE connection is established */
  isConnected: boolean
  /** Connection error if any */
  error: string | null
  /** Manually reconnect to SSE */
  reconnect: () => void
  /** Clear completed/failed operations */
  clearCompleted: () => void
}

const GitProgressContext = createContext<GitProgressContextValue | null>(null)

export function useGitProgressContext(): GitProgressContextValue {
  const context = useContext(GitProgressContext)
  if (!context) {
    throw new Error('useGitProgressContext must be used within a GitProgressProvider')
  }
  return context
}

interface GitProgressProviderProps {
  children: React.ReactNode
}

export function GitProgressProvider({ children }: GitProgressProviderProps) {
  const gitProgress = useGitProgress()

  return (
    <GitProgressContext.Provider value={gitProgress}>{children}</GitProgressContext.Provider>
  )
}
