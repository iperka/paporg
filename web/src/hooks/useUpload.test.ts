import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useUpload } from './useUpload'

describe('useUpload', () => {
  beforeEach(() => {
    vi.resetAllMocks()
  })

  it('should initialize with default state', () => {
    const { result } = renderHook(() => useUpload())

    expect(result.current.uploading).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('should set error when no files provided', async () => {
    const { result } = renderHook(() => useUpload())

    await act(async () => {
      try {
        await result.current.uploadFiles([])
      } catch {
        // Expected to throw
      }
    })

    expect(result.current.error).toBe('No files provided')
    expect(result.current.uploading).toBe(false)
  })

  it('should upload single file to /api/upload/single', async () => {
    const mockResponse = {
      success: true,
      data: { jobIds: ['job-1'], uploaded: 1, errors: [] },
    }

    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockResponse),
    })

    const { result } = renderHook(() => useUpload())

    const file = new File(['test content'], 'test.pdf', { type: 'application/pdf' })

    let response: Awaited<ReturnType<typeof result.current.uploadFiles>>
    await act(async () => {
      response = await result.current.uploadFiles([file])
    })

    expect(global.fetch).toHaveBeenCalledWith('/api/upload/single', expect.objectContaining({
      method: 'POST',
    }))
    expect(response!.jobIds).toEqual(['job-1'])
    expect(response!.uploaded).toBe(1)
    expect(result.current.uploading).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('should upload multiple files to /api/upload/multiple', async () => {
    const mockResponse = {
      success: true,
      data: { jobIds: ['job-1', 'job-2'], uploaded: 2, errors: [] },
    }

    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockResponse),
    })

    const { result } = renderHook(() => useUpload())

    const files = [
      new File(['test 1'], 'test1.pdf', { type: 'application/pdf' }),
      new File(['test 2'], 'test2.pdf', { type: 'application/pdf' }),
    ]

    await act(async () => {
      await result.current.uploadFiles(files)
    })

    expect(global.fetch).toHaveBeenCalledWith('/api/upload/multiple', expect.objectContaining({
      method: 'POST',
    }))
  })

  it('should handle server error response', async () => {
    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: () => Promise.resolve('Internal Server Error'),
    })

    const { result } = renderHook(() => useUpload())

    const file = new File(['test'], 'test.pdf', { type: 'application/pdf' })

    await act(async () => {
      await result.current.uploadFiles([file])
    })

    expect(result.current.error).toBe('Internal Server Error')
    expect(result.current.uploading).toBe(false)
  })

  it('should handle API error in response body', async () => {
    const mockResponse = {
      success: false,
      error: 'File type not supported',
    }

    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockResponse),
    })

    const { result } = renderHook(() => useUpload())

    const file = new File(['test'], 'test.pdf', { type: 'application/pdf' })

    await act(async () => {
      await result.current.uploadFiles([file])
    })

    expect(result.current.error).toBe('File type not supported')
  })

  it('should clear error when clearError is called', async () => {
    const { result } = renderHook(() => useUpload())

    await act(async () => {
      try {
        await result.current.uploadFiles([])
      } catch {
        // Expected
      }
    })

    expect(result.current.error).not.toBeNull()

    act(() => {
      result.current.clearError()
    })

    expect(result.current.error).toBeNull()
  })

  it('should set uploading to true during upload', async () => {
    let resolvePromise: (value: unknown) => void
    const pendingPromise = new Promise((resolve) => {
      resolvePromise = resolve
    })

    global.fetch = vi.fn().mockReturnValueOnce(pendingPromise)

    const { result } = renderHook(() => useUpload())

    const file = new File(['test'], 'test.pdf', { type: 'application/pdf' })

    act(() => {
      result.current.uploadFiles([file])
    })

    await waitFor(() => {
      expect(result.current.uploading).toBe(true)
    })

    // Resolve the promise
    resolvePromise!({
      ok: true,
      json: () => Promise.resolve({ success: true, data: { jobIds: [], uploaded: 0, errors: [] } }),
    })

    await waitFor(() => {
      expect(result.current.uploading).toBe(false)
    })
  })
})
