import { useState, useEffect, useCallback } from 'react'
import { Save, RotateCcw, Trash2, AlertCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useToast } from '@/components/ui/use-toast'
import { useSelectedFile } from '@/contexts/SelectedFileContext'
import { useUpdateResource, useDeleteResource } from '@/mutations/use-gitops-mutations'
import type { ResourceKind } from '@/types/gitops'
import * as yaml from 'js-yaml'

interface ParsedResource {
  kind?: ResourceKind
  metadata?: { name?: string }
}

export function ResourceEditor() {
  const { selectedResource, selectedPath } = useSelectedFile()
  const updateResourceMut = useUpdateResource()
  const deleteResourceMut = useDeleteResource()

  const isLoading = updateResourceMut.isPending || deleteResourceMut.isPending

  const { toast } = useToast()

  const [content, setContent] = useState('')
  const [hasChanges, setHasChanges] = useState(false)
  const [parseError, setParseError] = useState<string | null>(null)
  const [resourceKind, setResourceKind] = useState<ResourceKind | null>(null)
  const [resourceName, setResourceName] = useState<string | null>(null)

  // Update content when selected resource changes
  useEffect(() => {
    if (selectedResource) {
      setContent(selectedResource.yaml)
      setHasChanges(false)
      setParseError(null)

      // Try to parse and get resource info
      try {
        const parsed = yaml.load(selectedResource.yaml) as ParsedResource | undefined
        if (parsed?.kind) {
          setResourceKind(parsed.kind)
          setResourceName(parsed.metadata?.name || null)
        }
      } catch {
        // Ignore parse errors for now
      }
    } else {
      setContent('')
      setHasChanges(false)
      setParseError(null)
      setResourceKind(null)
      setResourceName(null)
    }
  }, [selectedResource])

  const handleChange = (newContent: string) => {
    setContent(newContent)
    setHasChanges(newContent !== selectedResource?.yaml)

    // Validate YAML
    try {
      const parsed = yaml.load(newContent)
      if (parsed && typeof parsed === 'object') {
        setParseError(null)

        // Update resource info
        const obj = parsed as ParsedResource
        if (obj.kind) {
          setResourceKind(obj.kind)
          setResourceName(obj.metadata?.name || null)
        }
      }
    } catch (e) {
      setParseError(e instanceof Error ? e.message : 'Invalid YAML')
    }
  }

  const handleSave = useCallback(async () => {
    if (!resourceKind || !resourceName) {
      toast({
        title: 'Error',
        description: 'Invalid resource: missing kind or name',
        variant: 'destructive',
      })
      return
    }

    try {
      await updateResourceMut.mutateAsync({ kind: resourceKind, name: resourceName, yamlContent: content })
      setHasChanges(false)
      toast({
        title: 'Saved',
        description: `${resourceKind} "${resourceName}" saved successfully`,
      })
    } catch (e) {
      toast({
        title: 'Error',
        description: e instanceof Error ? e.message : 'Failed to save resource',
        variant: 'destructive',
      })
    }
  }, [resourceKind, resourceName, content, updateResourceMut, toast])

  const handleRevert = () => {
    if (selectedResource) {
      setContent(selectedResource.yaml)
      setHasChanges(false)
      setParseError(null)
    }
  }

  const handleDelete = async () => {
    if (!resourceKind || !resourceName) return

    if (resourceKind === 'Settings') {
      toast({
        title: 'Error',
        description: 'Cannot delete Settings resource',
        variant: 'destructive',
      })
      return
    }

    const confirmed = confirm(`Are you sure you want to delete "${resourceName}"?`)
    if (!confirmed) return

    try {
      await deleteResourceMut.mutateAsync({ kind: resourceKind, name: resourceName })
      toast({
        title: 'Deleted',
        description: `${resourceKind} "${resourceName}" deleted`,
      })
    } catch (e) {
      toast({
        title: 'Error',
        description: e instanceof Error ? e.message : 'Failed to delete resource',
        variant: 'destructive',
      })
    }
  }

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault()
        if (hasChanges && !parseError && !isLoading) {
          handleSave()
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [hasChanges, parseError, isLoading, content, resourceKind, resourceName, handleSave])

  if (!selectedResource) {
    return (
      <div className="h-full flex items-center justify-center text-muted-foreground">
        <div className="text-center">
          <p>Select a file to edit</p>
          <p className="text-sm mt-1">or create a new resource</p>
        </div>
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-3 border-b flex items-center gap-2 bg-muted/30">
        <div className="flex-1 min-w-0">
          <h3 className="font-medium truncate">{selectedResource.name}</h3>
          <p className="text-xs text-muted-foreground truncate">{selectedPath}</p>
        </div>

        <div className="flex items-center gap-1">
          {hasChanges && (
            <Button
              variant="ghost"
              size="icon"
              onClick={handleRevert}
              title="Revert changes"
              disabled={isLoading}
            >
              <RotateCcw className="h-4 w-4" />
            </Button>
          )}

          <Button
            variant="ghost"
            size="icon"
            onClick={handleSave}
            disabled={!hasChanges || !!parseError || isLoading}
            title="Save (Cmd+S)"
          >
            <Save className={`h-4 w-4 ${hasChanges ? 'text-primary' : ''}`} />
          </Button>

          {resourceKind !== 'Settings' && (
            <Button
              variant="ghost"
              size="icon"
              onClick={handleDelete}
              disabled={isLoading}
              title="Delete resource"
              className="text-destructive hover:text-destructive"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          )}
        </div>
      </div>

      {/* Error Banner */}
      {parseError && (
        <div className="px-3 py-2 bg-destructive/10 text-destructive text-sm flex items-center gap-2">
          <AlertCircle className="h-4 w-4" />
          <span>YAML Error: {parseError}</span>
        </div>
      )}

      {/* Editor */}
      <div className="flex-1 relative">
        <textarea
          value={content}
          onChange={(e) => handleChange(e.target.value)}
          className="absolute inset-0 w-full h-full p-4 font-mono text-sm bg-background resize-none focus:outline-none"
          spellCheck={false}
          disabled={isLoading}
        />
      </div>

      {/* Status Bar */}
      <div className="px-3 py-1.5 border-t bg-muted/30 flex items-center gap-4 text-xs text-muted-foreground">
        <span>
          {resourceKind && (
            <>
              Kind: <strong>{resourceKind}</strong>
            </>
          )}
        </span>
        {resourceName && (
          <span>
            Name: <strong>{resourceName}</strong>
          </span>
        )}
        <span className="flex-1" />
        {hasChanges && <span className="text-primary">Unsaved changes</span>}
        {isLoading && <span>Saving...</span>}
      </div>
    </div>
  )
}
