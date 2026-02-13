import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'
import { Code, FormInput, Save, Loader2, AlertCircle, Trash2, Check, Cloud } from 'lucide-react'

type AutoSaveStatus = 'idle' | 'pending' | 'saving' | 'saved' | 'error'

interface ResourcePanelProps {
  title: string
  description?: string
  isLoading?: boolean
  isSaving?: boolean
  error?: string | null
  hasChanges?: boolean
  onSave?: () => void
  onDelete?: () => void
  formContent: React.ReactNode
  yamlContent: React.ReactNode
  isNew?: boolean
  /** Auto-save status */
  autoSaveStatus?: AutoSaveStatus
  /** Last saved timestamp for auto-save */
  lastSaved?: Date | null
}

function AutoSaveIndicator({ status, lastSaved }: { status?: AutoSaveStatus; lastSaved?: Date | null }) {
  if (!status || status === 'idle') {
    if (lastSaved) {
      return (
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Cloud className="h-3 w-3" />
          Saved
        </span>
      )
    }
    return null
  }

  switch (status) {
    case 'pending':
      return (
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Loader2 className="h-3 w-3 animate-spin" />
          Saving...
        </span>
      )
    case 'saving':
      return (
        <span className="flex items-center gap-1.5 text-xs text-amber-600">
          <Loader2 className="h-3 w-3 animate-spin" />
          Saving...
        </span>
      )
    case 'saved':
      return (
        <span className="flex items-center gap-1.5 text-xs text-green-600">
          <Check className="h-3 w-3" />
          Saved
        </span>
      )
    case 'error':
      return (
        <span className="flex items-center gap-1.5 text-xs text-destructive">
          <AlertCircle className="h-3 w-3" />
          Save failed
        </span>
      )
    default:
      return null
  }
}

export function ResourcePanel({
  title,
  description,
  isLoading,
  isSaving,
  error,
  hasChanges,
  onSave,
  onDelete,
  formContent,
  yamlContent,
  isNew,
  autoSaveStatus,
  lastSaved,
}: ResourcePanelProps) {
  const [mode, setMode] = useState<'form' | 'yaml'>('form')
  const isAutoSaveEnabled = !isNew && autoSaveStatus !== undefined

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div className="space-y-1">
            <CardTitle className="flex items-center gap-2">
              {title}
              {isAutoSaveEnabled && (
                <AutoSaveIndicator status={autoSaveStatus} lastSaved={lastSaved} />
              )}
              {!isAutoSaveEnabled && hasChanges && (
                <Badge variant="secondary" className="text-xs">
                  Unsaved changes
                </Badge>
              )}
            </CardTitle>
            {description && (
              <CardDescription>{description}</CardDescription>
            )}
          </div>
          <div className="flex items-center gap-2">
            {!isNew && onDelete && (
              <Button
                variant="destructive"
                size="sm"
                onClick={onDelete}
                disabled={isLoading || isSaving}
              >
                <Trash2 className="h-4 w-4 mr-2" />
                Delete
              </Button>
            )}
            {onSave && isNew && (
              <Button
                onClick={onSave}
                disabled={isLoading || isSaving || !hasChanges}
                size="sm"
              >
                {isSaving ? (
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                ) : (
                  <Save className="h-4 w-4 mr-2" />
                )}
                Create
              </Button>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        {error && (
          <div className="flex items-center gap-2 p-3 mb-4 text-sm text-destructive bg-destructive/10 rounded-lg">
            <AlertCircle className="h-4 w-4" />
            {error}
          </div>
        )}

        {isLoading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <Tabs value={mode} onValueChange={(v) => setMode(v as 'form' | 'yaml')}>
            <TabsList className="mb-4">
              <TabsTrigger value="form" className="gap-2">
                <FormInput className="h-4 w-4" />
                Form
              </TabsTrigger>
              <TabsTrigger value="yaml" className="gap-2">
                <Code className="h-4 w-4" />
                YAML
              </TabsTrigger>
            </TabsList>

            <TabsContent value="form" className="mt-0">
              {formContent}
            </TabsContent>

            <TabsContent value="yaml" className="mt-0">
              {yamlContent}
            </TabsContent>
          </Tabs>
        )}
      </CardContent>
    </Card>
  )
}
