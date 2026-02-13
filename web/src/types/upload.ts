/** Response from upload endpoints. */
export interface UploadResponse {
  /** IDs of jobs created for the uploaded files. */
  jobIds: string[]
  /** Number of files successfully uploaded. */
  uploaded: number
  /** Error messages for files that failed to upload. */
  errors: string[]
}

/** Supported file extensions for upload. */
export const SUPPORTED_EXTENSIONS = [
  'pdf',
  'docx',
  'txt',
  'text',
  'md',
  'png',
  'jpg',
  'jpeg',
  'tiff',
  'tif',
  'bmp',
  'gif',
  'webp',
] as const

/** Maximum file size in bytes (50 MB). */
export const MAX_FILE_SIZE = 50 * 1024 * 1024

/** Image file extensions (subset of SUPPORTED_EXTENSIONS). */
const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'tiff', 'tif', 'bmp', 'gif', 'webp'] as const

/** File type categories for display.
 * NOTE: These must be subsets of SUPPORTED_EXTENSIONS - keep in sync!
 */
export const FILE_TYPE_CATEGORIES = {
  documents: SUPPORTED_EXTENSIONS.filter(
    ext => !IMAGE_EXTENSIONS.includes(ext as typeof IMAGE_EXTENSIONS[number])
  ),
  images: IMAGE_EXTENSIONS,
} as const

/** Returns whether a file extension is supported. */
export function isSupportedExtension(extension: string): boolean {
  return SUPPORTED_EXTENSIONS.includes(extension.toLowerCase() as typeof SUPPORTED_EXTENSIONS[number])
}

/** Gets the extension from a filename. Handles dotfiles (e.g., .gitignore) properly. */
export function getFileExtension(filename: string): string {
  const lastDot = filename.lastIndexOf('.')
  // No dot, dot at start (dotfile), or dot at end
  if (lastDot <= 0 || lastDot === filename.length - 1) {
    return ''
  }
  return filename.slice(lastDot + 1).toLowerCase()
}

/** Validates a file for upload. Returns error message or null if valid. */
export function validateFile(file: File): string | null {
  const extension = getFileExtension(file.name)

  if (!extension) {
    return `File "${file.name}" has no extension`
  }

  if (!isSupportedExtension(extension)) {
    return `Unsupported file type ".${extension}". Supported: ${SUPPORTED_EXTENSIONS.join(', ')}`
  }

  if (file.size > MAX_FILE_SIZE) {
    return `File "${file.name}" exceeds maximum size of ${MAX_FILE_SIZE / 1024 / 1024} MB`
  }

  return null
}

/** Human-readable file size. */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
