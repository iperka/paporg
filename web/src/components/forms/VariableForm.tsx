import { TextField, SelectField, PatternField } from '@/components/form'
import { type VariableSpec } from '@/schemas/resources'

interface VariableFormProps {
  value: VariableSpec
  onChange: (value: VariableSpec) => void
  errors?: Record<string, string>
  isNew?: boolean
  name?: string
  onNameChange?: (name: string) => void
}

export function VariableForm({
  value,
  onChange,
  errors = {},
  isNew,
  name,
  onNameChange,
}: VariableFormProps) {
  const updateField = <K extends keyof VariableSpec>(
    field: K,
    fieldValue: VariableSpec[K]
  ) => {
    onChange({ ...value, [field]: fieldValue })
  }

  return (
    <div className="space-y-6">
      {isNew && onNameChange && (
        <TextField
          label="Variable Name"
          value={name || ''}
          onChange={onNameChange}
          description="Unique identifier for this variable (used in templates as $name)"
          error={errors['name']}
          required
          placeholder="my_variable"
        />
      )}

      <PatternField
        label="Pattern"
        value={value.pattern}
        onChange={(v) => updateField('pattern', v)}
        description="Regex pattern to extract value from document text. Use named groups like (?P<value>...)"
        error={errors['pattern']}
        required
        placeholder="(?P<value>\w+)"
      />

      <SelectField
        label="Transform"
        value={value.transform || 'none'}
        onChange={(v) => updateField('transform', v === 'none' ? undefined : v as VariableSpec['transform'])}
        options={[
          { value: 'none', label: 'None (keep as-is)' },
          { value: 'slugify', label: 'Slugify (url-friendly)' },
          { value: 'uppercase', label: 'Uppercase' },
          { value: 'lowercase', label: 'Lowercase' },
          { value: 'trim', label: 'Trim whitespace' },
        ]}
        description="Optional transformation to apply to extracted value"
      />

      <TextField
        label="Default Value"
        value={value.default || ''}
        onChange={(v) => updateField('default', v || undefined)}
        description="Value to use if pattern doesn't match"
        placeholder="unknown"
      />
    </div>
  )
}
