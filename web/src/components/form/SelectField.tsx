import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { FormField } from './FormField'
import { cn } from '@/lib/utils'

interface SelectOption {
  value: string
  label: string
}

interface SelectFieldProps {
  label: string
  value: string | undefined
  onChange: (value: string) => void
  options: SelectOption[]
  description?: string
  error?: string
  required?: boolean
  placeholder?: string
  disabled?: boolean
  className?: string
}

export function SelectField({
  label,
  value,
  onChange,
  options,
  description,
  error,
  required,
  placeholder = 'Select...',
  disabled,
  className,
}: SelectFieldProps) {
  return (
    <FormField
      label={label}
      description={description}
      error={error}
      required={required}
      className={className}
    >
      <Select value={value} onValueChange={onChange} disabled={disabled}>
        <SelectTrigger className={cn(error && 'border-destructive')}>
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </FormField>
  )
}
