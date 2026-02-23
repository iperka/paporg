import React, { useState, useEffect } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useCreateResource } from '@/mutations/use-gitops-mutations'
import { createDefaultResource, validateResourceName } from '@/types/gitops'
import type { ResourceKind } from '@/types/gitops'
import * as yaml from 'js-yaml'

interface CreateResourceDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  defaultKind?: ResourceKind
  basePath?: string
}

export function CreateResourceDialog({
  open,
  onOpenChange,
  defaultKind = 'Rule',
  basePath,
}: CreateResourceDialogProps) {
  const createResourceMut = useCreateResource()

  const isLoading = createResourceMut.isPending

  const [kind, setKind] = useState<ResourceKind>(defaultKind)
  const [name, setName] = useState('')
  const [nameError, setNameError] = useState<string | null>(null)
  const [submitError, setSubmitError] = useState<string | null>(null)

  // Reset form when dialog opens
  useEffect(() => {
    if (open) {
      setKind(defaultKind)
      setName('')
      setNameError(null)
      setSubmitError(null)
    }
  }, [open, defaultKind])

  const handleNameChange = (value: string) => {
    setName(value)
    setNameError(validateResourceName(value))
    setSubmitError(null)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    const error = validateResourceName(name)
    if (error) {
      setNameError(error)
      return
    }

    // Create the resource with default values
    const resource = createDefaultResource(kind)
    resource.metadata.name = name

    // Generate YAML
    const yamlContent = yaml.dump(resource, {
      indent: 2,
      lineWidth: -1,
      noRefs: true,
      sortKeys: false,
    })

    // Determine path
    let path: string | undefined
    if (basePath) {
      path = `${basePath}/${name}.yaml`
    }

    try {
      await createResourceMut.mutateAsync({ kind, yamlContent, path })
      onOpenChange(false)
    } catch {
      setSubmitError('Failed to create resource')
    }
  }

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={() => onOpenChange(false)}
      />

      {/* Dialog */}
      <div className="relative z-50 w-full max-w-md bg-background border rounded-lg shadow-lg p-6">
        <h2 className="text-lg font-semibold mb-4">Create New Resource</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="kind">Resource Type</Label>
            <Select
              value={kind}
              onValueChange={(value) => setKind(value as ResourceKind)}
              disabled={defaultKind === 'Settings'}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="Rule">Rule</SelectItem>
                <SelectItem value="Variable">Variable</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="name">Name</Label>
            <Input
              id="name"
              value={name}
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder="my-resource-name"
              className={nameError ? 'border-destructive' : ''}
            />
            {nameError && (
              <p className="text-sm text-destructive">{nameError}</p>
            )}
          </div>

          {basePath && (
            <div className="text-sm text-muted-foreground">
              Will be created at: <code>{basePath}/{name || '...'}.yaml</code>
            </div>
          )}

          {submitError && (
            <p className="text-sm text-destructive">{submitError}</p>
          )}

          <div className="flex justify-end gap-2 pt-4">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isLoading || !!nameError || !name}>
              {isLoading ? 'Creating...' : 'Create'}
            </Button>
          </div>
        </form>
      </div>
    </div>
  )
}
