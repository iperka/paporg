import { useEffect, useState, useCallback } from 'react'
import yaml from 'js-yaml'
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Code,
  Download,
  Loader2,
  Play,
  Sparkles,
  X,
  XCircle,
} from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useAiSuggestions } from '@/hooks/useAiSuggestions'
import { useJobOcr } from '@/hooks/useJobOcr'
import { YamlEditor } from '@/components/ui/yaml-editor'
import { api } from '@/api'
import { toast } from '@/components/ui/use-toast'
import type { StoredJob } from '@/types/jobs'
import { requiresOcr } from '@/types/jobs'
import type { RuleSuggestion, ExistingRuleSummary } from '@/types/ai'
import { API_VERSION } from '@/types/gitops'

interface CreateRuleFromJobDialogProps {
  open: boolean
  onClose: () => void
  job: StoredJob | null
}

type MatchType = 'contains' | 'containsAny' | 'containsAll' | 'pattern'

interface RuleForm {
  name: string
  category: string
  priority: number
  matchType: MatchType
  matchValue: string
  outputDirectory: string
  outputFilename: string
}

interface TestResult {
  category: string
  outputDirectory: string
  outputFilename: string
  symlinks: string[]
  matchedRule?: string
}

const defaultForm: RuleForm = {
  name: '',
  category: '',
  priority: 50,
  matchType: 'contains',
  matchValue: '',
  outputDirectory: '$y/$category',
  outputFilename: '$original',
}

export function CreateRuleFromJobDialog({
  open,
  onClose,
  job,
}: CreateRuleFromJobDialogProps) {
  const [form, setForm] = useState<RuleForm>(defaultForm)
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestResult | null>(null)
  const [testError, setTestError] = useState<string | null>(null)
  const [existingRules, setExistingRules] = useState<ExistingRuleSummary[]>([])
  const [isUpdatingRule, setIsUpdatingRule] = useState(false)
  const [updateRuleName, setUpdateRuleName] = useState<string | null>(null)
  const [yamlContent, setYamlContent] = useState('')
  const [yamlEditMode, setYamlEditMode] = useState(false)
  const [yamlError, setYamlError] = useState<string | null>(null)

  const {
    status,
    statusLoading,
    suggestionsLoading,
    downloading,
    suggestions,
    error: aiError,
    fetchStatus,
    downloadModel,
    getSuggestions,
    clear,
  } = useAiSuggestions()

  const {
    ocrText: fetchedOcrText,
    loading: ocrLoading,
    error: ocrError,
    fetchOcr,
    clear: clearOcr,
  } = useJobOcr()

  // Fetch AI status, OCR text, and existing rules when dialog opens
  useEffect(() => {
    if (!open) return

    let cancelled = false

    fetchStatus()
    setForm(defaultForm)
    setSaveError(null)
    setTestResult(null)
    setTestError(null)
    setIsUpdatingRule(false)
    setUpdateRuleName(null)
    setYamlContent('')
    setYamlEditMode(false)
    setYamlError(null)
    clear()
    clearOcr()

    // Fetch OCR text for the job
    if (job?.jobId) {
      fetchOcr(job.jobId)
    }

    // Fetch existing rules for AI context
    api.gitops.listResources('Rule').then(resources => {
      if (cancelled) return
      // Fetch each rule's details to get match values
      Promise.all(
        resources.map(async (item) => {
          try {
            const detail = await api.gitops.getResource('Rule', item.name)
            const parsed = parseYamlRule(detail.yaml)
            return parsed
          } catch (err) {
            console.error('Failed to fetch rule details for', item.name, err)
            return null
          }
        })
      ).then(rules => {
        if (cancelled) return
        setExistingRules(rules.filter((r): r is ExistingRuleSummary => r !== null))
      })
    }).catch(err => {
      console.error('Failed to fetch rules:', err)
    })

    return () => { cancelled = true }
  }, [open, fetchStatus, clear, clearOcr, job?.jobId, fetchOcr])

  // Generate YAML for the rule
  const generateYaml = useCallback(() => {
    const matchCondition: Record<string, unknown> = {}

    if (form.matchType === 'containsAny' || form.matchType === 'containsAll') {
      const values = form.matchValue
        .split(',')
        .map(v => v.trim())
        .filter(Boolean)
      matchCondition[form.matchType] = values
    } else if (form.matchType === 'pattern') {
      matchCondition.pattern = form.matchValue
    } else {
      matchCondition.contains = form.matchValue
    }

    // Convert to YAML (simple conversion)
    const lines = [
      `apiVersion: ${API_VERSION}`,
      'kind: Rule',
      'metadata:',
      `  name: ${form.name}`,
      'spec:',
      `  priority: ${form.priority}`,
      `  category: ${form.category}`,
      '  match:',
    ]

    if (form.matchType === 'containsAny' || form.matchType === 'containsAll') {
      lines.push(`    ${form.matchType}:`)
      const values = form.matchValue
        .split(',')
        .map(v => v.trim())
        .filter(Boolean)
      values.forEach(v => lines.push(`      - "${v}"`))
    } else if (form.matchType === 'pattern') {
      lines.push(`    pattern: "${form.matchValue}"`)
    } else {
      lines.push(`    contains: "${form.matchValue}"`)
    }

    lines.push('  output:')
    lines.push(`    directory: "${form.outputDirectory}"`)
    lines.push(`    filename: "${form.outputFilename}"`)

    return lines.join('\n')
  }, [form])

  // Sync form changes to YAML when not in edit mode
  useEffect(() => {
    if (!yamlEditMode) {
      setYamlContent(generateYaml())
    }
  }, [form, yamlEditMode, generateYaml])

  // Parse edited YAML back to form
  const parseYamlToForm = useCallback((yamlStr: string) => {
    try {
      const parsed = yaml.load(yamlStr) as {
        metadata?: { name?: string }
        spec?: {
          category?: string
          priority?: number
          match?: {
            contains?: string
            containsAny?: string[]
            containsAll?: string[]
            pattern?: string
          }
          output?: {
            directory?: string
            filename?: string
          }
        }
      } | null

      if (!parsed?.metadata?.name || !parsed?.spec) {
        throw new Error('Invalid rule YAML structure')
      }

      const match = parsed.spec.match || {}
      let matchType: MatchType = 'contains'
      let matchValue = ''

      if (match.containsAny) {
        matchType = 'containsAny'
        matchValue = match.containsAny.join(', ')
      } else if (match.containsAll) {
        matchType = 'containsAll'
        matchValue = match.containsAll.join(', ')
      } else if (match.pattern) {
        matchType = 'pattern'
        matchValue = match.pattern
      } else if (match.contains) {
        matchType = 'contains'
        matchValue = match.contains
      }

      setForm({
        name: parsed.metadata.name,
        category: parsed.spec.category || '',
        priority: parsed.spec.priority || 50,
        matchType,
        matchValue,
        outputDirectory: parsed.spec.output?.directory || '$y/$category',
        outputFilename: parsed.spec.output?.filename || '$original',
      })
      setYamlError(null)
    } catch (err) {
      setYamlError(err instanceof Error ? err.message : 'Invalid YAML')
    }
  }, [])

  // Parse YAML rule to extract match info using js-yaml
  function parseYamlRule(yamlContent: string): ExistingRuleSummary | null {
    try {
      const parsed = yaml.load(yamlContent) as {
        metadata?: { name?: string }
        spec?: {
          category?: string
          match?: {
            contains?: string
            containsAny?: string[]
            containsAll?: string[]
            pattern?: string
          }
        }
      } | null

      if (!parsed || typeof parsed !== 'object') return null

      const name = parsed.metadata?.name
      const category = parsed.spec?.category
      const match = parsed.spec?.match

      if (!name || !category || !match) return null

      let matchType = 'contains'
      let matchValues: string[] = []

      if (match.containsAny && Array.isArray(match.containsAny)) {
        matchType = 'containsAny'
        matchValues = match.containsAny
      } else if (match.containsAll && Array.isArray(match.containsAll)) {
        matchType = 'containsAll'
        matchValues = match.containsAll
      } else if (match.pattern) {
        matchType = 'pattern'
        matchValues = [match.pattern]
      } else if (match.contains) {
        matchType = 'contains'
        matchValues = [match.contains]
      }

      return {
        name,
        category,
        matchType,
        matchValues,
      }
    } catch {
      return null
    }
  }

  // Update form field
  const updateForm = useCallback((field: keyof RuleForm, value: string | number) => {
    setForm(prev => ({ ...prev, [field]: value }))
  }, [])

  // Apply suggestion to form
  const applySuggestion = useCallback((suggestion: RuleSuggestion) => {
    // Handle update suggestion
    if (suggestion.isUpdate && suggestion.updateRuleName) {
      setIsUpdatingRule(true)
      setUpdateRuleName(suggestion.updateRuleName)

      // Find the existing rule to get its current values
      const existingRule = existingRules.find(r => r.name === suggestion.updateRuleName)
      if (existingRule) {
        // Combine existing values with new values
        const allValues = [...existingRule.matchValues, ...(suggestion.addValues || [])]
        setForm(prev => ({
          ...prev,
          category: existingRule.category,
          matchType: 'containsAny',
          matchValue: allValues.join(', '),
          name: existingRule.name,
          outputDirectory: suggestion.outputDirectory || prev.outputDirectory,
          outputFilename: suggestion.outputFilename || prev.outputFilename,
        }))
      }
      return
    }

    // Handle new rule suggestion
    setIsUpdatingRule(false)
    setUpdateRuleName(null)

    const matchValue = Array.isArray(suggestion.matchValue)
      ? suggestion.matchValue.join(', ')
      : suggestion.matchValue

    // Slugify the category for the rule name
    const slugifiedCategory = suggestion.category
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '')

    setForm(prev => ({
      ...prev,
      category: slugifiedCategory,
      matchType: suggestion.matchType as MatchType,
      matchValue,
      name: `${slugifiedCategory}-rule`,
      outputDirectory: suggestion.outputDirectory || `$category/$y/$m`,
      outputFilename: suggestion.outputFilename || '$y-$m-$d_$original',
    }))
  }, [existingRules])

  // Get AI suggestions
  const handleGetSuggestions = useCallback(async () => {
    if (!fetchedOcrText || !job?.filename) return
    await getSuggestions(fetchedOcrText, job.filename, existingRules)
  }, [fetchedOcrText, job?.filename, getSuggestions, existingRules])

  // Download model
  const handleDownloadModel = useCallback(async () => {
    await downloadModel()
  }, [downloadModel])

  // Test the rule
  const handleTestRule = useCallback(async () => {
    if (!fetchedOcrText || !form.matchValue || !job?.filename) return

    setTesting(true)
    setTestError(null)
    setTestResult(null)

    try {
      const result = await api.gitops.simulateRule(fetchedOcrText, job.filename)
      setTestResult({
        category: result.category,
        outputDirectory: result.outputDirectory,
        outputFilename: result.outputFilename,
        symlinks: result.symlinks,
        matchedRule: result.matchedRule || undefined,
      })
    } catch (err) {
      setTestError(err instanceof Error ? err.message : 'Failed to test rule')
    } finally {
      setTesting(false)
    }
  }, [fetchedOcrText, job?.filename, form])

  // Save rule
  const handleSave = useCallback(async () => {
    const finalYaml = yamlEditMode ? yamlContent : generateYaml()

    // Validate YAML before saving
    try {
      const parsed = yaml.load(finalYaml) as { metadata?: { name?: string } } | null
      if (!parsed?.metadata?.name) {
        setSaveError('Invalid YAML: missing metadata.name')
        return
      }
    } catch {
      setSaveError('Invalid YAML syntax')
      return
    }

    // When not in YAML edit mode, validate form fields
    if (!yamlEditMode && (!form.name || !form.category || !form.matchValue)) {
      setSaveError('Please fill in all required fields')
      return
    }

    setSaving(true)
    setSaveError(null)

    try {
      if (isUpdatingRule && updateRuleName) {
        // Update existing rule
        await api.gitops.updateResource('Rule', updateRuleName, finalYaml)
        toast({
          title: 'Rule updated',
          description: `Rule "${updateRuleName}" has been updated`,
        })
        onClose()
      } else {
        // Create new rule - extract name from YAML when in edit mode
        const ruleName = yamlEditMode
          ? (yaml.load(finalYaml) as { metadata?: { name?: string } })?.metadata?.name || form.name
          : form.name
        await api.gitops.createResource('Rule', ruleName, finalYaml)
        toast({
          title: 'Rule created',
          description: `Rule "${ruleName}" has been created`,
        })
        onClose()
      }
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to save rule')
    } finally {
      setSaving(false)
    }
  }, [form, yamlContent, yamlEditMode, generateYaml, onClose, isUpdatingRule, updateRuleName])

  const ocrText = fetchedOcrText || ''
  const hasOcrText = ocrText.length > 0
  const isOcrBased = requiresOcr(job?.mimeType)
  const noTextMessage = isOcrBased
    ? 'No OCR text available for this document'
    : 'No text content available for this document'

  return (
    <Dialog open={open} onOpenChange={open => !open && onClose()}>
      <DialogContent className="max-w-7xl w-[95vw] h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>Create Rule from Document</DialogTitle>
          <DialogDescription>
            Create a classification rule based on "{job?.filename}"
          </DialogDescription>
        </DialogHeader>

        <div className="flex-1 min-h-0 grid grid-cols-1 md:grid-cols-3 gap-4 overflow-hidden">
          {/* Left panel - OCR Text and AI Suggestions */}
          <div className="flex flex-col min-h-0 overflow-hidden">
            <ScrollArea className="flex-1">
              <div className="flex flex-col gap-4 pr-4">
                {/* Document Text */}
                <div className="flex flex-col">
                  <Label className="mb-2 flex items-center gap-2">
                    Document Text
                    <Badge variant="outline" className="text-xs">
                      {isOcrBased ? 'OCR' : 'Direct'}
                    </Badge>
                    {ocrLoading && <Loader2 className="h-3 w-3 animate-spin" />}
                  </Label>
                  {ocrLoading ? (
                    <div className="border rounded-md p-3 flex items-center justify-center text-muted-foreground min-h-[100px]">
                      <div className="text-center">
                        <Loader2 className="h-8 w-8 mx-auto mb-2 animate-spin opacity-50" />
                        <p>Extracting text...</p>
                      </div>
                    </div>
                  ) : ocrError ? (
                    <div className="border rounded-md p-3 flex items-center justify-center text-muted-foreground min-h-[100px]">
                      <div className="text-center">
                        <AlertTriangle className="h-8 w-8 mx-auto mb-2 opacity-50 text-destructive" />
                        <p className="text-destructive">{ocrError}</p>
                      </div>
                    </div>
                  ) : hasOcrText ? (
                    <div className="border rounded-md p-3 bg-muted/30 max-h-[40vh] overflow-auto">
                      <pre className="text-xs whitespace-pre-wrap font-mono">{ocrText}</pre>
                    </div>
                  ) : (
                    <div className="border rounded-md p-3 flex items-center justify-center text-muted-foreground min-h-[100px]">
                      <div className="text-center">
                        <AlertTriangle className="h-8 w-8 mx-auto mb-2 opacity-50" />
                        <p>{noTextMessage}</p>
                      </div>
                    </div>
                  )}
                </div>

            {/* AI Suggestions */}
            <div className="flex flex-col">
              <div className="flex items-center justify-between mb-2">
                <Label className="flex items-center gap-2">
                  <Sparkles className="h-4 w-4 text-amber-500" />
                  AI Suggestions
                </Label>
                {statusLoading && <Loader2 className="h-4 w-4 animate-spin" />}
              </div>

              <div className="border rounded-md p-3 space-y-3">
                {/* AI Status */}
                {status && !status.available && (
                  <div className="text-sm text-muted-foreground">
                    AI suggestions are not available
                  </div>
                )}

                {status && status.available && !status.modelLoaded && (
                  <div className="space-y-2">
                    <p className="text-sm">
                      AI model ({status.modelName || 'default'}) needs to be downloaded
                    </p>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleDownloadModel}
                      disabled={downloading}
                    >
                      {downloading ? (
                        <>
                          <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                          Downloading...
                        </>
                      ) : (
                        <>
                          <Download className="h-4 w-4 mr-2" />
                          Download Model
                        </>
                      )}
                    </Button>
                  </div>
                )}

                {status?.available && status?.modelLoaded && hasOcrText && (
                  <>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleGetSuggestions}
                      disabled={suggestionsLoading}
                      className="w-full"
                    >
                      {suggestionsLoading ? (
                        <>
                          <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                          Analyzing document...
                        </>
                      ) : (
                        <>
                          <Sparkles className="h-4 w-4 mr-2" />
                          Get AI Suggestions
                        </>
                      )}
                    </Button>

                    {suggestions.length > 0 && (
                      <div className="space-y-2">
                        {suggestions.map((suggestion) => (
                          <button
                            key={`${suggestion.category}-${suggestion.matchType}-${suggestion.confidence}`}
                            onClick={() => applySuggestion(suggestion)}
                            className={`w-full text-left p-3 rounded-md border hover:bg-muted/50 transition-colors ${
                              suggestion.isUpdate ? 'border-blue-300 bg-blue-50/50 dark:bg-blue-950/20' : ''
                            }`}
                          >
                            <div className="flex items-center justify-between">
                              <div className="flex items-center gap-2">
                                <span className="font-medium">{suggestion.category}</span>
                                {suggestion.isUpdate && (
                                  <Badge variant="outline" className="text-blue-600 border-blue-300">
                                    Update
                                  </Badge>
                                )}
                              </div>
                              <Badge variant="secondary">
                                {Math.round(suggestion.confidence * 100)}%
                              </Badge>
                            </div>
                            <p className="text-xs text-muted-foreground mt-1">
                              {suggestion.reasoning}
                            </p>
                            {suggestion.isUpdate && suggestion.addValues && (
                              <div className="flex items-center gap-2 mt-2 text-xs">
                                <Badge variant="outline" className="text-blue-600">Add to {suggestion.updateRuleName}</Badge>
                                <span className="text-muted-foreground truncate">
                                  {suggestion.addValues.join(', ')}
                                </span>
                              </div>
                            )}
                            {!suggestion.isUpdate && (
                              <div className="flex items-center gap-2 mt-2 text-xs">
                                <Badge variant="outline">{suggestion.matchType}</Badge>
                                <span className="text-muted-foreground truncate">
                                  {Array.isArray(suggestion.matchValue)
                                    ? suggestion.matchValue.join(', ')
                                    : suggestion.matchValue}
                                </span>
                              </div>
                            )}
                            {suggestion.outputDirectory && (
                              <div className="text-xs text-muted-foreground mt-1 font-mono">
                                → {suggestion.outputDirectory}/{suggestion.outputFilename || '$original'}
                              </div>
                            )}
                          </button>
                        ))}
                      </div>
                    )}
                  </>
                )}

                {aiError && (
                  <div className="text-sm text-destructive flex items-center gap-2">
                    <X className="h-4 w-4" />
                    {aiError}
                  </div>
                )}
              </div>
            </div>
              </div>
            </ScrollArea>
          </div>

          {/* Middle panel - Rule Form */}
          <div className="flex flex-col min-h-0 overflow-hidden">
            <ScrollArea className="flex-1">
              <div className="space-y-4 pr-4">
              <div className="grid gap-2">
                <Label htmlFor="rule-name">Rule Name *</Label>
                <Input
                  id="rule-name"
                  placeholder="e.g., invoices-acme"
                  value={form.name}
                  onChange={e => updateForm('name', e.target.value)}
                />
              </div>

              <div className="grid gap-2">
                <Label htmlFor="category">Category *</Label>
                <Input
                  id="category"
                  placeholder="e.g., invoices"
                  value={form.category}
                  onChange={e => updateForm('category', e.target.value)}
                />
              </div>

              <div className="grid gap-2">
                <Label htmlFor="priority">Priority</Label>
                <Input
                  id="priority"
                  type="number"
                  value={form.priority}
                  onChange={e => updateForm('priority', parseInt(e.target.value, 10) || 0)}
                />
                <p className="text-xs text-muted-foreground">
                  Higher priority rules are evaluated first
                </p>
              </div>

              <Separator />

              <div className="grid gap-2">
                <Label>Match Condition</Label>
                <Select
                  value={form.matchType}
                  onValueChange={value => updateForm('matchType', value)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="contains">Contains (single term)</SelectItem>
                    <SelectItem value="containsAny">Contains Any (one of many)</SelectItem>
                    <SelectItem value="containsAll">Contains All (all terms)</SelectItem>
                    <SelectItem value="pattern">Pattern (regex)</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="grid gap-2">
                <Label htmlFor="match-value">Match Value *</Label>
                <Input
                  id="match-value"
                  placeholder={
                    form.matchType === 'containsAny' || form.matchType === 'containsAll'
                      ? 'term1, term2, term3'
                      : form.matchType === 'pattern'
                        ? 'Invoice.*\\d+'
                        : 'Invoice'
                  }
                  value={form.matchValue}
                  onChange={e => updateForm('matchValue', e.target.value)}
                />
                {(form.matchType === 'containsAny' || form.matchType === 'containsAll') && (
                  <p className="text-xs text-muted-foreground">
                    Separate multiple terms with commas
                  </p>
                )}
              </div>

              <Separator />

              <div className="grid gap-2">
                <Label htmlFor="output-dir">Output Directory</Label>
                <Input
                  id="output-dir"
                  placeholder="$y/$category"
                  value={form.outputDirectory}
                  onChange={e => updateForm('outputDirectory', e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  Variables: $y (year), $m (month), $d (day), $category, $original, etc.
                </p>
              </div>

              <div className="grid gap-2">
                <Label htmlFor="output-file">Output Filename</Label>
                <Input
                  id="output-file"
                  placeholder="$original"
                  value={form.outputFilename}
                  onChange={e => updateForm('outputFilename', e.target.value)}
                />
              </div>

              <Separator />

              {/* Test Rule Section */}
              <div className="space-y-3">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleTestRule}
                  disabled={testing || !form.matchValue || !hasOcrText}
                  className="w-full"
                >
                  {testing ? (
                    <>
                      <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                      Testing...
                    </>
                  ) : (
                    <>
                      <Play className="h-4 w-4 mr-2" />
                      Test Rule
                    </>
                  )}
                </Button>

                {testResult && (
                  <div className={`rounded-md p-3 text-sm ${
                    testResult.matchedRule
                      ? 'bg-green-50 border border-green-200 dark:bg-green-950/30 dark:border-green-800'
                      : 'bg-red-50 border border-red-200 dark:bg-red-950/30 dark:border-red-800'
                  }`}>
                    <div className="flex items-center gap-2 mb-2">
                      {testResult.matchedRule ? (
                        <>
                          <CheckCircle2 className="h-4 w-4 text-green-600" />
                          <span className="font-medium text-green-700 dark:text-green-400">
                            Rule matches! ({testResult.matchedRule})
                          </span>
                        </>
                      ) : (
                        <>
                          <XCircle className="h-4 w-4 text-red-600" />
                          <span className="font-medium text-red-700 dark:text-red-400">No matching rule</span>
                        </>
                      )}
                    </div>
                    {testResult.matchedRule && (
                      <div className="text-xs font-mono bg-background/50 rounded p-2 mt-2 space-y-1">
                        <div><span className="text-muted-foreground">Category:</span> {testResult.category}</div>
                        <div><span className="text-muted-foreground">Output:</span> {testResult.outputDirectory}/{testResult.outputFilename}</div>
                        {testResult.symlinks.length > 0 && (
                          <div><span className="text-muted-foreground">Symlinks:</span> {testResult.symlinks.join(', ')}</div>
                        )}
                      </div>
                    )}
                  </div>
                )}

                {testError && (
                  <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3 text-destructive text-sm">
                    {testError}
                  </div>
                )}
              </div>
              </div>
            </ScrollArea>
          </div>

          {/* Right panel - YAML Editor */}
          <div className="flex flex-col min-h-0 overflow-hidden">
            <div className="flex items-center justify-between mb-2">
              <Label className="flex items-center gap-2">
                <Code className="h-4 w-4" />
                YAML {yamlEditMode ? '(Editing)' : '(Preview)'}
              </Label>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  if (yamlEditMode) {
                    parseYamlToForm(yamlContent)
                  }
                  setYamlEditMode(!yamlEditMode)
                }}
              >
                {yamlEditMode ? 'Apply Changes' : 'Edit YAML'}
              </Button>
            </div>

            <div className="flex-1 min-h-0 border rounded-md overflow-hidden">
              <YamlEditor
                value={yamlContent}
                onChange={setYamlContent}
                height="100%"
                readOnly={!yamlEditMode}
              />
            </div>

            {yamlError && (
              <div className="mt-2 text-sm text-destructive flex items-center gap-2">
                <AlertTriangle className="h-4 w-4" />
                {yamlError}
              </div>
            )}
          </div>
        </div>

        {saveError && (
          <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3 text-destructive text-sm">
            {saveError}
          </div>
        )}

        <DialogFooter>
          {isUpdatingRule && (
            <div className="flex-1 text-sm text-blue-600 dark:text-blue-400">
              Updating existing rule: <span className="font-mono">{updateRuleName}</span>
            </div>
          )}
          <Button variant="outline" onClick={onClose} disabled={saving}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving || (!yamlEditMode && (!form.name || !form.category || !form.matchValue))}>
            {saving ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                {isUpdatingRule ? 'Updating...' : 'Saving...'}
              </>
            ) : (
              <>
                <Check className="h-4 w-4 mr-2" />
                {isUpdatingRule ? 'Update Rule' : 'Save Rule'}
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
