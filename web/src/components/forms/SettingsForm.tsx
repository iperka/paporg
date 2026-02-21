import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { TextField, NumberField, SwitchField, SelectField, ArrayField, SecretField, PathField } from '@/components/form'
import { type SettingsSpec } from '@/schemas/resources'
import { Folder, Eye, GitBranch, Settings as SettingsIcon } from 'lucide-react'

interface SettingsFormProps {
  value: SettingsSpec
  onChange: (value: SettingsSpec) => void
  errors?: Record<string, string>
}

export function SettingsForm({ value, onChange, errors = {} }: SettingsFormProps) {
  const updateField = <K extends keyof SettingsSpec>(
    field: K,
    fieldValue: SettingsSpec[K]
  ) => {
    onChange({ ...value, [field]: fieldValue })
  }

  const updateOcr = <K extends keyof SettingsSpec['ocr']>(
    field: K,
    fieldValue: SettingsSpec['ocr'][K]
  ) => {
    onChange({
      ...value,
      ocr: { ...value.ocr, [field]: fieldValue },
    })
  }

  const updateDefaults = <K extends keyof SettingsSpec['defaults']['output']>(
    field: K,
    fieldValue: SettingsSpec['defaults']['output'][K]
  ) => {
    onChange({
      ...value,
      defaults: {
        ...value.defaults,
        output: { ...value.defaults.output, [field]: fieldValue },
      },
    })
  }

  const updateGit = <K extends keyof SettingsSpec['git']>(
    field: K,
    fieldValue: SettingsSpec['git'][K]
  ) => {
    onChange({
      ...value,
      git: { ...value.git, [field]: fieldValue },
    })
  }

  const updateGitAuth = <K extends keyof SettingsSpec['git']['auth']>(
    field: K,
    fieldValue: SettingsSpec['git']['auth'][K] | undefined,
    clearField?: keyof SettingsSpec['git']['auth']
  ) => {
    const updates: Partial<SettingsSpec['git']['auth']> = { [field]: fieldValue }
    if (clearField) {
      updates[clearField] = undefined
    }
    onChange({
      ...value,
      git: {
        ...value.git,
        auth: { ...value.git.auth, ...updates },
      },
    })
  }

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
            <PathField
              label="Input Directory"
              value={value.inputDirectory}
              onChange={(v) => updateField('inputDirectory', v)}
              description="Directory to watch for incoming documents"
              error={errors['inputDirectory']}
              required
              mode="folder"
            />
            <PathField
              label="Output Directory"
              value={value.outputDirectory}
              onChange={(v) => updateField('outputDirectory', v)}
              description="Base directory for organized documents"
              error={errors['outputDirectory']}
              required
              mode="folder"
            />
            <NumberField
              label="Worker Count"
              value={value.workerCount}
              onChange={(v) => updateField('workerCount', v)}
              description="Number of parallel document processing workers"
              error={errors['workerCount']}
              min={1}
              max={32}
            />
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
            <SwitchField
              label="Enable OCR"
              checked={value.ocr.enabled}
              onChange={(v) => updateOcr('enabled', v)}
              description="Extract text from scanned documents and images"
            />
            {value.ocr.enabled && (
              <>
                <ArrayField
                  label="Languages"
                  values={value.ocr.languages}
                  onChange={(v) => updateOcr('languages', v)}
                  description="OCR language codes (e.g., eng, deu, fra)"
                  error={errors['ocr.languages']}
                  placeholder="Enter language code..."
                  addLabel="Add Language"
                  minItems={1}
                />
                <NumberField
                  label="DPI"
                  value={value.ocr.dpi}
                  onChange={(v) => updateOcr('dpi', v)}
                  description="Resolution for OCR processing (higher = more accurate but slower)"
                  error={errors['ocr.dpi']}
                  min={72}
                  max={600}
                />
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
            <TextField
              label="Default Directory Template"
              value={value.defaults.output.directory}
              onChange={(v) => updateDefaults('directory', v)}
              description="Output directory when no rule matches. Supports variables like $y (year), $m (month)"
              error={errors['defaults.output.directory']}
              required
              mono
            />
            <TextField
              label="Default Filename Template"
              value={value.defaults.output.filename}
              onChange={(v) => updateDefaults('filename', v)}
              description="Filename template. Use $original for original filename, $timestamp for date"
              error={errors['defaults.output.filename']}
              required
              mono
            />
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
            <SwitchField
              label="Enable Git Sync"
              checked={value.git.enabled}
              onChange={(v) => updateGit('enabled', v)}
              description="Sync configuration with a Git repository"
            />
            {value.git.enabled && (
              <>
                <TextField
                  label="Repository URL"
                  value={value.git.repository}
                  onChange={(v) => updateGit('repository', v)}
                  description="Git repository URL (HTTPS or SSH)"
                  error={errors['git.repository']}
                  mono
                />
                <TextField
                  label="Branch"
                  value={value.git.branch}
                  onChange={(v) => updateGit('branch', v)}
                  description="Git branch to use"
                  error={errors['git.branch']}
                />
                <NumberField
                  label="Sync Interval (seconds)"
                  value={value.git.syncInterval}
                  onChange={(v) => updateGit('syncInterval', v)}
                  description="How often to sync with remote (0 to disable auto-sync)"
                  error={errors['git.syncInterval']}
                  min={0}
                />
                <TextField
                  label="User Name"
                  value={value.git.userName}
                  onChange={(v) => updateGit('userName', v)}
                  description="Git user name for commits"
                  error={errors['git.userName']}
                />
                <TextField
                  label="User Email"
                  value={value.git.userEmail}
                  onChange={(v) => updateGit('userEmail', v)}
                  description="Git user email for commits"
                  error={errors['git.userEmail']}
                />
                <SelectField
                  label="Authentication Type"
                  value={value.git.auth.type}
                  onChange={(v) => updateGitAuth('type', v as 'none' | 'token' | 'ssh-key')}
                  options={[
                    { value: 'none', label: 'None (public repo or SSH agent)' },
                    { value: 'ssh-key', label: 'SSH Key File' },
                    { value: 'token', label: 'Access Token' },
                  ]}
                  description="SSH agent keys are used automatically when 'None' is selected"
                />
                {value.git.auth.type === 'token' && (
                  <SecretField
                    label="Git Token"
                    sourceName="git"
                    secretType="token"
                    filePath={value.git.auth.tokenFile}
                    envVar={value.git.auth.tokenEnvVar}
                    onFilePathChange={(v) => updateGitAuth('tokenFile', v, 'tokenEnvVar')}
                    onEnvVarChange={(v) => updateGitAuth('tokenEnvVar', v, 'tokenFile')}
                    description="Personal access token for Git authentication"
                  />
                )}
                {value.git.auth.type === 'ssh-key' && (
                  <PathField
                    label="SSH Key Path"
                    value={value.git.auth.sshKeyPath || ''}
                    onChange={(v) => updateGitAuth('sshKeyPath', v)}
                    description="Path to SSH private key (leave empty to use SSH agent)"
                    placeholder="~/.ssh/id_rsa"
                    mode="file"
                  />
                )}
              </>
            )}
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  )
}
