/** Shared API response types. */

/** Successful API response. */
export interface SuccessResponse<T> {
  /** Whether the request was successful. */
  success: true
  /** Response data. */
  data: T
}

/** Failed API response. */
export interface ErrorResponse {
  /** Whether the request was successful. */
  success: false
  /** Error message. */
  error: string
}

/** Standard API response wrapper. */
export type ApiResponse<T> = SuccessResponse<T> | ErrorResponse
