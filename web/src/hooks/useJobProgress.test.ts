import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useJobProgress } from './useJobProgress'

// Get the mocked listen function
const mockListen = vi.mocked((await import('@tauri-apps/api/event')).listen)

describe('useJobProgress', () => {
  const mockUnlisten = vi.fn()
  let eventCallback: ((event: { payload: unknown }) => void) | null = null

  beforeEach(() => {
    vi.clearAllMocks()
    eventCallback = null

    // Capture the callback when listen is called
    mockListen.mockImplementation(async (_event, callback) => {
      eventCallback = callback as (event: { payload: unknown }) => void
      return mockUnlisten as unknown as () => void
    })
  })

  it('should initialize with empty state', () => {
    const { result } = renderHook(() => useJobProgress())

    expect(result.current.jobs.size).toBe(0)
    expect(result.current.error).toBeNull()
    expect(result.current.processingJobs).toEqual([])
    expect(result.current.completedJobs).toEqual([])
    expect(result.current.failedJobs).toEqual([])
  })

  it('should set up Tauri event listener on mount', async () => {
    renderHook(() => useJobProgress())

    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('paporg://job-progress', expect.any(Function))
    })
  })

  it('should add job on progress event', async () => {
    const { result } = renderHook(() => useJobProgress())

    await waitFor(() => {
      expect(eventCallback).not.toBeNull()
    })

    act(() => {
      eventCallback!({
        payload: {
          jobId: 'job-1',
          sourcePath: '/input/test.pdf',
          filename: 'test.pdf',
          status: 'processing',
          phase: 'ocr',
          message: 'Processing...',
          timestamp: '2024-01-01T00:00:00Z',
        },
      })
    })

    expect(result.current.jobs.has('job-1')).toBe(true)
    expect(result.current.jobs.get('job-1')?.status).toBe('processing')
    expect(result.current.processingJobs.length).toBe(1)
  })

  it('should update existing job on progress event', async () => {
    const { result } = renderHook(() => useJobProgress())

    await waitFor(() => {
      expect(eventCallback).not.toBeNull()
    })

    // Initial job
    act(() => {
      eventCallback!({
        payload: {
          jobId: 'job-1',
          sourcePath: '/input/test.pdf',
          filename: 'test.pdf',
          status: 'processing',
          phase: 'ocr',
          message: 'Processing...',
        },
      })
    })

    expect(result.current.jobs.get('job-1')?.status).toBe('processing')

    // Update to completed
    act(() => {
      eventCallback!({
        payload: {
          jobId: 'job-1',
          sourcePath: '/input/test.pdf',
          filename: 'test.pdf',
          status: 'completed',
          phase: 'done',
          message: 'Done',
          outputPath: '/output/test.pdf',
        },
      })
    })

    expect(result.current.jobs.get('job-1')?.status).toBe('completed')
    expect(result.current.completedJobs.length).toBe(1)
    expect(result.current.processingJobs.length).toBe(0)
  })

  it('should categorize failed jobs', async () => {
    const { result } = renderHook(() => useJobProgress())

    await waitFor(() => {
      expect(eventCallback).not.toBeNull()
    })

    act(() => {
      eventCallback!({
        payload: {
          jobId: 'job-1',
          sourcePath: '/input/test.pdf',
          filename: 'test.pdf',
          status: 'failed',
          phase: 'error',
          message: 'OCR failed',
          error: 'Some error',
        },
      })
    })

    expect(result.current.failedJobs.length).toBe(1)
    expect(result.current.failedJobs[0].error).toBe('Some error')
  })

  it('should clean up listener on unmount', async () => {
    const { unmount } = renderHook(() => useJobProgress())

    await waitFor(() => {
      expect(mockListen).toHaveBeenCalled()
    })

    unmount()

    expect(mockUnlisten).toHaveBeenCalled()
  })

  it('should reconnect when reconnect is called', async () => {
    const { result } = renderHook(() => useJobProgress())

    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledTimes(1)
    })

    act(() => {
      result.current.reconnect()
    })

    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledTimes(2)
    })
  })

  it('should sort jobs by timestamp (newest first)', async () => {
    const { result } = renderHook(() => useJobProgress())

    await waitFor(() => {
      expect(eventCallback).not.toBeNull()
    })

    act(() => {
      eventCallback!({
        payload: {
          jobId: 'job-old',
          sourcePath: '/input/old.pdf',
          filename: 'old.pdf',
          status: 'completed',
          phase: 'done',
          message: 'Done',
          timestamp: '2024-01-01T00:00:00Z',
        },
      })
      eventCallback!({
        payload: {
          jobId: 'job-new',
          sourcePath: '/input/new.pdf',
          filename: 'new.pdf',
          status: 'completed',
          phase: 'done',
          message: 'Done',
          timestamp: '2024-01-02T00:00:00Z',
        },
      })
    })

    expect(result.current.completedJobs[0].jobId).toBe('job-new')
    expect(result.current.completedJobs[1].jobId).toBe('job-old')
  })
})
