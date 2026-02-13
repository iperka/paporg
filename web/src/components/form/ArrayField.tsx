import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { FormField } from './FormField'
import { Plus, X } from 'lucide-react'
import { cn } from '@/lib/utils'

interface ArrayFieldProps {
  label: string
  values: string[]
  onChange: (values: string[]) => void
  description?: string
  error?: string
  required?: boolean
  placeholder?: string
  disabled?: boolean
  className?: string
  addLabel?: string
  mono?: boolean
  minItems?: number
}

export function ArrayField({
  label,
  values,
  onChange,
  description,
  error,
  required,
  placeholder = 'Enter value...',
  disabled,
  className,
  addLabel = 'Add Item',
  mono,
  minItems = 0,
}: ArrayFieldProps) {
  const addValue = () => {
    onChange([...values, ''])
  }

  const removeValue = (index: number) => {
    const newValues = values.filter((_, i) => i !== index)
    onChange(newValues.length > 0 ? newValues : minItems > 0 ? [''] : [])
  }

  const updateValue = (index: number, value: string) => {
    const newValues = [...values]
    newValues[index] = value
    onChange(newValues)
  }

  // Ensure minimum items
  const displayValues = values.length > 0 ? values : minItems > 0 ? [''] : []

  return (
    <FormField
      label={label}
      description={description}
      error={error}
      required={required}
      className={className}
    >
      <div className="space-y-2">
        {displayValues.map((value, index) => (
          <div key={index} className="flex items-center gap-2">
            <Input
              value={value}
              onChange={(e) => updateValue(index, e.target.value)}
              placeholder={placeholder}
              disabled={disabled}
              className={cn(mono && 'font-mono')}
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => removeValue(index)}
              disabled={disabled || displayValues.length <= minItems}
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
        ))}
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={addValue}
          disabled={disabled}
        >
          <Plus className="h-4 w-4 mr-2" />
          {addLabel}
        </Button>
      </div>
    </FormField>
  )
}
