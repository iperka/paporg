import { useState, useCallback, useRef } from 'react'
import { Upload, X, FileText, Image, AlertCircle, CheckCircle2, Loader2 } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useToast } from '@/components/ui/use-toast'
import { useUpload } from '@/hooks/useUpload'
import {
  validateFile,
  formatFileSize,
  getFileExtension,
  FILE_TYPE_CATEGORIES,
  SUPPORTED_EXTENSIONS,
} from '@/types/upload'

interface UploadDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface SelectedFile {
  file: File
  error: string | null
}

function getFileIcon(filename: string) {
  const ext = getFileExtension(filename)
  if (FILE_TYPE_CATEGORIES.images.includes(ext as typeof FILE_TYPE_CATEGORIES.images[number])) {
    return <Image className="h-4 w-4" />
  }
  return <FileText className="h-4 w-4" />
}

/** Creates a unique key for a file using name, size, and lastModified timestamp. */
function getFileKey(file: File): string {
  return `${file.name}-${file.size}-${file.lastModified}`
}

export function UploadDialog({ open, onOpenChange }: UploadDialogProps) {
  const [selectedFiles, setSelectedFiles] = useState<SelectedFile[]>([])
  const [isDragging, setIsDragging] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const { toast } = useToast()
  const { uploadFiles, uploading } = useUpload()

  const addFiles = useCallback((files: FileList | File[]) => {
    const fileArray = Array.from(files)
    const newFiles: SelectedFile[] = fileArray.map((file) => ({
      file,
      error: validateFile(file),
    }))

    setSelectedFiles((prev) => {
      // Filter out duplicates using name + size + lastModified for stronger uniqueness
      const existingKeys = new Set(prev.map((f) => getFileKey(f.file)))
      const uniqueNew = newFiles.filter((f) => !existingKeys.has(getFileKey(f.file)))
      const duplicateCount = newFiles.length - uniqueNew.length

      if (duplicateCount > 0) {
        console.info(`Skipped ${duplicateCount} duplicate file(s)`)
      }

      return [...prev, ...uniqueNew]
    })
  }, [])

  const removeFile = useCallback((index: number) => {
    setSelectedFiles((prev) => prev.filter((_, i) => i !== index))
  }, [])

  const clearFiles = useCallback(() => {
    setSelectedFiles([])
  }, [])

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(false)
  }, [])

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      setIsDragging(false)

      if (e.dataTransfer.files.length > 0) {
        addFiles(e.dataTransfer.files)
      }
    },
    [addFiles]
  )

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (e.target.files && e.target.files.length > 0) {
        addFiles(e.target.files)
      }
      // Reset input so the same file can be selected again
      e.target.value = ''
    },
    [addFiles]
  )

  const handleUpload = useCallback(async () => {
    const validFiles = selectedFiles.filter((f) => !f.error).map((f) => f.file)

    if (validFiles.length === 0) {
      toast({
        variant: 'destructive',
        title: 'No valid files',
        description: 'Please add valid files before uploading.',
      })
      return
    }

    try {
      const result = await uploadFiles(validFiles)

      if (result.uploaded > 0) {
        toast({
          variant: 'success',
          title: 'Upload successful',
          description: `${result.uploaded} file${result.uploaded > 1 ? 's' : ''} uploaded for processing.`,
        })
      }

      if (result.errors.length > 0) {
        toast({
          variant: 'destructive',
          title: 'Some files failed',
          description: result.errors.join(', '),
        })
      }

      // Close dialog and clear files on success
      if (result.uploaded > 0) {
        clearFiles()
        onOpenChange(false)
      }
    } catch (err) {
      console.error('Upload failed:', err)
      toast({
        variant: 'destructive',
        title: 'Upload failed',
        description: err instanceof Error ? err.message : 'An error occurred while uploading files.',
      })
    }
  }, [selectedFiles, uploadFiles, toast, clearFiles, onOpenChange])

  const handleClose = useCallback(() => {
    if (!uploading) {
      clearFiles()
      onOpenChange(false)
    }
  }, [uploading, clearFiles, onOpenChange])

  const validFileCount = selectedFiles.filter((f) => !f.error).length
  const invalidFileCount = selectedFiles.filter((f) => f.error).length

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>Upload Documents</DialogTitle>
          <DialogDescription>
            Drop files here or click to select. Supported formats: PDF, DOCX, TXT, MD, and images.
          </DialogDescription>
        </DialogHeader>

        {/* Drop zone */}
        <div
          className={`relative flex min-h-[150px] cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed p-6 transition-colors ${
            isDragging
              ? 'border-primary bg-primary/5'
              : 'border-muted-foreground/25 hover:border-primary/50'
          }`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onClick={() => fileInputRef.current?.click()}
        >
          <input
            ref={fileInputRef}
            type="file"
            multiple
            accept={SUPPORTED_EXTENSIONS.map((ext) => `.${ext}`).join(',')}
            onChange={handleFileSelect}
            className="hidden"
          />

          <Upload className={`mb-2 h-10 w-10 ${isDragging ? 'text-primary' : 'text-muted-foreground'}`} />
          <p className="text-sm text-muted-foreground">
            {isDragging ? 'Drop files here' : 'Drag & drop files or click to browse'}
          </p>
          <p className="mt-1 text-xs text-muted-foreground">Max 50 MB per file</p>
        </div>

        {/* File list */}
        {selectedFiles.length > 0 && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <p className="text-sm font-medium">
                {selectedFiles.length} file{selectedFiles.length !== 1 ? 's' : ''} selected
                {invalidFileCount > 0 && (
                  <span className="ml-2 text-destructive">({invalidFileCount} invalid)</span>
                )}
              </p>
              <Button variant="ghost" size="sm" onClick={clearFiles} disabled={uploading}>
                Clear all
              </Button>
            </div>

            <ScrollArea className="h-[150px] rounded-md border">
              <div className="space-y-1 p-2">
                {selectedFiles.map((item, index) => (
                  <div
                    key={`${item.file.name}-${index}`}
                    className={`flex items-center gap-2 rounded-md p-2 text-sm ${
                      item.error ? 'bg-destructive/10' : 'bg-muted/50'
                    }`}
                  >
                    {item.error ? (
                      <AlertCircle className="h-4 w-4 shrink-0 text-destructive" />
                    ) : (
                      getFileIcon(item.file.name)
                    )}
                    <div className="min-w-0 flex-1">
                      <p className="truncate font-medium">{item.file.name}</p>
                      {item.error ? (
                        <p className="truncate text-xs text-destructive">{item.error}</p>
                      ) : (
                        <p className="text-xs text-muted-foreground">{formatFileSize(item.file.size)}</p>
                      )}
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6 shrink-0"
                      onClick={(e) => {
                        e.stopPropagation()
                        removeFile(index)
                      }}
                      disabled={uploading}
                    >
                      <X className="h-3 w-3" />
                    </Button>
                  </div>
                ))}
              </div>
            </ScrollArea>
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={handleClose} disabled={uploading}>
            Cancel
          </Button>
          <Button onClick={handleUpload} disabled={uploading || validFileCount === 0}>
            {uploading ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Uploading...
              </>
            ) : (
              <>
                <CheckCircle2 className="mr-2 h-4 w-4" />
                Upload {validFileCount > 0 && `(${validFileCount})`}
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
