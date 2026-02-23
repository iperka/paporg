import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { TextField, NumberField, ArrayField } from '@/components/form'
import { Label } from '@/components/ui/label'
import { MatchConditionBuilder } from './MatchConditionBuilder'
import { type MatchCondition, type SymlinkSettings } from '@/schemas/resources'
import { Filter, FileOutput, Link2, Settings } from 'lucide-react'
import type { FormInstance } from '@/lib/form-utils'

interface RuleFormProps {
  form: FormInstance
  isNew?: boolean
  name?: string
  onNameChange?: (name: string) => void
}

export function RuleForm({
  form,
  isNew,
  name,
  onNameChange,
}: RuleFormProps) {
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
                required
                placeholder="my_rule"
              />
            )}

            <form.Field name="category" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Category"
                value={field.state.value}
                onChange={field.handleChange}
                description="Category for this rule (used for organization)"
                error={field.state.meta.errors?.[0]}
                required
                placeholder="invoices"
              />
            )} />

            <form.Field name="priority" children={(field: { state: { value: number; meta: { errors: string[] } }; handleChange: (v: number) => void }) => (
              <NumberField
                label="Priority"
                value={field.state.value}
                onChange={field.handleChange}
                description="Higher priority rules are matched first (default: 0)"
                error={field.state.meta.errors?.[0]}
              />
            )} />
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
              <form.Field name="match" children={(field: { state: { value: MatchCondition; meta: { errors: string[] } }; handleChange: (v: MatchCondition) => void }) => (
                <MatchConditionBuilder
                  condition={field.state.value}
                  onChange={field.handleChange}
                />
              )} />
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
            <form.Field name="output.directory" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Output Directory"
                value={field.state.value}
                onChange={field.handleChange}
                description="Output directory template. Variables: $y (year), $l (last year), $m (month), $category, custom variables"
                error={field.state.meta.errors?.[0]}
                required
                mono
                placeholder="$y/$category"
              />
            )} />

            <form.Field name="output.filename" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Filename"
                value={field.state.value}
                onChange={field.handleChange}
                description="Filename template. Variables: $original, $timestamp, custom variables"
                error={field.state.meta.errors?.[0]}
                required
                mono
                placeholder="$original"
              />
            )} />
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* Symlinks */}
      <AccordionItem value="symlinks">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Link2 className="h-4 w-4" />
            Symlinks
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="symlinks" children={(field: { state: { value: SymlinkSettings[]; meta: { errors: string[] } }; handleChange: (v: SymlinkSettings[]) => void }) => {
              const symlinkTargets = (field.state.value || []).map((s: SymlinkSettings) => s.target)
              return (
                <ArrayField
                  label="Symlink Targets"
                  values={symlinkTargets.length > 0 ? symlinkTargets : []}
                  onChange={(targets: string[]) => {
                    const symlinks = targets
                      .filter((t) => t.trim() !== '')
                      .map((target) => ({ target }))
                    field.handleChange(symlinks)
                  }}
                  description="Additional locations to create symlinks to the output file"
                  error={field.state.meta.errors?.[0]}
                  placeholder="$y/all_documents"
                  addLabel="Add Symlink"
                  mono
                />
              )
            }} />
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  )
}
