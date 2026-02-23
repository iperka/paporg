import { useParams, useNavigate, useSearch } from '@tanstack/react-router'
import { useEffect, useState, useMemo, useCallback } from 'react'
import { FileText, ArrowLeft, Share2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { YamlEditor } from '@/components/ui/yaml-editor'
import { useResource } from '@/queries/use-resource'
import { useUpdateResource, useCreateResource, useDeleteResource } from '@/mutations/use-gitops-mutations'
import { useToast } from '@/components/ui/use-toast'
import { ResourcePanel } from '@/components/resource/ResourcePanel'
import { RuleForm } from '@/components/forms/RuleForm'
import { ShareRuleDialog } from '@/components/rules/ShareRuleDialog'
import { useAutoSave } from '@/hooks/useAutoSave'
import { useForm, useStore } from '@tanstack/react-form'
import {
  type RuleResource,
  createDefaultRuleSpec,
  ruleSpecSchema,
} from '@/schemas/resources'
import { zodFormValidator } from '@/lib/form-utils'
import { trackEvent } from '@/utils/analytics'
import { buildRuleShareText } from '@/utils/ruleShare'
import yaml from 'js-yaml'

export function RuleEditPage() {
  const { name: urlName } = useParams({ from: '/rules/$name' })
  const { folder } = useSearch({ from: '/rules/$name' })
  const navigate = useNavigate()
  const { toast } = useToast()

  const isNew = urlName === 'new'

  // Load resource directly via useResource for non-new resources
  const { data: resourceData, isLoading: isResourceLoading } = useResource('Rule', urlName, !isNew)

  // Mutations
  const updateResourceMut = useUpdateResource()
  const createResourceMut = useCreateResource()
  const deleteResourceMut = useDeleteResource()

  const isLoading = isResourceLoading || updateResourceMut.isPending || createResourceMut.isPending || deleteResourceMut.isPending

  const [resourceName, setResourceName] = useState('')
  const [initialName, setInitialName] = useState('')
  const [yamlContent, setYamlContent] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [isShareOpen, setIsShareOpen] = useState(false)

  // TanStack Form - use function validator for Zod schemas with .default() fields
  const form = useForm({
    defaultValues: createDefaultRuleSpec(),
    validators: {
      onChange: zodFormValidator(ruleSpecSchema),
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
      form.reset(createDefaultRuleSpec())
      setResourceName('')
      setInitialName('')
      return
    }

    if (!resourceData?.yaml) return

    try {
      const parsed = yaml.load(resourceData.yaml) as RuleResource
      if (parsed?.spec && parsed.kind === 'Rule') {
        form.reset(parsed.spec)
        setResourceName(parsed.metadata.name)
        setInitialName(parsed.metadata.name)
        setYamlContent(resourceData.yaml)
        setError(null)
      }
    } catch {
      setError('Failed to parse rule YAML')
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resourceData, isNew])

  // Check for unsaved changes
  const hasChanges = useMemo(() => {
    if (isNew) {
      return resourceName.trim() !== '' || formValues.category !== ''
    }
    return isDirty || resourceName !== initialName
  }, [formValues, resourceName, initialName, isNew, isDirty])

  // Check if form is valid for auto-save
  const isValidForSave = useMemo(() => {
    if (!resourceName.trim()) return false
    if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(resourceName)) return false
    return canSubmit
  }, [resourceName, canSubmit])

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

    const resource: RuleResource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Rule',
      metadata: { name: savedName, labels: {}, annotations: {} },
      spec: savedValues,
    }
    const newYaml = yaml.dump(resource, { lineWidth: -1 })

    try {
      await updateResourceMut.mutateAsync({ kind: 'Rule', name: urlName, yamlContent: newYaml })
      // Reset form baseline after save (updates default values to current)
      form.reset(savedValues)
      setInitialName(savedName)
    } catch {
      throw new Error('Failed to save')
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isNew, isValidForSave, resourceName, formValues, updateResourceMut, urlName])

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
      const parsed = yaml.load(newYaml) as RuleResource
      if (parsed?.spec && parsed.kind === 'Rule') {
        const validated = ruleSpecSchema.safeParse(parsed.spec)
        if (validated.success) {
          for (const [key, val] of Object.entries(validated.data)) {
            // @ts-expect-error TS2589: DeepKeys<RuleSpec> infinite recursion from recursive MatchCondition
            form.setFieldValue(key, val)
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
    // Validate name
    if (!resourceName.trim()) {
      setError('Rule name is required')
      return
    }

    if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(resourceName)) {
      setError('Rule name must start with a letter or underscore, and contain only letters, numbers, underscores, and hyphens')
      return
    }

    if (!canSubmit) {
      setError('Form has validation errors')
      return
    }

    setIsSaving(true)
    setError(null)

    try {
      const resource: RuleResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Rule',
        metadata: { name: resourceName, labels: {}, annotations: {} },
        spec: formValues,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })

      if (isNew) {
        const targetPath = folder ? `${folder}/${resourceName}.yaml` : undefined
        await createResourceMut.mutateAsync({ kind: 'Rule', yamlContent: newYaml, path: targetPath })
      } else {
        await updateResourceMut.mutateAsync({ kind: 'Rule', name: urlName, yamlContent: newYaml })
      }

      form.reset(formValues)
      setInitialName(resourceName)
      toast({
        title: isNew ? 'Rule created' : 'Rule saved',
        description: isNew
          ? `Rule "${resourceName}" has been created.`
          : 'Your changes have been saved.',
      })

      if (isNew) {
        navigate({ to: '/rules/$name', params: { name: resourceName } })
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : isNew ? 'Failed to create rule' : 'Failed to save rule')
    } finally {
      setIsSaving(false)
    }
  }

  // Delete handler
  const handleDelete = async () => {
    if (isNew || isSaving) return

    const confirmed = window.confirm(`Are you sure you want to delete the rule "${urlName}"?`)
    if (!confirmed) return

    setIsSaving(true)
    try {
      await deleteResourceMut.mutateAsync({ kind: 'Rule', name: urlName })
      toast({
        title: 'Rule deleted',
        description: `Rule "${urlName}" has been deleted.`,
      })
      navigate({ to: '/rules' })
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
