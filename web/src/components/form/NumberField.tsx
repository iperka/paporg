import { Input } from '@/components/ui/input'
import { FormField } from './FormField'
import { cn } from '@/lib/utils'

interface NumberFieldProps {
  label: string
  value: number
  onChange: (value: number) => void
  description?: string
  error?: string
  required?: boolean
  placeholder?: string
  disabled?: boolean
  className?: string
  min?: number
  max?: number
  step?: number
}

export function NumberField({
  label,
  value,
  onChange,
  description,
  error,
  required,
  placeholder,
  disabled,
  className,
  min,
  max,
  step = 1,
}: NumberFieldProps) {
  return (
    <FormField
      label={label}
      description={description}
      error={error}
      required={required}
      className={className}
    >
      <Input
        type="number"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        placeholder={placeholder}
        disabled={disabled}
        min={min}
        max={max}
        step={step}
        className={cn(error && 'border-destructive')}
      />
    </FormField>
  )
}
