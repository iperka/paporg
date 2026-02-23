import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Plus, Trash2, X } from 'lucide-react'
import {
  type MatchCondition,
  type MatchConditionType,
  getMatchConditionType,
  createMatchConditionOfType,
} from '@/schemas/resources'
import { cn } from '@/lib/utils'

interface MatchConditionBuilderProps {
  condition: MatchCondition
  onChange: (condition: MatchCondition) => void
  depth?: number
}

const MAX_DEPTH = 4

export function MatchConditionBuilder({
  condition,
  onChange,
  depth = 0,
}: MatchConditionBuilderProps) {
  const uniqueId = useId()
  const type = getMatchConditionType(condition)

  const caseSensitive = 'caseSensitive' in condition ? condition.caseSensitive : undefined

  /** Spread caseSensitive onto a new condition object if it was set. */
  const withCaseSensitive = (cond: MatchCondition): MatchCondition => {
    if (caseSensitive !== undefined) {
      ;(cond as Record<string, unknown>).caseSensitive = caseSensitive
    }
    return cond
  }

  const handleTypeChange = (newType: MatchConditionType) => {
    onChange(withCaseSensitive(createMatchConditionOfType(newType)))
  }

  const handleCaseSensitiveChange = (checked: boolean) => {
    const updated = { ...condition }
    if (checked) {
      ;(updated as Record<string, unknown>).caseSensitive = true
    } else {
      delete (updated as Record<string, unknown>).caseSensitive
    }
    onChange(updated as MatchCondition)
  }

  const renderSimpleCondition = () => {
    if ('contains' in condition) {
      return (
        <Input
          value={condition.contains}
          onChange={(e) => onChange(withCaseSensitive({ contains: e.target.value }))}
          placeholder="Text to search for..."
          className="font-mono"
        />
      )
    }

    if ('pattern' in condition) {
      return (
        <Input
          value={condition.pattern}
          onChange={(e) => onChange(withCaseSensitive({ pattern: e.target.value }))}
          placeholder="Regex pattern..."
          className="font-mono"
        />
      )
    }

    if ('containsAny' in condition) {
      return (
        <StringArrayEditor
          values={condition.containsAny}
          onChange={(values) => onChange(withCaseSensitive({ containsAny: values }))}
          placeholder="Add text..."
        />
      )
    }

    if ('containsAll' in condition) {
      return (
        <StringArrayEditor
          values={condition.containsAll}
          onChange={(values) => onChange(withCaseSensitive({ containsAll: values }))}
          placeholder="Add text..."
        />
      )
    }

    return null
  }

  const renderCompoundCondition = () => {
    if (depth >= MAX_DEPTH) {
      return (
        <p className="text-sm text-muted-foreground">
          Maximum nesting depth reached
        </p>
      )
    }

    if ('all' in condition) {
      return (
        <CompoundConditionList
          conditions={condition.all}
          onChange={(conditions) => onChange(withCaseSensitive({ all: conditions }))}
          depth={depth}
          label="All conditions must match (AND)"
        />
      )
    }

    if ('any' in condition) {
      return (
        <CompoundConditionList
          conditions={condition.any}
          onChange={(conditions) => onChange(withCaseSensitive({ any: conditions }))}
          depth={depth}
          label="Any condition must match (OR)"
        />
      )
    }

    if ('not' in condition) {
      return (
        <div className="space-y-2">
          <Label className="text-xs text-muted-foreground">
            Condition to negate (NOT)
          </Label>
          <MatchConditionBuilder
            condition={condition.not}
            onChange={(c) => onChange(withCaseSensitive({ not: c }))}
            depth={depth + 1}
          />
        </div>
      )
    }

    return null
  }

  const isCompound = type === 'all' || type === 'any' || type === 'not'

  return (
    <div
      className={cn(
        'space-y-3 p-3 rounded-lg border',
        depth === 0 && 'bg-muted/30',
        depth === 1 && 'bg-muted/50',
        depth === 2 && 'bg-muted/70',
        depth >= 3 && 'bg-muted'
      )}
    >
      <div className="flex items-center gap-2">
        <Select value={type} onValueChange={(v) => handleTypeChange(v as MatchConditionType)}>
          <SelectTrigger className="w-48">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="contains">Contains</SelectItem>
            <SelectItem value="containsAny">Contains Any</SelectItem>
            <SelectItem value="containsAll">Contains All</SelectItem>
            <SelectItem value="pattern">Regex Pattern</SelectItem>
            <SelectItem value="all" disabled={depth >= MAX_DEPTH}>
              All (AND)
            </SelectItem>
            <SelectItem value="any" disabled={depth >= MAX_DEPTH}>
              Any (OR)
            </SelectItem>
            <SelectItem value="not" disabled={depth >= MAX_DEPTH}>
              Not
            </SelectItem>
          </SelectContent>
        </Select>

        <div className="flex items-center gap-2 ml-auto">
          <Label htmlFor={`${uniqueId}-case-sensitive`} className="text-xs text-muted-foreground cursor-pointer">
            Case Sensitive
          </Label>
          <Switch
            id={`${uniqueId}-case-sensitive`}
            checked={caseSensitive === true}
            onCheckedChange={handleCaseSensitiveChange}
          />
        </div>

        {depth > 0 && (
          <Badge variant="outline" className="text-xs">
            Depth: {depth}
          </Badge>
        )}
      </div>

      {isCompound ? renderCompoundCondition() : renderSimpleCondition()}
    </div>
  )
}

interface CompoundConditionListProps {
  conditions: MatchCondition[]
  onChange: (conditions: MatchCondition[]) => void
  depth: number
  label: string
}

function CompoundConditionList({
  conditions,
  onChange,
  depth,
  label,
}: CompoundConditionListProps) {
  const addCondition = () => {
    onChange([...conditions, { contains: '' }])
  }

  const removeCondition = (index: number) => {
    const newConditions = conditions.filter((_, i) => i !== index)
    onChange(newConditions.length > 0 ? newConditions : [{ contains: '' }])
  }

  const updateCondition = (index: number, condition: MatchCondition) => {
    const newConditions = [...conditions]
    newConditions[index] = condition
    onChange(newConditions)
  }

  return (
    <div className="space-y-3">
      <Label className="text-xs text-muted-foreground">{label}</Label>

      {conditions.map((cond, index) => (
        <div key={index} className="flex items-start gap-2">
          <div className="flex-1">
            <MatchConditionBuilder
              condition={cond}
              onChange={(c) => updateCondition(index, c)}
              depth={depth + 1}
            />
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => removeCondition(index)}
            className="shrink-0 mt-1"
            disabled={conditions.length === 1}
          >
            <Trash2 className="h-4 w-4 text-muted-foreground" />
          </Button>
        </div>
      ))}

      <Button type="button" variant="outline" size="sm" onClick={addCondition}>
        <Plus className="h-4 w-4 mr-2" />
        Add Condition
      </Button>
    </div>
  )
}

interface StringArrayEditorProps {
  values: string[]
  onChange: (values: string[]) => void
  placeholder: string
}

function StringArrayEditor({ values, onChange, placeholder }: StringArrayEditorProps) {
  const addValue = () => {
    onChange([...values, ''])
  }

  const removeValue = (index: number) => {
    const newValues = values.filter((_, i) => i !== index)
    onChange(newValues.length > 0 ? newValues : [''])
  }

  const updateValue = (index: number, value: string) => {
    const newValues = [...values]
    newValues[index] = value
    onChange(newValues)
  }

  return (
    <div className="space-y-2">
      {values.map((value, index) => (
        <div key={index} className="flex items-center gap-2">
          <Input
            value={value}
            onChange={(e) => updateValue(index, e.target.value)}
            placeholder={placeholder}
            className="font-mono"
          />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => removeValue(index)}
            disabled={values.length === 1}
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
      ))}
      <Button type="button" variant="outline" size="sm" onClick={addValue}>
        <Plus className="h-4 w-4 mr-2" />
        Add Value
      </Button>
    </div>
  )
}
