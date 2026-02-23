import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { TextField, NumberField, SwitchField, SelectField, ArrayField, SecretField, PathField } from '@/components/form'
import { Folder, Eye, GitBranch, Settings as SettingsIcon } from 'lucide-react'
import { useStore } from '@tanstack/react-form'
import type { FormInstance } from '@/lib/form-utils'

interface SettingsFormProps {
  form: FormInstance
}

export function SettingsForm({ form }: SettingsFormProps) {
  // Subscribe to conditional-rendering values at the top level
  const ocrEnabled: boolean = useStore(form.store, (state) => state.values.ocr.enabled)
  const gitEnabled: boolean = useStore(form.store, (state) => state.values.git.enabled)
  const gitAuthType: string = useStore(form.store, (state) => state.values.git.auth.type)

  return (
    <Accordion type="multiple" defaultValue={['general', 'ocr', 'defaults']} className="w-full">
      {/* General Settings */}
      <AccordionItem value="general">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Folder className="h-4 w-4" />
            General Settings
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="inputDirectory" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
              <PathField
                label="Input Directory"
                value={field.state.value}
                onChange={field.handleChange}
                description="Directory to watch for incoming documents"
                error={field.state.meta.errors?.[0]}
                required
                mode="folder"
              />
            )} />
            <form.Field name="outputDirectory" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
              <PathField
                label="Output Directory"
                value={field.state.value}
                onChange={field.handleChange}
                description="Base directory for organized documents"
                error={field.state.meta.errors?.[0]}
                required
                mode="folder"
              />
            )} />
            <form.Field name="workerCount" children={(field: { state: { value: number; meta: { errors: string[] } }; handleChange: (v: number) => void }) => (
              <NumberField
                label="Worker Count"
                value={field.state.value}
                onChange={field.handleChange}
                description="Number of parallel document processing workers"
                error={field.state.meta.errors?.[0]}
                min={1}
                max={32}
              />
            )} />
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* OCR Settings */}
      <AccordionItem value="ocr">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Eye className="h-4 w-4" />
            OCR Settings
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="ocr.enabled" children={(field: { state: { value: boolean; meta: { errors: string[] } }; handleChange: (v: boolean) => void }) => (
              <SwitchField
                label="Enable OCR"
                checked={field.state.value}
                onChange={field.handleChange}
                description="Extract text from scanned documents and images"
              />
            )} />
            {ocrEnabled && (
              <>
                <form.Field name="ocr.languages" children={(field: { state: { value: string[]; meta: { errors: string[] } }; handleChange: (v: string[]) => void }) => (
                  <ArrayField
                    label="Languages"
                    values={field.state.value}
                    onChange={field.handleChange}
                    description="OCR language codes (e.g., eng, deu, fra)"
                    error={field.state.meta.errors?.[0]}
                    placeholder="Enter language code..."
                    addLabel="Add Language"
                    minItems={1}
                  />
                )} />
                <form.Field name="ocr.dpi" children={(field: { state: { value: number; meta: { errors: string[] } }; handleChange: (v: number) => void }) => (
                  <NumberField
                    label="DPI"
                    value={field.state.value}
                    onChange={field.handleChange}
                    description="Resolution for OCR processing (higher = more accurate but slower)"
                    error={field.state.meta.errors?.[0]}
                    min={72}
                    max={600}
                  />
                )} />
              </>
            )}
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* Default Output Settings */}
      <AccordionItem value="defaults">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <SettingsIcon className="h-4 w-4" />
            Default Output
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="defaults.output.directory" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Default Directory Template"
                value={field.state.value}
                onChange={field.handleChange}
                description="Output directory when no rule matches. Supports variables like $y (year), $m (month)"
                error={field.state.meta.errors?.[0]}
                required
                mono
              />
            )} />
            <form.Field name="defaults.output.filename" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Default Filename Template"
                value={field.state.value}
                onChange={field.handleChange}
                description="Filename template. Use $original for original filename, $timestamp for date"
                error={field.state.meta.errors?.[0]}
                required
                mono
              />
            )} />
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* Git Settings */}
      <AccordionItem value="git">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <GitBranch className="h-4 w-4" />
            Git Sync
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="git.enabled" children={(field: { state: { value: boolean; meta: { errors: string[] } }; handleChange: (v: boolean) => void }) => (
              <SwitchField
                label="Enable Git Sync"
                checked={field.state.value}
                onChange={field.handleChange}
                description="Sync configuration with a Git repository"
              />
            )} />
            {gitEnabled && (
              <>
                <form.Field name="git.repository" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
                  <TextField
                    label="Repository URL"
                    value={field.state.value}
                    onChange={field.handleChange}
                    description="Git repository URL (HTTPS or SSH)"
                    error={field.state.meta.errors?.[0]}
                    mono
                  />
                )} />
                <form.Field name="git.branch" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
                  <TextField
                    label="Branch"
                    value={field.state.value}
                    onChange={field.handleChange}
                    description="Git branch to use"
                    error={field.state.meta.errors?.[0]}
                  />
                )} />
                <form.Field name="git.syncInterval" children={(field: { state: { value: number; meta: { errors: string[] } }; handleChange: (v: number) => void }) => (
                  <NumberField
                    label="Sync Interval (seconds)"
                    value={field.state.value}
                    onChange={field.handleChange}
                    description="How often to sync with remote (0 to disable auto-sync)"
                    error={field.state.meta.errors?.[0]}
                    min={0}
                  />
                )} />
                <form.Field name="git.userName" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
                  <TextField
                    label="User Name"
                    value={field.state.value}
                    onChange={field.handleChange}
                    description="Git user name for commits"
                    error={field.state.meta.errors?.[0]}
                  />
                )} />
                <form.Field name="git.userEmail" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
                  <TextField
                    label="User Email"
                    value={field.state.value}
                    onChange={field.handleChange}
                    description="Git user email for commits"
                    error={field.state.meta.errors?.[0]}
                  />
                )} />
                <form.Field name="git.auth.type" children={(field: { state: { value: string; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
                  <SelectField
                    label="Authentication Type"
                    value={field.state.value}
                    onChange={(v: string) => field.handleChange(v as 'none' | 'token' | 'ssh-key')}
                    options={[
                      { value: 'none', label: 'None (public repo or SSH agent)' },
                      { value: 'ssh-key', label: 'SSH Key File' },
                      { value: 'token', label: 'Access Token' },
                    ]}
                    description="SSH agent keys are used automatically when 'None' is selected"
                  />
                )} />
                {gitAuthType === 'token' && (
                  <form.Field name="git.auth.tokenFile" children={(tokenFileField: { state: { value: string | undefined; meta: { errors: string[] } }; handleChange: (v: string | undefined) => void }) => (
                    <form.Field name="git.auth.tokenEnvVar" children={(tokenEnvVarField: { state: { value: string | undefined; meta: { errors: string[] } }; handleChange: (v: string | undefined) => void }) => (
                      <SecretField
                        label="Git Token"
                        sourceName="git"
                        secretType="token"
                        filePath={tokenFileField.state.value}
                        envVar={tokenEnvVarField.state.value}
                        onFilePathChange={(v) => {
                          tokenFileField.handleChange(v)
                          if (v !== undefined) {
                            tokenEnvVarField.handleChange(undefined)
                          }
                        }}
                        onEnvVarChange={(v) => {
                          tokenEnvVarField.handleChange(v)
                          if (v !== undefined) {
                            tokenFileField.handleChange(undefined)
                          }
                        }}
                        description="Personal access token for Git authentication"
                      />
                    )} />
                  )} />
                )}
                {gitAuthType === 'ssh-key' && (
                  <form.Field name="git.auth.sshKeyPath" children={(field: { state: { value: string | undefined; meta: { errors: string[] } }; handleChange: (v: string) => void }) => (
                    <PathField
                      label="SSH Key Path"
                      value={field.state.value || ''}
                      onChange={field.handleChange}
                      description="Path to SSH private key (leave empty to use SSH agent)"
                      placeholder="~/.ssh/id_rsa"
                      mode="file"
                    />
                  )} />
                )}
              </>
            )}
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  )
}
