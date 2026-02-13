import { useEffect, useRef, useState, useCallback } from 'react'

interface UseAutoSaveOptions {
  /** Data to watch for changes */
  data: unknown
  /** Save function to call */
  onSave: () => Promise<void>
  /** Debounce delay in ms (default: 1000) */
  delay?: number
  /** Whether auto-save is enabled (default: true) */
  enabled?: boolean
  /** Whether there are valid changes to save */
  hasChanges?: boolean
  /** Whether the resource is new (skip auto-save for new resources) */
  isNew?: boolean
}

interface UseAutoSaveReturn {
  /** Whether a save is in progress */
  isSaving: boolean
  /** Last saved timestamp */
  lastSaved: Date | null
  /** Status message */
  status: 'idle' | 'pending' | 'saving' | 'saved' | 'error'
  /** Error if save failed */
  error: string | null
  /** Manually trigger save */
  saveNow: () => Promise<void>
}

export function useAutoSave({
  data,
  onSave,
  delay = 1000,
  enabled = true,
  hasChanges = false,
  isNew = false,
}: UseAutoSaveOptions): UseAutoSaveReturn {
  const [isSaving, setIsSaving] = useState(false)
  const [lastSaved, setLastSaved] = useState<Date | null>(null)
  const [status, setStatus] = useState<'idle' | 'pending' | 'saving' | 'saved' | 'error'>('idle')
  const [error, setError] = useState<string | null>(null)

  const timeoutRef = useRef<NodeJS.Timeout | null>(null)
  const dataRef = useRef(data)
  const isMountedRef = useRef(true)

  // Track if data has changed
  useEffect(() => {
    dataRef.current = data
  }, [data])

  // Cleanup on unmount
  useEffect(() => {
    isMountedRef.current = true
    return () => {
      isMountedRef.current = false
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
      }
    }
  }, [])

  const saveNow = useCallback(async () => {
    if (!isMountedRef.current) return

    setIsSaving(true)
    setStatus('saving')
    setError(null)

    try {
      await onSave()
      if (isMountedRef.current) {
        setLastSaved(new Date())
        setStatus('saved')
        // Reset to idle after showing "saved" briefly
        setTimeout(() => {
          if (isMountedRef.current) {
            setStatus('idle')
          }
        }, 2000)
      }
    } catch (e) {
      if (isMountedRef.current) {
        setError(e instanceof Error ? e.message : 'Save failed')
        setStatus('error')
      }
    } finally {
      if (isMountedRef.current) {
        setIsSaving(false)
      }
    }
  }, [onSave])

  // Auto-save effect
  useEffect(() => {
    // Skip auto-save if disabled, no changes, or new resource
    if (!enabled || !hasChanges || isNew) {
      return
    }

    // Clear existing timeout
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
    }

    // Set pending status
    setStatus('pending')

    // Schedule save
    timeoutRef.current = setTimeout(() => {
      saveNow()
    }, delay)

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
      }
    }
  }, [data, enabled, hasChanges, isNew, delay, saveNow])

  return {
    isSaving,
    lastSaved,
    status,
    error,
    saveNow,
  }
}
