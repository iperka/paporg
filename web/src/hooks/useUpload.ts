import { useState, useCallback } from 'react'
import type { UploadResponse } from '@/types/upload'
import type { ApiResponse } from '@/types/api'

interface UseUploadReturn {
  /** Upload one or more files. */
  uploadFiles: (files: File[]) => Promise<UploadResponse>
  /** Whether an upload is in progress. */
  uploading: boolean
  /** Error message if the last upload failed. */
  error: string | null
  /** Reset error state. */
  clearError: () => void
}

export function useUpload(): UseUploadReturn {
  const [uploading, setUploading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const uploadFiles = useCallback(async (files: File[]): Promise<UploadResponse> => {
    if (files.length === 0) {
      const errorMsg = 'No files provided'
      setError(errorMsg)
      throw new Error(errorMsg)
    }

    setUploading(true)
    setError(null)

    try {
      const formData = new FormData()

      // Use single or multiple endpoint based on file count
      const endpoint = files.length === 1 ? '/api/upload/single' : '/api/upload/multiple'
      const fieldName = files.length === 1 ? 'file' : 'files'

      files.forEach((file) => {
        formData.append(fieldName, file)
      })

      const response = await fetch(endpoint, {
        method: 'POST',
        body: formData,
        // Don't set Content-Type header - browser will set it with boundary
      })

      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || `Upload failed: ${response.status}`)
      }

      const result: ApiResponse<UploadResponse> = await response.json()

      if (!result.success) {
        throw new Error(result.error || 'Upload failed')
      }

      if (!result.data) {
        throw new Error('Invalid response from server')
      }

      return result.data
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Upload failed'
      setError(message)
      // Return empty response - consumers should check error state
      return { jobIds: [], uploaded: 0, errors: [message] }
    } finally {
      setUploading(false)
    }
  }, [])

  const clearError = useCallback(() => {
    setError(null)
  }, [])

  return {
    uploadFiles,
    uploading,
    error,
    clearError,
  }
}
