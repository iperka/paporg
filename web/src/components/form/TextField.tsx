import { Input } from '@/components/ui/input'
import { FormField } from './FormField'
import { cn } from '@/lib/utils'

interface TextFieldProps {
  label: string
  value: string
  onChange: (value: string) => void
  description?: string
  error?: string
  required?: boolean
  placeholder?: string
  disabled?: boolean
  className?: string
  mono?: boolean
  type?: 'text' | 'password' | 'email'
}

export function TextField({
  label,
  value,
  onChange,
  description,
  error,
  required,
  placeholder,
  disabled,
  className,
  mono,
  type = 'text',
}: TextFieldProps) {
  return (
    <FormField
      label={label}
      description={description}
      error={error}
      required={required}
      className={className}
    >
      <Input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        className={cn(error && 'border-destructive', mono && 'font-mono')}
      />
    </FormField>
  )
}
