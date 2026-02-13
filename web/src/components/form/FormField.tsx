import React from 'react'
import { Label } from '@/components/ui/label'
import { cn } from '@/lib/utils'

interface FormFieldProps {
  label: string
  description?: string
  error?: string
  required?: boolean
  className?: string
  children: React.ReactElement
  /** Override the auto-generated ID for the input element */
  htmlFor?: string
  /** Override the auto-generated ID for the error message element */
  errorId?: string
  /** Override the auto-generated ID for the description element */
  descriptionId?: string
}

export function FormField({
  label,
  description,
  error,
  required,
  className,
  children,
  htmlFor: htmlForProp,
  errorId: errorIdProp,
  descriptionId: descriptionIdProp,
}: FormFieldProps) {
  // Generate unique IDs for accessibility
  const generatedId = React.useId()
  const inputId = htmlForProp ?? `${generatedId}-input`
  const errorIdFinal = errorIdProp ?? `${generatedId}-error`
  const descriptionIdFinal = descriptionIdProp ?? `${generatedId}-description`

  // Build aria-describedby string - only include IDs for elements that are actually rendered.
  // Description is only rendered when (description && !error), so only include its ID in that case.
  // Error is rendered when error is truthy.
  const ariaDescribedBy = [
    description && !error ? descriptionIdFinal : null,
    error ? errorIdFinal : null,
  ]
    .filter(Boolean)
    .join(' ') || undefined

  // Clone child element to inject accessibility attributes.
  // React.cloneElement already shallow-merges existing props with new ones,
  // so we don't need to spread children.props explicitly.
  const enhancedChild = React.cloneElement(children, {
    id: inputId,
    'aria-describedby': ariaDescribedBy,
    'aria-invalid': !!error || undefined,
    'aria-required': !!required || undefined,
  })

  return (
    <div className={cn('space-y-2', className)}>
      <Label htmlFor={inputId} className={cn(error && 'text-destructive')}>
        {label}
        {required && <span className="text-destructive ml-1">*</span>}
      </Label>
      {enhancedChild}
      {description && !error && (
        <p id={descriptionIdFinal} className="text-xs text-muted-foreground">
          {description}
        </p>
      )}
      {error && (
        <p id={errorIdFinal} className="text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
    </div>
  )
}
