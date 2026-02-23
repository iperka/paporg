import { TextField, SelectField, PatternField } from '@/components/form'
import { type VariableSpec } from '@/schemas/resources'
import type { FormInstance } from '@/lib/form-utils'

interface VariableFormProps {
  form: FormInstance
  isNew?: boolean
  name?: string
  onNameChange?: (name: string) => void
}

export function VariableForm({
  form,
  isNew,
  name,
  onNameChange,
}: VariableFormProps) {
  return (
    <div className="space-y-6">
      {isNew && onNameChange && (
        <TextField
          label="Variable Name"
          value={name || ''}
          onChange={onNameChange}
          description="Unique identifier for this variable (used in templates as $name)"
          required
          placeholder="my_variable"
        />
      )}

      <form.Field name="pattern" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
        <PatternField
          label="Pattern"
          value={field.state.value}
          onChange={field.handleChange}
          description="Regex pattern to extract value from document text. Use named groups like (?P<value>...)"
          error={field.state.meta.errors?.[0]}
          required
          placeholder="(?P<value>\w+)"
        />
      )} />

      <form.Field name="transform" children={(field: { state: { value: VariableSpec['transform']; meta: { errors: string[] } }; handleChange: (v: VariableSpec['transform']) => void }) => (
        <SelectField
          label="Transform"
          value={field.state.value || 'none'}
          onChange={(v: string) => field.handleChange(v === 'none' ? undefined : v as VariableSpec['transform'])}
          options={[
            { value: 'none', label: 'None (keep as-is)' },
            { value: 'slugify', label: 'Slugify (url-friendly)' },
            { value: 'uppercase', label: 'Uppercase' },
            { value: 'lowercase', label: 'Lowercase' },
            { value: 'trim', label: 'Trim whitespace' },
          ]}
          description="Optional transformation to apply to extracted value"
        />
      )} />

      <form.Field name="default" children={(field: { state: { value: string | undefined; meta: { errors: string[] } }; handleChange: (v: string | undefined) => void }) => (
        <TextField
          label="Default Value"
          value={field.state.value || ''}
          onChange={(v: string) => field.handleChange(v || undefined)}
          description="Value to use if pattern doesn't match"
          placeholder="unknown"
        />
      )} />
    </div>
  )
}
