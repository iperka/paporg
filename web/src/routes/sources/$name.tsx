import { useParams, useNavigate, useSearch } from '@tanstack/react-router'
import { useEffect, useState, useMemo, useCallback } from 'react'
import { FolderInput, ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { YamlEditor } from '@/components/ui/yaml-editor'
import { useGitOps } from '@/contexts/GitOpsContext'
import { useToast } from '@/components/ui/use-toast'
import { ResourcePanel } from '@/components/resource/ResourcePanel'
import { ImportSourceForm } from '@/components/forms/ImportSourceForm'
import { useAutoSave } from '@/hooks/useAutoSave'
import {
  type ImportSourceSpec,
  type ImportSourceResource,
  createDefaultImportSourceSpec,
  importSourceSpecSchema,
} from '@/schemas/resources'
import yaml from 'js-yaml'

export function SourceEditPage() {
  const { name: urlName } = useParams({ from: '/sources/$name' })
  const { folder } = useSearch({ from: '/sources/$name' })
  const navigate = useNavigate()
  const { fileTree, selectFile, selectedResource, updateResource, createResource, deleteResource, isLoading } = useGitOps()
  const { toast } = useToast()

  const isNew = urlName === 'new'

  const [resourceName, setResourceName] = useState('')
  const [formData, setFormData] = useState<ImportSourceSpec>(createDefaultImportSourceSpec())
  const [yamlContent, setYamlContent] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [initialData, setInitialData] = useState<{ name: string; spec: ImportSourceSpec } | null>(null)

  // Find and select the import source resource
  useEffect(() => {
    if (isNew) {
      setFormData(createDefaultImportSourceSpec())
      setResourceName('')
      setInitialData(null)
      return
    }

    const findResourcePath = (node: typeof fileTree): string | null => {
      if (!node) return null
      if (node.resource?.kind === 'ImportSource' && node.resource.name === urlName) {
        return node.path
      }
      for (const child of node.children) {
        const found = findResourcePath(child)
        if (found) return found
      }
      return null
    }

    const path = findResourcePath(fileTree)
    if (path) {
      selectFile(path)
    }
  }, [urlName, fileTree, selectFile, isNew])

  // Parse the loaded resource
  useEffect(() => {
    if (isNew || !selectedResource?.yaml) return

    try {
      const parsed = yaml.load(selectedResource.yaml) as ImportSourceResource
      if (parsed?.spec && parsed.kind === 'ImportSource') {
        setFormData(parsed.spec)
        setResourceName(parsed.metadata.name)
        setYamlContent(selectedResource.yaml)
        setInitialData({
          name: parsed.metadata.name,
          spec: JSON.parse(JSON.stringify(parsed.spec)),
        })
        setError(null)
      }
    } catch {
      setError('Failed to parse import source YAML')
    }
  }, [selectedResource, isNew])

  // Check for unsaved changes
  const hasChanges = useMemo(() => {
    if (isNew) {
      return resourceName.trim() !== '' || formData.local?.path !== ''
    }
    if (!initialData) return false

    return (
      resourceName !== initialData.name ||
      JSON.stringify(formData) !== JSON.stringify(initialData.spec)
    )
  }, [formData, resourceName, initialData, isNew])

  // Check if form is valid for auto-save
  const isValidForSave = useMemo(() => {
    if (!resourceName.trim()) return false
    if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(resourceName)) return false
    const validation = importSourceSpecSchema.safeParse(formData)
    return validation.success
  }, [resourceName, formData])

  // Sync form changes to YAML
  useEffect(() => {
    if (!resourceName && !isNew) return

    try {
      const resource: ImportSourceResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'ImportSource',
        metadata: { name: resourceName || 'new_source', labels: {}, annotations: {} },
        spec: formData,
      }
      setYamlContent(yaml.dump(resource, { lineWidth: -1 }))
    } catch {
      // Ignore serialization errors while editing
    }
  }, [formData, resourceName, isNew])

  // Auto-save handler
  const handleAutoSave = useCallback(async () => {
    if (isNew || !isValidForSave) return

    const resource: ImportSourceResource = {
      apiVersion: 'paporg.io/v1',
      kind: 'ImportSource',
      metadata: { name: resourceName, labels: {}, annotations: {} },
      spec: formData,
    }
    const newYaml = yaml.dump(resource, { lineWidth: -1 })

    const success = await updateResource('ImportSource', urlName, newYaml)
    if (success) {
      setInitialData({
        name: resourceName,
        spec: JSON.parse(JSON.stringify(formData)),
      })
    } else {
      throw new Error('Failed to save')
    }
  }, [isNew, isValidForSave, resourceName, formData, updateResource, urlName])

  // Auto-save hook
  const { status: autoSaveStatus, lastSaved, error: autoSaveError } = useAutoSave({
    data: { resourceName, formData },
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
      const parsed = yaml.load(newYaml) as ImportSourceResource
      if (parsed?.spec && parsed.kind === 'ImportSource') {
        const validated = importSourceSpecSchema.safeParse(parsed.spec)
        if (validated.success) {
          setFormData(validated.data)
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
    // Validate name
    if (!resourceName.trim()) {
      setError('Source name is required')
      return
    }

    if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(resourceName)) {
      setError('Source name must start with a letter or underscore, and contain only letters, numbers, underscores, and hyphens')
      return
    }

    // Validate spec
    const validation = importSourceSpecSchema.safeParse(formData)
    if (!validation.success) {
      const firstError = validation.error.errors[0]
      setError(`Validation error: ${firstError.path.join('.')}: ${firstError.message}`)
      return
    }

    setIsSaving(true)
    setError(null)

    try {
      const resource: ImportSourceResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'ImportSource',
        metadata: { name: resourceName, labels: {}, annotations: {} },
        spec: formData,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })

      let success: boolean
      if (isNew) {
        // If folder is provided, create the resource in that folder
        const targetPath = folder ? `${folder}/${resourceName}.yaml` : undefined
        success = await createResource('ImportSource', newYaml, targetPath)
      } else {
        success = await updateResource('ImportSource', urlName, newYaml)
      }

      if (success) {
        setInitialData({
          name: resourceName,
          spec: JSON.parse(JSON.stringify(formData)),
        })
        toast({
          title: isNew ? 'Source created' : 'Source saved',
          description: isNew
            ? `Import source "${resourceName}" has been created.`
            : 'Your changes have been saved.',
        })

        if (isNew) {
          navigate({ to: '/sources/$name', params: { name: resourceName } })
        }
      } else {
        setError(isNew ? 'Failed to create source' : 'Failed to save source')
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save source')
    } finally {
      setIsSaving(false)
    }
  }

  // Delete handler
  const handleDelete = async () => {
    if (isNew) return

    const confirmed = window.confirm(`Are you sure you want to delete the import source "${urlName}"?`)
    if (!confirmed) return

    setIsSaving(true)
    try {
      const success = await deleteResource('ImportSource', urlName)
      if (success) {
        toast({
          title: 'Source deleted',
          description: `Import source "${urlName}" has been deleted.`,
        })
        navigate({ to: '/sources' })
      } else {
        setError('Failed to delete source')
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete source')
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate({ to: '/sources' })}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex items-center gap-3">
          <FolderInput className="h-8 w-8" />
          <div>
            <h1 className="text-3xl font-bold tracking-tight">
              {isNew ? 'New Import Source' : urlName}
            </h1>
            <p className="text-muted-foreground">
              {isNew ? 'Create a new import source' : 'Edit import source configuration'}
            </p>
          </div>
        </div>
      </div>

      <ResourcePanel
        title={isNew ? 'New Import Source' : resourceName}
        description="Configure where documents are imported from"
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
          <ImportSourceForm
            value={formData}
            onChange={setFormData}
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
            placeholder="# Import Source YAML"
          />
        }
      />
    </div>
  )
}
