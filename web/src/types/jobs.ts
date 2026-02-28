export type JobPhase =
  | 'queued'
  | 'processing'
  | 'extract_variables'
  | 'categorizing'
  | 'substituting'
  | 'storing'
  | 'creating_symlinks'
  | 'archiving'
  | 'completed'
  | 'failed'

export type JobStatus = 'processing' | 'completed' | 'failed' | 'superseded'

export interface JobProgressEvent {
  jobId: string
  filename: string
  phase: JobPhase
  status: JobStatus
  message: string
  timestamp: string
  outputPath?: string
  archivePath?: string
  symlinks: string[]
  category?: string
  error?: string
  sourcePath?: string
  sourceName?: string
  mimeType?: string
}

export interface StoredJob {
  jobId: string
  filename: string
  status: JobStatus
  currentPhase: JobPhase
  startedAt: string
  completedAt?: string
  outputPath?: string
  archivePath?: string
  symlinks: string[]
  category?: string
  error?: string
  message: string
  sourcePath?: string
  sourceName?: string
  ignored?: boolean
  mimeType?: string
}

export interface JobsResponse {
  jobs: StoredJob[]
  processingCount: number
  completedCount: number
  failedCount: number
}

// Query parameters for filtering jobs
export interface JobQueryParams {
  status?: string
  category?: string
  sourceName?: string
  fromDate?: string
  toDate?: string
  limit?: number
  offset?: number
}

// Response from /jobs/query endpoint
export interface JobListResponse {
  jobs: StoredJob[]
  total: number
  limit?: number
  offset?: number
}

// OCR text response
export interface OcrResponse {
  text: string
}

// Rerun single job response
export interface RerunResponse {
  jobId: string
}

// Rerun bulk response
export interface RerunResult {
  submitted: number
  errors: number
}

export function getPhaseLabel(phase: JobPhase): string {
  switch (phase) {
    case 'queued':
      return 'Queued'
    case 'processing':
      return 'Running OCR...'
    case 'extract_variables':
      return 'Extracting variables...'
    case 'categorizing':
      return 'Categorizing...'
    case 'substituting':
      return 'Substituting variables...'
    case 'storing':
      return 'Storing document...'
    case 'creating_symlinks':
      return 'Creating symlinks...'
    case 'archiving':
      return 'Archiving source...'
    case 'completed':
      return 'Completed'
    case 'failed':
      return 'Failed'
    default:
      return phase
  }
}

/** MIME type to human-readable label mapping */
const MIME_TYPE_LABELS: Record<string, string> = {
  'application/pdf': 'PDF',
  'image/png': 'PNG',
  'image/jpeg': 'JPEG',
  // image/jpg is a non-IANA alias but commonly used in practice
  'image/jpg': 'JPEG',
  'image/gif': 'GIF',
  'image/webp': 'WebP',
  'image/tiff': 'TIFF',
  'image/bmp': 'BMP',
  'text/plain': 'Text',
  'text/html': 'HTML',
  'application/xml': 'XML',
  'application/json': 'JSON',
  'application/msword': 'Word',
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document': 'Word',
  'application/vnd.ms-excel': 'Excel',
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet': 'Excel',
  'application/octet-stream': 'Binary',
}

export function getMimeTypeLabel(mimeType: string | null | undefined): string {
  // Handle undefined, null, and empty/whitespace-only strings
  const trimmed = mimeType?.trim()
  if (!trimmed) return 'Unknown'

  // Check for known MIME type
  if (trimmed in MIME_TYPE_LABELS) {
    return MIME_TYPE_LABELS[trimmed]
  }

  // Fallback: extract subtype if valid MIME format (type/subtype)
  if (trimmed.includes('/')) {
    const subtype = trimmed.split('/').pop()
    return subtype || 'Unknown'
  }

  return 'Unknown'
}

/** Check if a MIME type requires OCR for text extraction */
export function requiresOcr(mimeType: string | null | undefined): boolean {
  const trimmed = mimeType?.trim()?.toLowerCase()
  if (!trimmed) return true // Unknown = assume OCR

  // All image types require OCR
  if (trimmed.startsWith('image/')) return true

  // PDF may or may not require OCR (backend handles this smartly)
  // We'll label it as OCR since that's the common case
  if (trimmed === 'application/pdf') return true

  // Text-based formats don't require OCR
  return false
}
