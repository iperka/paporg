import { useEffect, useState, useMemo } from 'react'
import { Settings } from 'lucide-react'
import { useGitOps } from '@/contexts/GitOpsContext'
import { useToast } from '@/components/ui/use-toast'
import { ResourcePanel } from '@/components/resource/ResourcePanel'
import { SettingsForm } from '@/components/forms/SettingsForm'
import { YamlEditor } from '@/components/ui/yaml-editor'
import {
  type SettingsSpec,
  type SettingsResource,
  createDefaultSettingsSpec,
  settingsSpecSchema,
} from '@/schemas/resources'
import yaml from 'js-yaml'

export function SettingsPage() {
  const { fileTree, selectFile, selectedResource, updateResource, isLoading } = useGitOps()
  const { toast } = useToast()

  const [formData, setFormData] = useState<SettingsSpec>(createDefaultSettingsSpec())
  const [yamlContent, setYamlContent] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [initialData, setInitialData] = useState<SettingsSpec | null>(null)

  // Find and select the settings resource on mount
  useEffect(() => {
    const findSettingsPath = (node: typeof fileTree): string | null => {
      if (!node) return null
      if (node.resource?.kind === 'Settings') {
        return node.path
      }
      for (const child of node.children) {
        const found = findSettingsPath(child)
        if (found) return found
      }
      return null
    }

    const path = findSettingsPath(fileTree)
    if (path) {
      selectFile(path)
    }
  }, [fileTree, selectFile])

  // Parse the loaded resource
  useEffect(() => {
    if (!selectedResource?.yaml) return

    try {
      const parsed = yaml.load(selectedResource.yaml) as SettingsResource
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
        setFormData(mergedSpec)
        setYamlContent(selectedResource.yaml)
        setInitialData(JSON.parse(JSON.stringify(mergedSpec)))
        setError(null)
      }
    } catch {
      setError('Failed to parse settings YAML')
    }
  }, [selectedResource])

  // Check for unsaved changes
  const hasChanges = useMemo(() => {
    if (!initialData) return false
    return JSON.stringify(formData) !== JSON.stringify(initialData)
  }, [formData, initialData])

  // Sync form changes to YAML
  useEffect(() => {
    try {
      const resource: SettingsResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Settings',
        metadata: { name: 'settings', labels: {}, annotations: {} },
        spec: formData,
      }
      setYamlContent(yaml.dump(resource, { lineWidth: -1 }))
    } catch {
      // Ignore serialization errors while editing
    }
  }, [formData])

  // Handle YAML changes
  const handleYamlChange = (newYaml: string) => {
    setYamlContent(newYaml)
    try {
      const parsed = yaml.load(newYaml) as SettingsResource
      if (parsed?.spec) {
        const validated = settingsSpecSchema.safeParse(parsed.spec)
        if (validated.success) {
          setFormData(validated.data)
          setError(null)
        }
      }
    } catch {
      // Allow invalid YAML during editing
    }
  }

  // Save handler
  const handleSave = async () => {
    // Validate
    const validation = settingsSpecSchema.safeParse(formData)
    if (!validation.success) {
      const firstError = validation.error.errors[0]
      setError(`Validation error: ${firstError.path.join('.')}: ${firstError.message}`)
      return
    }

    setIsSaving(true)
    setError(null)

    try {
      const resource: SettingsResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Settings',
        metadata: { name: 'settings', labels: {}, annotations: {} },
        spec: formData,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })

      const success = await updateResource('Settings', 'settings', newYaml)

      if (success) {
        setInitialData(JSON.parse(JSON.stringify(formData)))
        toast({
          title: 'Settings saved',
          description: 'Your settings have been saved successfully.',
        })
      } else {
        setError('Failed to save settings')
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save settings')
    } finally {
      setIsSaving(false)
    }
  }

  const settingsNotFound = !isLoading && fileTree && !selectedResource

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
          error={error}
          hasChanges={hasChanges}
          onSave={handleSave}
          formContent={
            <SettingsForm
              value={formData}
              onChange={setFormData}
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
