import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { TextField, NumberField, ArrayField } from '@/components/form'
import { Label } from '@/components/ui/label'
import { MatchConditionBuilder } from './MatchConditionBuilder'
import { type RuleSpec } from '@/schemas/resources'
import { Filter, FileOutput, Link2, Settings } from 'lucide-react'

interface RuleFormProps {
  value: RuleSpec
  onChange: (value: RuleSpec) => void
  errors?: Record<string, string>
  isNew?: boolean
  name?: string
  onNameChange?: (name: string) => void
}

export function RuleForm({
  value,
  onChange,
  errors = {},
  isNew,
  name,
  onNameChange,
}: RuleFormProps) {
  const updateField = <K extends keyof RuleSpec>(
    field: K,
    fieldValue: RuleSpec[K]
  ) => {
    onChange({ ...value, [field]: fieldValue })
  }

  const updateOutput = <K extends keyof RuleSpec['output']>(
    field: K,
    fieldValue: RuleSpec['output'][K]
  ) => {
    onChange({
      ...value,
      output: { ...value.output, [field]: fieldValue },
    })
  }

  const handleSymlinksChange = (targets: string[]) => {
    const symlinks = targets
      .filter((t) => t.trim() !== '')
      .map((target) => ({ target }))
    updateField('symlinks', symlinks)
  }

  const symlinkTargets = (value.symlinks || []).map((s) => s.target)

  return (
    <Accordion type="multiple" defaultValue={['basic', 'match', 'output']} className="w-full">
      {/* Basic Settings */}
      <AccordionItem value="basic">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Settings className="h-4 w-4" />
            Basic Settings
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            {isNew && onNameChange && (
              <TextField
                label="Rule Name"
                value={name || ''}
                onChange={onNameChange}
                description="Unique identifier for this rule"
                error={errors['name']}
                required
                placeholder="my_rule"
              />
            )}

            <TextField
              label="Category"
              value={value.category}
              onChange={(v) => updateField('category', v)}
              description="Category for this rule (used for organization)"
              error={errors['category']}
              required
              placeholder="invoices"
            />

            <NumberField
              label="Priority"
              value={value.priority}
              onChange={(v) => updateField('priority', v)}
              description="Higher priority rules are matched first (default: 0)"
              error={errors['priority']}
            />
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* Match Conditions */}
      <AccordionItem value="match">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Filter className="h-4 w-4" />
            Match Conditions
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <div className="space-y-2">
              <Label>Match Condition</Label>
              <p className="text-xs text-muted-foreground mb-3">
                Define conditions that must match for this rule to apply. Use simple conditions
                (contains, pattern) or combine them with AND/OR/NOT logic.
              </p>
              <MatchConditionBuilder
                condition={value.match}
                onChange={(match) => updateField('match', match)}
              />
            </div>
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* Output Settings */}
      <AccordionItem value="output">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <FileOutput className="h-4 w-4" />
            Output Settings
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <TextField
              label="Output Directory"
              value={value.output.directory}
              onChange={(v) => updateOutput('directory', v)}
              description="Output directory template. Variables: $y (year), $m (month), $category, custom variables"
              error={errors['output.directory']}
              required
              mono
              placeholder="$y/$category"
            />

            <TextField
              label="Filename"
              value={value.output.filename}
              onChange={(v) => updateOutput('filename', v)}
              description="Filename template. Variables: $original, $timestamp, custom variables"
              error={errors['output.filename']}
              required
              mono
              placeholder="$original"
            />
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* Symlinks */}
      <AccordionItem value="symlinks">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Link2 className="h-4 w-4" />
            Symlinks
            {symlinkTargets.length > 0 && (
              <span className="text-xs text-muted-foreground">
                ({symlinkTargets.length})
              </span>
            )}
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <ArrayField
              label="Symlink Targets"
              values={symlinkTargets.length > 0 ? symlinkTargets : []}
              onChange={handleSymlinksChange}
              description="Additional locations to create symlinks to the output file"
              placeholder="$y/all_documents"
              addLabel="Add Symlink"
              mono
            />
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  )
}
