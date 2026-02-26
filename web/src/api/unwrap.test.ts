import { describe, it, expect } from 'vitest'
import { unwrap } from './index'
import type { ApiResponse } from './index'

describe('unwrap', () => {
  it('returns data on success', async () => {
    const response: ApiResponse<string> = { success: true, data: 'hello' }
    const result = await unwrap(response)
    expect(result).toBe('hello')
  })

  it('returns null data as-is on success', async () => {
    const response: ApiResponse<null> = { success: true, data: null }
    const result = await unwrap(response)
    expect(result).toBeNull()
  })

  it('returns undefined data as-is on success', async () => {
    const response: ApiResponse<string> = { success: true }
    const result = await unwrap(response)
    expect(result).toBeUndefined()
  })

  it('throws with error message on failure', async () => {
    const response: ApiResponse<string> = {
      success: false,
      error: 'Something went wrong',
    }
    await expect(unwrap(response)).rejects.toThrow('Something went wrong')
  })

  it('throws "Unknown error" when failure has no error message', async () => {
    const response: ApiResponse<string> = { success: false }
    await expect(unwrap(response)).rejects.toThrow('Unknown error')
  })
})
