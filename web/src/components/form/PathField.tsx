import { useState } from 'react'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { FormField } from './FormField'
import { cn } from '@/lib/utils'
import { FolderOpen, FileText, Loader2 } from 'lucide-react'
import { api } from '@/api'

interface PathFieldProps {
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
  mode?: 'folder' | 'file'
}

export function PathField({
  label,
  value,
  onChange,
  description,
  error,
  required,
  placeholder,
  disabled,
  className,
  mono = true,
  mode = 'folder',
}: PathFieldProps) {
  const [picking, setPicking] = useState(false)

  const handleBrowse = async () => {
    setPicking(true)
    try {
      const selected =
        mode === 'folder' ? await api.files.pickFolder() : await api.files.pickFile()
      if (selected !== null) {
        onChange(selected)
      }
    } catch (err) {
      console.error('Failed to open picker:', err)
    } finally {
      setPicking(false)
    }
  }

  const Icon = mode === 'folder' ? FolderOpen : FileText

  return (
    <FormField
      label={label}
      description={description}
      error={error}
      required={required}
      className={className}
    >
      <div className="flex gap-2">
        <Input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          disabled={disabled}
          className={cn('flex-1', error && 'border-destructive', mono && 'font-mono')}
        />
        <Button
          type="button"
          variant="outline"
          disabled={disabled || picking}
          onClick={handleBrowse}
        >
          {picking ? <Loader2 className="h-4 w-4 animate-spin" /> : <Icon className="h-4 w-4" />}
        </Button>
      </div>
    </FormField>
  )
}
