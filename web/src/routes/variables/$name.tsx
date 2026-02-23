import { useParams, useNavigate, useSearch } from '@tanstack/react-router'
import { useEffect, useState, useMemo, useCallback } from 'react'
import { Variable, ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { YamlEditor } from '@/components/ui/yaml-editor'
import { useResource } from '@/queries/use-resource'
import { useUpdateResource, useCreateResource, useDeleteResource } from '@/mutations/use-gitops-mutations'
import { useToast } from '@/components/ui/use-toast'
import { ResourcePanel } from '@/components/resource/ResourcePanel'
import { VariableForm } from '@/components/forms/VariableForm'
import { useAutoSave } from '@/hooks/useAutoSave'
import { useForm, useStore } from '@tanstack/react-form'
import {
  type VariableSpec,
  type VariableResource,
  createDefaultVariableSpec,
  variableSpecSchema,
} from '@/schemas/resources'
import { zodFormValidator } from '@/lib/form-utils'
import yaml from 'js-yaml'

export function VariableEditPage() {
  const { name: urlName } = useParams({ from: '/variables/$name' })
  const { folder } = useSearch({ from: '/variables/$name' })
  const navigate = useNavigate()
  const { toast } = useToast()

  const isNew = urlName === 'new'

  const { data: resourceData, isLoading: resourceLoading } = useResource('Variable', urlName, !isNew)
  const updateResourceMut = useUpdateResource()
  const createResourceMut = useCreateResource()
  const deleteResourceMut = useDeleteResource()

  const isLoading = resourceLoading || updateResourceMut.isPending || createResourceMut.isPending || deleteResourceMut.isPending

  const [resourceName, setResourceName] = useState('')
  const [initialName, setInitialName] = useState('')
  const [yamlContent, setYamlContent] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // TanStack Form - use function validator for Zod schemas with .default() fields
  const form = useForm({
    defaultValues: createDefaultVariableSpec(),
    validators: {
      onChange: zodFormValidator(variableSpecSchema),
    },
    onSubmit: async () => {
      // Submit handled via handleSave
    },
  })

  // Subscribe to form state via the form's store
  const formValues = useStore(form.store, (state) => state.values)
  const isDirty = useStore(form.store, (state) => state.isDirty)
  const canSubmit = useStore(form.store, (state) => state.canSubmit)

  // Parse the loaded resource
  useEffect(() => {
    if (isNew) {
      form.reset(createDefaultVariableSpec())
      setResourceName('')
      setInitialName('')
      return
    }

    if (!resourceData?.yaml) return

    try {
      const parsed = yaml.load(resourceData.yaml) as VariableResource
      if (parsed?.spec && parsed.kind === 'Variable') {
        form.reset(parsed.spec)
        setResourceName(parsed.metadata.name)
        setInitialName(parsed.metadata.name)
        setYamlContent(resourceData.yaml)
        setError(null)
      }
    } catch {
      setError('Failed to parse variable YAML')
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resourceData, isNew])

  // Check for unsaved changes
  const hasChanges = useMemo(() => {
    if (isNew) {
      return resourceName.trim() !== '' || isDirty
    }
    return isDirty || resourceName !== initialName
  }, [resourceName, initialName, isNew, isDirty])

  // Check if form is valid for auto-save
  const isValidForSave = useMemo(() => {
    if (!resourceName.trim()) return false
    if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(resourceName)) return false
    return canSubmit
  }, [resourceName, canSubmit])

  // Sync form changes to YAML
  useEffect(() => {
    if (!resourceName && !isNew) return

    try {
      const resource: VariableResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Variable',
        metadata: { name: resourceName || 'new_variable', labels: {}, annotations: {} },
        spec: formValues,
      }
      setYamlContent(yaml.dump(resource, { lineWidth: -1 }))
    } catch {
      // Ignore serialization errors while editing
    }
  }, [formValues, resourceName, isNew])

  // Auto-save handler
  const handleAutoSave = useCallback(async () => {
    if (isNew || !isValidForSave) return

    // Snapshot values before async save to avoid overwriting concurrent edits
    const savedValues = structuredClone(formValues)
    const savedName = resourceName

    const resource: VariableResource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Variable',
      metadata: { name: savedName, labels: {}, annotations: {} },
      spec: savedValues,
    }
    const newYaml = yaml.dump(resource, { lineWidth: -1 })

    try {
      await updateResourceMut.mutateAsync({ kind: 'Variable', name: urlName, yamlContent: newYaml })
      // Reset form baseline after save (updates default values to current)
      form.reset(savedValues)
      setInitialName(savedName)
    } catch {
      throw new Error('Failed to save')
    }
  }, [isNew, isValidForSave, resourceName, formValues, updateResourceMut, urlName, form])

  // Auto-save hook
  const { status: autoSaveStatus, lastSaved, error: autoSaveError } = useAutoSave({
    data: { resourceName, formValues },
    onSave: handleAutoSave,
    delay: 1500,
    enabled: !isNew && isValidForSave,
    hasChanges,
    isNew,
  })

  // Handle YAML changes
  const handleYamlChange = (newYaml: string) => {
    setYamlContent(newYaml)
    try {
      const parsed = yaml.load(newYaml) as VariableResource
      if (parsed?.spec && parsed.kind === 'Variable') {
        const validated = variableSpecSchema.safeParse(parsed.spec)
        if (validated.success) {
          for (const [key, val] of Object.entries(validated.data)) {
            form.setFieldValue(key as keyof VariableSpec, val as never)
          }
          if (parsed.metadata?.name) {
            setResourceName(parsed.metadata.name)
          }
          setError(null)
        }
      }
    } catch {
      // Allow invalid YAML during editing
    }
  }

  // Save handler
  const handleSave = async () => {
    if (!resourceName.trim()) {
      setError('Variable name is required')
      return
    }

    if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(resourceName)) {
      setError('Variable name must start with a letter or underscore, and contain only letters, numbers, underscores, and hyphens')
      return
    }

    if (!canSubmit) {
      setError('Form has validation errors')
      return
    }

    setIsSaving(true)
    setError(null)

    try {
      const resource: VariableResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Variable',
        metadata: { name: resourceName, labels: {}, annotations: {} },
        spec: formValues,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })

      if (isNew) {
        const targetPath = folder ? `${folder}/${resourceName}.yaml` : undefined
        await createResourceMut.mutateAsync({ kind: 'Variable', yamlContent: newYaml, path: targetPath })
      } else {
        await updateResourceMut.mutateAsync({ kind: 'Variable', name: urlName, yamlContent: newYaml })
      }

      form.reset(formValues)
      setInitialName(resourceName)
      toast({
        title: isNew ? 'Variable created' : 'Variable saved',
        description: isNew
          ? `Variable "${resourceName}" has been created.`
          : 'Your changes have been saved.',
      })

      if (isNew) {
        navigate({ to: '/variables/$name', params: { name: resourceName } })
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save variable')
    } finally {
      setIsSaving(false)
    }
  }

  // Delete handler
  const handleDelete = async () => {
    if (isNew || isSaving) return

    const confirmed = window.confirm(`Are you sure you want to delete the variable "${urlName}"?`)
    if (!confirmed) return

    setIsSaving(true)
    try {
      await deleteResourceMut.mutateAsync({ kind: 'Variable', name: urlName })
      toast({
        title: 'Variable deleted',
        description: `Variable "${urlName}" has been deleted.`,
      })
      navigate({ to: '/variables' })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete variable')
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate({ to: '/variables' })}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex items-center gap-3">
          <Variable className="h-8 w-8" />
          <div>
            <h1 className="text-3xl font-bold tracking-tight">
              {isNew ? 'New Variable' : urlName}
            </h1>
            <p className="text-muted-foreground">
              {isNew ? 'Create a new variable for text extraction' : 'Edit variable configuration'}
            </p>
          </div>
        </div>
      </div>

      <ResourcePanel
        title={isNew ? 'New Variable' : resourceName}
        description="Extract values from documents using regex patterns"
        isLoading={!isNew && isLoading}
        isSaving={isSaving}
        error={error || autoSaveError}
        hasChanges={hasChanges}
        onSave={handleSave}
        onDelete={isNew ? undefined : handleDelete}
        isNew={isNew}
        autoSaveStatus={isNew ? undefined : autoSaveStatus}
        lastSaved={lastSaved}
        formContent={
          <VariableForm
            form={form}
            isNew={isNew}
            name={resourceName}
            onNameChange={setResourceName}
          />
        }
        yamlContent={
          <YamlEditor
            value={yamlContent}
            onChange={handleYamlChange}
            height="400px"
            placeholder="# Variable YAML"
          />
        }
      />
    </div>
  )
}
