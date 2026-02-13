import { useParams, useNavigate, useSearch } from '@tanstack/react-router'
import { useEffect, useState, useMemo, useCallback } from 'react'
import { FileText, ArrowLeft, Share2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { YamlEditor } from '@/components/ui/yaml-editor'
import { useGitOps } from '@/contexts/GitOpsContext'
import { useToast } from '@/components/ui/use-toast'
import { ResourcePanel } from '@/components/resource/ResourcePanel'
import { RuleForm } from '@/components/forms/RuleForm'
import { ShareRuleDialog } from '@/components/rules/ShareRuleDialog'
import { useAutoSave } from '@/hooks/useAutoSave'
import {
  type RuleSpec,
  type RuleResource,
  createDefaultRuleSpec,
  ruleSpecSchema,
} from '@/schemas/resources'
import { trackEvent } from '@/utils/analytics'
import { buildRuleShareText } from '@/utils/ruleShare'
import yaml from 'js-yaml'

export function RuleEditPage() {
  const { name: urlName } = useParams({ from: '/rules/$name' })
  const { folder } = useSearch({ from: '/rules/$name' })
  const navigate = useNavigate()
  const { fileTree, selectFile, selectedResource, updateResource, createResource, deleteResource, isLoading } = useGitOps()
  const { toast } = useToast()

  const isNew = urlName === 'new'

  const [resourceName, setResourceName] = useState('')
  const [formData, setFormData] = useState<RuleSpec>(createDefaultRuleSpec())
  const [yamlContent, setYamlContent] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [initialData, setInitialData] = useState<{ name: string; spec: RuleSpec } | null>(null)
  const [isShareOpen, setIsShareOpen] = useState(false)

  // Find and select the rule resource
  useEffect(() => {
    if (isNew) {
      setFormData(createDefaultRuleSpec())
      setResourceName('')
      setInitialData(null)
      return
    }

    const findResourcePath = (node: typeof fileTree): string | null => {
      if (!node) return null
      if (node.resource?.kind === 'Rule' && node.resource.name === urlName) {
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
      const parsed = yaml.load(selectedResource.yaml) as RuleResource
      if (parsed?.spec && parsed.kind === 'Rule') {
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
      setError('Failed to parse rule YAML')
    }
  }, [selectedResource, isNew])

  // Check for unsaved changes
  const hasChanges = useMemo(() => {
    if (isNew) {
      return resourceName.trim() !== '' || formData.category !== ''
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
    const validation = ruleSpecSchema.safeParse(formData)
    return validation.success
  }, [resourceName, formData])

  const canShare = !isNew && resourceName.trim().length > 0
  const shareText = useMemo(
    () => buildRuleShareText(resourceName || urlName, yamlContent),
    [resourceName, urlName, yamlContent],
  )

  // Sync form changes to YAML
  useEffect(() => {
    if (!resourceName && !isNew) return

    try {
      const resource: RuleResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Rule',
        metadata: { name: resourceName || 'new_rule', labels: {}, annotations: {} },
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

    const resource: RuleResource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Rule',
      metadata: { name: resourceName, labels: {}, annotations: {} },
      spec: formData,
    }
    const newYaml = yaml.dump(resource, { lineWidth: -1 })

    const success = await updateResource('Rule', urlName, newYaml)
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
      const parsed = yaml.load(newYaml) as RuleResource
      if (parsed?.spec && parsed.kind === 'Rule') {
        const validated = ruleSpecSchema.safeParse(parsed.spec)
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
      setError('Rule name is required')
      return
    }

    if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(resourceName)) {
      setError('Rule name must start with a letter or underscore, and contain only letters, numbers, underscores, and hyphens')
      return
    }

    // Validate spec
    const validation = ruleSpecSchema.safeParse(formData)
    if (!validation.success) {
      const firstError = validation.error.errors[0]
      setError(`Validation error: ${firstError.path.join('.')}: ${firstError.message}`)
      return
    }

    setIsSaving(true)
    setError(null)

    try {
      const resource: RuleResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Rule',
        metadata: { name: resourceName, labels: {}, annotations: {} },
        spec: formData,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })

      let success: boolean
      if (isNew) {
        // If folder is provided, create the resource in that folder
        const targetPath = folder ? `${folder}/${resourceName}.yaml` : undefined
        success = await createResource('Rule', newYaml, targetPath)
      } else {
        success = await updateResource('Rule', urlName, newYaml)
      }

      if (success) {
        setInitialData({
          name: resourceName,
          spec: JSON.parse(JSON.stringify(formData)),
        })
        toast({
          title: isNew ? 'Rule created' : 'Rule saved',
          description: isNew
            ? `Rule "${resourceName}" has been created.`
            : 'Your changes have been saved.',
        })

        if (isNew) {
          navigate({ to: '/rules/$name', params: { name: resourceName } })
        }
      } else {
        setError(isNew ? 'Failed to create rule' : 'Failed to save rule')
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save rule')
    } finally {
      setIsSaving(false)
    }
  }

  // Delete handler
  const handleDelete = async () => {
    if (isNew) return

    const confirmed = window.confirm(`Are you sure you want to delete the rule "${urlName}"?`)
    if (!confirmed) return

    setIsSaving(true)
    try {
      const success = await deleteResource('Rule', urlName)
      if (success) {
        toast({
          title: 'Rule deleted',
          description: `Rule "${urlName}" has been deleted.`,
        })
        navigate({ to: '/rules' })
      } else {
        setError('Failed to delete rule')
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete rule')
    } finally {
      setIsSaving(false)
    }
  }

  const handleShareOpen = () => {
    if (!canShare) return
    setIsShareOpen(true)
    trackEvent('rule_share_opened', { rule: resourceName || urlName })
  }

  const copyToClipboard = async (text: string) => {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
      return
    }

    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.style.position = 'fixed'
    textarea.style.left = '-9999px'
    textarea.style.top = '-9999px'
    document.body.appendChild(textarea)
    textarea.focus()
    textarea.select()
    const success = document.execCommand('copy')
    document.body.removeChild(textarea)
    if (!success) {
      throw new Error('Copy failed')
    }
  }

  const handleShareCopy = async () => {
    try {
      await copyToClipboard(shareText)
      toast({
        title: 'Share text copied',
        description: 'Send it to a teammate to reuse this rule.',
      })
      trackEvent('rule_share_copied', { rule: resourceName || urlName })
    } catch (err) {
      toast({
        title: 'Copy failed',
        description: err instanceof Error ? err.message : 'Unable to copy share text.',
      })
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-4">
          <Button variant="ghost" size="icon" onClick={() => navigate({ to: '/rules' })}>
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <div className="flex items-center gap-3">
            <FileText className="h-8 w-8" />
            <div>
              <h1 className="text-3xl font-bold tracking-tight">
                {isNew ? 'New Rule' : urlName}
              </h1>
              <p className="text-muted-foreground">
                {isNew ? 'Create a new document classification rule' : 'Edit rule configuration'}
              </p>
            </div>
          </div>
        </div>
        {!isNew && (
          <Button variant="outline" onClick={handleShareOpen} disabled={!canShare}>
            <Share2 className="h-4 w-4 mr-2" />
            Share rule
          </Button>
        )}
      </div>

      <ResourcePanel
        title={isNew ? 'New Rule' : resourceName}
        description="Classify and route documents based on content matching"
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
          <RuleForm
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
            height="500px"
            placeholder="# Rule YAML"
          />
        }
      />

      <ShareRuleDialog
        open={isShareOpen}
        onOpenChange={setIsShareOpen}
        shareText={shareText}
        onCopy={handleShareCopy}
      />
    </div>
  )
}
