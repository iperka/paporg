import { useState, useCallback, useRef, useId } from 'react'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { FormField } from './FormField'
import { cn } from '@/lib/utils'
import { api } from '@/api'
import { Info, Loader2, CheckCircle2 } from 'lucide-react'

export type SecretType = 'password' | 'client_id' | 'client_secret' | 'refresh_token' | 'token'

export type SecretSource = 'direct' | 'file' | 'env'

interface SecretFieldProps {
  label: string
  sourceName: string
  secretType: SecretType
  filePath?: string
  envVar?: string
  onFilePathChange: (value: string | undefined) => void
  onEnvVarChange: (value: string | undefined) => void
  description?: string
  required?: boolean
  error?: string
  disabled?: boolean
  className?: string
}

/**
 * Determines the current secret source based on which prop is set.
 *
 * Priority: env > file > direct
 * Note: Treats undefined and non-empty string as the signal for that mode.
 * Empty string is treated as "mode selected but value not yet entered".
 */
function getSecretSource(filePath?: string, envVar?: string): SecretSource {
  // Warn if both are set (conflicting config)
  if (envVar !== undefined && filePath !== undefined && envVar !== '' && filePath !== '') {
    console.warn(
      `SecretField: Both envVar (${envVar}) and filePath (${filePath}) are set. ` +
        `Using envVar as the source. Consider clearing one to avoid confusion.`
    )
  }

  // Check for defined values (including empty string for mode indicator)
  if (envVar !== undefined) return 'env'
  if (filePath !== undefined) return 'file'
  return 'direct'
}

export function SecretField({
  label,
  sourceName,
  secretType,
  filePath,
  envVar,
  onFilePathChange,
  onEnvVarChange,
  description,
  required,
  error,
  disabled,
  className,
}: SecretFieldProps) {
  const [directValue, setDirectValue] = useState('')
  const [isWriting, setIsWriting] = useState(false)
  const [writeError, setWriteError] = useState<string | null>(null)
  const [isEditing, setIsEditing] = useState(false)
  // Ref to prevent double API calls when Enter triggers both keydown and blur
  const isSavingRef = useRef(false)
  // Generate stable ID for accessibility
  const inputId = useId()

  // Compute source directly from props - no derived state needed
  const source = getSecretSource(filePath, envVar)

  // Handle source change - clear fields to switch source type
  const handleSourceChange = useCallback(
    (newSource: string) => {
      const sourceValue = newSource as SecretSource
      setWriteError(null)
      setDirectValue('')
      setIsEditing(false)

      // Clear the appropriate fields to trigger source change via props.
      // We use empty string ('') as a sentinel to indicate "mode selected but no value yet".
      // This allows getSecretSource to detect the intended mode even before user enters a value.
      if (sourceValue === 'direct') {
        // Clear both - direct mode starts fresh
        onEnvVarChange(undefined)
        onFilePathChange(undefined)
      } else if (sourceValue === 'file') {
        // Clear envVar first, then set filePath to empty string if not already set
        onEnvVarChange(undefined)
        // Always set to empty string to indicate file mode (even if filePath already exists)
        onFilePathChange(filePath || '')
      } else if (sourceValue === 'env') {
        // Clear filePath first, then set envVar to empty string if not already set
        onFilePathChange(undefined)
        // Always set to empty string to indicate env mode (even if envVar already exists)
        onEnvVarChange(envVar || '')
      }
    },
    [onFilePathChange, onEnvVarChange, filePath, envVar]
  )

  // Handle direct value save (writes to file)
  const handleDirectValueBlur = useCallback(async () => {
    // Prevent double API calls (Enter key triggers both keydown and blur)
    if (isSavingRef.current) return

    const trimmedValue = directValue.trim()

    // If empty after trim, just exit edit mode without saving
    if (!trimmedValue) {
      setDirectValue('')
      setIsEditing(false)
      return
    }

    isSavingRef.current = true
    setIsWriting(true)
    setWriteError(null)

    try {
      const response = await api.secrets.write(sourceName, secretType, trimmedValue)

      // Set the file path from the response
      onFilePathChange(response.filePath)
      // Clear envVar since we're now using file-based storage
      onEnvVarChange(undefined)

      // Clear the input and exit edit mode
      setDirectValue('')
      setIsEditing(false)
    } catch (e) {
      console.error('SecretField: API error:', e)
      setWriteError(e instanceof Error ? e.message : 'Failed to save secret')
    } finally {
      setIsWriting(false)
      isSavingRef.current = false
    }
  }, [directValue, sourceName, secretType, onFilePathChange, onEnvVarChange])

  // Display error from write operation or passed in error
  const displayError = writeError || error

  // Check if we have a stored value
  const hasStoredValue = Boolean(filePath)

  // Should we show the input field?
  const showInput = source === 'direct' && (isEditing || !hasStoredValue)

  const getInfoText = () => {
    switch (source) {
      case 'direct':
        if (filePath) {
          return `Stored in: ${filePath}`
        }
        return `Will be stored in: ~/.paporg/secrets/${sourceName}-${secretType}`
      case 'file':
        return 'Reference an existing secret file'
      case 'env':
        return 'Set this variable before running paporg'
    }
  }

  return (
    <FormField
      label={label}
      description={description}
      error={displayError}
      required={required}
      className={className}
    >
      <div className="space-y-2">
        {/* Source selector */}
        <Select value={source} onValueChange={handleSourceChange} disabled={disabled}>
          <SelectTrigger className="w-[200px]">
            <SelectValue placeholder="Select source..." />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="direct">Direct Value</SelectItem>
            <SelectItem value="file">File Path</SelectItem>
            <SelectItem value="env">Environment Variable</SelectItem>
          </SelectContent>
        </Select>

        {/* Direct value mode */}
        {source === 'direct' && (
          <div className="relative">
            {showInput ? (
              <>
                <Input
                  id={inputId}
                  type="password"
                  value={directValue}
                  onChange={(e) => setDirectValue(e.target.value)}
                  onBlur={handleDirectValueBlur}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault()
                      // Fire-and-forget with error capture
                      void handleDirectValueBlur().catch((err) =>
                        console.error('SecretField: Error saving on Enter:', err)
                      )
                    }
                  }}
                  placeholder="Enter secret value..."
                  disabled={disabled || isWriting}
                  className={cn(displayError && 'border-destructive', 'font-mono')}
                  autoFocus={isEditing}
                  aria-label={`${label} secret value`}
                />
                {isWriting && (
                  <Loader2 className="absolute right-3 top-3 h-4 w-4 animate-spin text-muted-foreground" />
                )}
              </>
            ) : (
              <div className="flex items-center gap-2">
                <div className="flex-1 flex items-center h-10 px-3 rounded-md border border-input bg-muted text-sm text-muted-foreground font-mono">
                  <CheckCircle2 className="h-4 w-4 mr-2 text-green-600 dark:text-green-400" />
                  Secret stored securely
                </div>
                <button
                  type="button"
                  onClick={() => setIsEditing(true)}
                  className="text-xs text-muted-foreground hover:text-foreground underline"
                  disabled={disabled}
                  aria-label={`Update ${label}`}
                >
                  Update
                </button>
              </div>
            )}
          </div>
        )}

        {/* File path mode */}
        {source === 'file' && (
          <Input
            type="text"
            value={filePath || ''}
            onChange={(e) => onFilePathChange(e.target.value || undefined)}
            placeholder="~/.paporg/secrets/my-password"
            disabled={disabled}
            className={cn(displayError && 'border-destructive', 'font-mono')}
          />
        )}

        {/* Environment variable mode */}
        {source === 'env' && (
          <Input
            type="text"
            value={envVar || ''}
            onChange={(e) => onEnvVarChange(e.target.value || undefined)}
            placeholder="GMAIL_APP_PASSWORD"
            disabled={disabled}
            className={cn(displayError && 'border-destructive', 'font-mono')}
          />
        )}

        {/* Info text */}
        <p className={cn(
          "flex items-center gap-1 text-xs",
          source === 'direct' && filePath ? "text-green-600 dark:text-green-400" : "text-muted-foreground"
        )}>
          {source === 'direct' && filePath ? (
            <CheckCircle2 className="h-3 w-3" />
          ) : (
            <Info className="h-3 w-3" />
          )}
          {getInfoText()}
        </p>
      </div>
    </FormField>
  )
}
