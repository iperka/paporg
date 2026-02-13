import { useState, useEffect } from 'react'
import { Input } from '@/components/ui/input'
import { FormField } from './FormField'
import { Badge } from '@/components/ui/badge'
import { validateRegexPattern } from '@/schemas/resources'
import { cn } from '@/lib/utils'
import { Check, X } from 'lucide-react'

interface PatternFieldProps {
  label: string
  value: string
  onChange: (value: string) => void
  description?: string
  error?: string
  required?: boolean
  placeholder?: string
  disabled?: boolean
  className?: string
}

export function PatternField({
  label,
  value,
  onChange,
  description,
  error,
  required,
  placeholder = 'Enter regex pattern...',
  disabled,
  className,
}: PatternFieldProps) {
  const [regexError, setRegexError] = useState<string | null>(null)

  useEffect(() => {
    if (!value) {
      setRegexError(null)
      return
    }

    const result = validateRegexPattern(value)
    setRegexError(result.valid ? null : result.error || 'Invalid regex')
  }, [value])

  const displayError = error || regexError

  return (
    <FormField
      label={label}
      description={description}
      error={displayError ?? undefined}
      required={required}
      className={className}
    >
      <div className="space-y-2">
        <div className="relative">
          <Input
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
            className={cn(
              'font-mono pr-10',
              displayError && 'border-destructive'
            )}
          />
          {value && (
            <div className="absolute right-3 top-1/2 -translate-y-1/2">
              {regexError ? (
                <X className="h-4 w-4 text-destructive" />
              ) : (
                <Check className="h-4 w-4 text-green-500" />
              )}
            </div>
          )}
        </div>
        {value && !regexError && (
          <Badge variant="outline" className="text-xs">
            Valid regex pattern
          </Badge>
        )}
      </div>
    </FormField>
  )
}
