import { Input } from '@/components/ui/input'
import { FormField } from './FormField'
import { cn } from '@/lib/utils'

interface DateFieldProps {
  label: string
  value: string
  onChange: (value: string) => void
  description?: string
  error?: string
  required?: boolean
  disabled?: boolean
  className?: string
  min?: string
  max?: string
  name?: string
}

export function DateField({
  label,
  value,
  onChange,
  description,
  error,
  required,
  disabled,
  className,
  min,
  max,
  name,
}: DateFieldProps) {
  // FormField now handles ID generation and ARIA attributes automatically
  return (
    <FormField
      label={label}
      description={description}
      error={error}
      required={required}
      className={className}
    >
      <Input
        name={name}
        type="date"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        min={min}
        max={max}
        className={cn(error && 'border-destructive')}
      />
    </FormField>
  )
}
