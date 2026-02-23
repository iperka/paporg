import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook } from '@testing-library/react'
import { isUnsortedCategory, isUnsortedJob } from '@/utils/jobs'
import type { StoredJob } from '@/types/jobs'
import type { FileTreeNode } from '@/types/gitops'

// Mock the contexts
vi.mock('@/contexts/JobsContext', () => ({
  useJobsContext: vi.fn(() => ({
    jobs: new Map(),
    isConnected: true,
    error: null,
    processingJobs: [],
    completedJobs: [],
    failedJobs: [],
  })),
}))

vi.mock('@/queries/use-file-tree', () => ({
  useFileTree: vi.fn(() => ({
    data: null,
    isLoading: false,
  })),
}))

// Test helper functions that useStats depends on
describe('isUnsortedCategory', () => {
  it('should return false for null/undefined', () => {
    expect(isUnsortedCategory(null)).toBe(false)
    expect(isUnsortedCategory(undefined)).toBe(false)
  })

  it('should return true for "unsorted"', () => {
    expect(isUnsortedCategory('unsorted')).toBe(true)
    expect(isUnsortedCategory('Unsorted')).toBe(true)
    expect(isUnsortedCategory('UNSORTED')).toBe(true)
  })

  it('should return true for paths ending with /unsorted', () => {
    expect(isUnsortedCategory('documents/unsorted')).toBe(true)
    expect(isUnsortedCategory('docs/archive/unsorted')).toBe(true)
  })

  it('should return false for other categories', () => {
    expect(isUnsortedCategory('invoices')).toBe(false)
    expect(isUnsortedCategory('documents')).toBe(false)
    expect(isUnsortedCategory('unsorted/documents')).toBe(false) // unsorted at start, not end
  })
})

describe('isUnsortedJob', () => {
  const createJob = (category: string | undefined): StoredJob => ({
    jobId: 'test-id',
    filename: 'test.pdf',
    status: 'completed',
    currentPhase: 'completed',
    startedAt: '2024-01-01T00:00:00Z',
    symlinks: [],
    message: '',
    category,
  })

  it('should return true for unsorted jobs', () => {
    expect(isUnsortedJob(createJob('unsorted'))).toBe(true)
    expect(isUnsortedJob(createJob('docs/unsorted'))).toBe(true)
  })

  it('should return false for sorted jobs', () => {
    expect(isUnsortedJob(createJob('invoices'))).toBe(false)
    expect(isUnsortedJob(createJob('contracts'))).toBe(false)
  })

  it('should return false for jobs without category', () => {
    expect(isUnsortedJob(createJob(undefined))).toBe(false)
  })
})

// Test the actual hook with mocked contexts
describe('useStats', () => {
  beforeEach(() => {
    vi.resetModules()
  })

  it('should return dashboard stats from mocked contexts', async () => {
    const { useJobsContext } = await import('@/contexts/JobsContext')
    const { useFileTree } = await import('@/queries/use-file-tree')

    const mockJobs = new Map<string, StoredJob>([
      ['job-1', {
        jobId: 'job-1',
        filename: 'test1.pdf',
        status: 'completed',
        currentPhase: 'completed',
        startedAt: '2024-01-01T00:00:00Z',
        symlinks: [],
        message: 'Completed',
        category: 'invoices',
      }],
      ['job-2', {
        jobId: 'job-2',
        filename: 'test2.pdf',
        status: 'processing',
        currentPhase: 'processing',
        startedAt: '2024-01-01T00:01:00Z',
        symlinks: [],
        message: 'Processing',
      }],
      ['job-3', {
        jobId: 'job-3',
        filename: 'test3.pdf',
        status: 'completed',
        currentPhase: 'completed',
        startedAt: '2024-01-01T00:02:00Z',
        symlinks: [],
        message: 'Completed',
        category: 'unsorted',
      }],
    ])

    vi.mocked(useJobsContext).mockReturnValue({
      jobs: mockJobs,
      isConnected: true,
      error: null,
      reconnect: vi.fn(),
      processingJobs: [mockJobs.get('job-2')!],
      completedJobs: [mockJobs.get('job-1')!, mockJobs.get('job-3')!],
      failedJobs: [],
    })

    const mockFileTree: FileTreeNode = {
      name: 'config',
      path: '/config',
      isDirectory: true,
      children: [
        {
          name: 'rule1.yaml',
          path: '/config/rule1.yaml',
          isDirectory: false,
          children: [],
          resource: { kind: 'Rule', name: 'rule1' },
        },
        {
          name: 'rule2.yaml',
          path: '/config/rule2.yaml',
          isDirectory: false,
          children: [],
          resource: { kind: 'Rule', name: 'rule2' },
        },
        {
          name: 'settings.yaml',
          path: '/config/settings.yaml',
          isDirectory: false,
          children: [],
          resource: { kind: 'Settings', name: 'settings' },
        },
      ],
    }

    vi.mocked(useFileTree).mockReturnValue({
      data: mockFileTree,
      isLoading: false,
    })

    // Import useStats after mocks are set up
    const { useStats } = await import('./useStats')

    const { result } = renderHook(() => useStats())

    expect(result.current.totalDocuments).toBe(3)
    expect(result.current.processingCount).toBe(1)
    expect(result.current.completedCount).toBe(2)
    expect(result.current.failedCount).toBe(0)
    expect(result.current.rulesCount).toBe(2) // 2 rules in tree
    expect(result.current.unsortedCount).toBe(1) // job-3 is unsorted
    expect(result.current.isJobsConnected).toBe(true)
    expect(result.current.isGitOpsConnected).toBe(true)
  })
})
