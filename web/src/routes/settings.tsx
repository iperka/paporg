import { useEffect, useState, useMemo, useCallback } from 'react'
import { Settings } from 'lucide-react'
import { useFileTree } from '@/queries/use-file-tree'
import { useResource } from '@/queries/use-resource'
import { useUpdateResource } from '@/mutations/use-gitops-mutations'
import { useToast } from '@/components/ui/use-toast'
import { ResourcePanel } from '@/components/resource/ResourcePanel'
import { SettingsForm } from '@/components/forms/SettingsForm'
import { YamlEditor } from '@/components/ui/yaml-editor'
import { useAutoSave } from '@/hooks/useAutoSave'
import { useForm, useStore } from '@tanstack/react-form'
import {
  type SettingsSpec,
  type SettingsResource,
  createDefaultSettingsSpec,
  settingsSpecSchema,
} from '@/schemas/resources'
import { zodFormValidator } from '@/lib/form-utils'
import type { FileTreeNode } from '@/types/gitops'
import yaml from 'js-yaml'

export function SettingsPage() {
  const { data: fileTree, isLoading: isTreeLoading } = useFileTree()
  const updateResourceMut = useUpdateResource()
  const { toast } = useToast()

  const [yamlContent, setYamlContent] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // TanStack Form - use function validator for Zod schemas with .default() fields
  const form = useForm({
    defaultValues: createDefaultSettingsSpec(),
    validators: {
      onChange: zodFormValidator(settingsSpecSchema),
    },
    onSubmit: async () => {
      // Submit handled via handleSave
    },
  })

  // Subscribe to form state via the form's store
  const formValues = useStore(form.store, (state) => state.values)
  const isDirty = useStore(form.store, (state) => state.isDirty)
  const canSubmit = useStore(form.store, (state) => state.canSubmit)

  // Find the Settings resource name from the file tree
  const settingsName = useMemo(() => {
    const findSettingsName = (node: FileTreeNode): string | null => {
      if (!node) return null
      if (node.resource?.kind === 'Settings') {
        return node.resource.name
      }
      for (const child of node.children) {
        const found = findSettingsName(child)
        if (found) return found
      }
      return null
    }

    return fileTree ? findSettingsName(fileTree) : null
  }, [fileTree])

  // Load the settings resource directly by kind+name
  const { data: settingsResource, isLoading: isResourceLoading } = useResource('Settings', settingsName ?? '', !!settingsName)
  const isLoading = isTreeLoading || isResourceLoading

  // Parse the loaded resource
  useEffect(() => {
    if (!settingsResource?.yaml) return

    try {
      const parsed = yaml.load(settingsResource.yaml) as SettingsResource
      if (parsed?.spec) {
        // Merge with defaults to ensure all nested objects exist
        const defaults = createDefaultSettingsSpec()
        const mergedSpec: SettingsSpec = {
          ...defaults,
          ...parsed.spec,
          ocr: { ...defaults.ocr, ...parsed.spec.ocr },
          defaults: {
            ...defaults.defaults,
            output: { ...defaults.defaults.output, ...parsed.spec.defaults?.output },
          },
          git: {
            ...defaults.git,
            ...parsed.spec.git,
            auth: { ...defaults.git.auth, ...parsed.spec.git?.auth },
          },
        }
        form.reset(mergedSpec)
        setYamlContent(settingsResource.yaml)
        setError(null)
      }
    } catch {
      setError('Failed to parse settings YAML')
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsResource])

  // Check for unsaved changes
  const hasChanges = isDirty

  // Check if form is valid for auto-save
  const isValidForSave = canSubmit

  const effectiveSettingsName = settingsName ?? 'settings'

  // Sync form changes to YAML
  useEffect(() => {
    try {
      const resource: SettingsResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Settings',
        metadata: { name: effectiveSettingsName, labels: {}, annotations: {} },
        spec: formValues,
      }
      setYamlContent(yaml.dump(resource, { lineWidth: -1 }))
    } catch {
      // Ignore serialization errors while editing
    }
  }, [formValues, effectiveSettingsName])

  // Auto-save handler
  const handleAutoSave = useCallback(async () => {
    if (!isValidForSave) return

    // Snapshot values before async save to avoid overwriting concurrent edits
    const savedValues = structuredClone(formValues)

    const resource: SettingsResource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Settings',
      metadata: { name: effectiveSettingsName, labels: {}, annotations: {} },
      spec: savedValues,
    }
    const newYaml = yaml.dump(resource, { lineWidth: -1 })

    try {
      await updateResourceMut.mutateAsync({ kind: 'Settings', name: effectiveSettingsName, yamlContent: newYaml })
      // Reset form baseline after save (updates default values to current)
      form.reset(savedValues)
    } catch {
      throw new Error('Failed to save')
    }
  }, [isValidForSave, formValues, updateResourceMut, effectiveSettingsName, form])

  // Auto-save hook
  const { status: autoSaveStatus, lastSaved, error: autoSaveError } = useAutoSave({
    data: { formValues },
    onSave: handleAutoSave,
    delay: 1500,
    enabled: isValidForSave,
    hasChanges,
  })

  // Handle YAML changes
  const handleYamlChange = (newYaml: string) => {
    setYamlContent(newYaml)
    try {
      const parsed = yaml.load(newYaml) as SettingsResource
      if (parsed?.spec) {
        const validated = settingsSpecSchema.safeParse(parsed.spec)
        if (validated.success) {
          for (const [key, val] of Object.entries(validated.data)) {
            form.setFieldValue(key as keyof SettingsSpec, val as never)
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
    if (!canSubmit) {
      setError('Form has validation errors')
      return
    }

    setIsSaving(true)
    setError(null)

    try {
      const resource: SettingsResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Settings',
        metadata: { name: effectiveSettingsName, labels: {}, annotations: {} },
        spec: formValues,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })

      await updateResourceMut.mutateAsync({ kind: 'Settings', name: effectiveSettingsName, yamlContent: newYaml })

      form.reset(formValues)
      toast({
        title: 'Settings saved',
        description: 'Your settings have been saved successfully.',
      })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save settings')
    } finally {
      setIsSaving(false)
    }
  }

  const settingsNotFound = !isLoading && fileTree && !settingsResource

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <Settings className="h-8 w-8" />
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Settings</h1>
          <p className="text-muted-foreground">
            Configure paporg global settings
          </p>
        </div>
      </div>

      {settingsNotFound ? (
        <div className="text-center py-12">
          <Settings className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
          <h3 className="text-lg font-semibold mb-2">No Settings Found</h3>
          <p className="text-muted-foreground mb-4">
            Create a settings.yaml file to configure paporg.
          </p>
        </div>
      ) : (
        <ResourcePanel
          title="Global Settings"
          description="Configure input/output directories, OCR, and Git sync"
          isLoading={isLoading}
          isSaving={isSaving}
          error={error || autoSaveError}
          hasChanges={hasChanges}
          onSave={handleSave}
          autoSaveStatus={autoSaveStatus}
          lastSaved={lastSaved}
          formContent={
            <SettingsForm
              form={form}
            />
          }
          yamlContent={
            <YamlEditor
              value={yamlContent}
              onChange={handleYamlChange}
              height="500px"
              placeholder="# Settings YAML"
            />
          }
        />
      )}
    </div>
  )
}
