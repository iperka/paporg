import { useState, useCallback } from 'react'
import { api } from '@/api'
import { useIsMounted } from '@/hooks/useIsMounted'
import type { AiStatus, RuleSuggestion, ExistingRuleSummary } from '@/types/ai'

export interface UseAiSuggestionsReturn {
  /** Current AI status. */
  status: AiStatus | null
  /** Whether the status is loading. */
  statusLoading: boolean
  /** Whether suggestions are being generated. */
  suggestionsLoading: boolean
  /** Whether the model is being downloaded. */
  downloading: boolean
  /** Current suggestions. */
  suggestions: RuleSuggestion[]
  /** Error message if any. */
  error: string | null
  /** Fetch AI status. */
  fetchStatus: () => Promise<void>
  /** Download the AI model. */
  downloadModel: () => Promise<boolean>
  /** Get rule suggestions for a document. */
  getSuggestions: (ocrText: string, filename: string, existingRules?: ExistingRuleSummary[]) => Promise<RuleSuggestion[]>
  /** Clear suggestions and errors. */
  clear: () => void
}

export function useAiSuggestions(): UseAiSuggestionsReturn {
  const [status, setStatus] = useState<AiStatus | null>(null)
  const [statusLoading, setStatusLoading] = useState(false)
  const [suggestionsLoading, setSuggestionsLoading] = useState(false)
  const [downloading, setDownloading] = useState(false)
  const [suggestions, setSuggestions] = useState<RuleSuggestion[]>([])
  const [error, setError] = useState<string | null>(null)

  const isMounted = useIsMounted()

  const fetchStatus = useCallback(async () => {
    setStatusLoading(true)
    setError(null)
    try {
      const data = await api.ai.getStatus()
      if (!isMounted()) return
      setStatus({
        available: data.available,
        modelLoaded: data.modelLoaded,
        modelName: data.modelName ?? undefined,
        error: data.error ?? undefined,
      })
    } catch (err) {
      if (!isMounted()) return
      setError(err instanceof Error ? err.message : 'Failed to fetch AI status')
    } finally {
      if (isMounted()) {
        setStatusLoading(false)
      }
    }
  }, [isMounted])

  const downloadModel = useCallback(async () => {
    setDownloading(true)
    setError(null)
    try {
      const result = await api.ai.downloadModel('default')
      if (!isMounted()) return false
      if (result.status === 'completed') {
        await fetchStatus()
        return true
      } else {
        setError(result.message)
        return false
      }
    } catch (err) {
      if (!isMounted()) return false
      setError(err instanceof Error ? err.message : 'Failed to download model')
      return false
    } finally {
      if (isMounted()) {
        setDownloading(false)
      }
    }
  }, [isMounted, fetchStatus])

  const getSuggestions = useCallback(async (ocrText: string, filename: string, _existingRules?: ExistingRuleSummary[]) => {
    setSuggestionsLoading(true)
    setError(null)
    try {
      const suggestion = await api.ai.suggestRule(ocrText, filename)
      if (!isMounted()) return []
      // Convert single suggestion to array for compatibility
      const suggestionArray: RuleSuggestion[] = [{
        name: suggestion.name,
        category: suggestion.category,
        matchType: 'pattern',
        matchValue: suggestion.pattern,
        confidence: suggestion.confidence,
        explanation: suggestion.explanation,
      }]
      setSuggestions(suggestionArray)
      return suggestionArray
    } catch (err) {
      if (!isMounted()) return []
      setError(err instanceof Error ? err.message : 'Failed to get suggestions')
      return []
    } finally {
      if (isMounted()) {
        setSuggestionsLoading(false)
      }
    }
  }, [isMounted])

  const clear = useCallback(() => {
    setSuggestions([])
    setError(null)
  }, [])

  return {
    status,
    statusLoading,
    suggestionsLoading,
    downloading,
    suggestions,
    error,
    fetchStatus,
    downloadModel,
    getSuggestions,
    clear,
  }
}
