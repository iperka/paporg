import { TextField, NumberField, SwitchField, ArrayField, SecretField, SelectField, DateField, PathField } from '@/components/form'
import {
  type ImportSourceSpec,
  type ImportSourceType,
  type EmailAuthType,
  type OAuth2Provider,
  createDefaultEmailSourceConfig,
} from '@/schemas/resources'
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { FolderOpen, Mail, Server, Shield, Filter, Clock, Info, Settings } from 'lucide-react'
import { useStore } from '@tanstack/react-form'
import { open } from '@tauri-apps/plugin-shell'
import type { FormInstance } from '@/lib/form-utils'
import { cn } from '@/lib/utils'

interface ImportSourceFormProps {
  form: FormInstance
  isNew?: boolean
  name?: string
  onNameChange?: (name: string) => void
}

// ---------------------------------------------------------------------------
// SourceTypeSelector – radio card buttons (replaces Radix Select)
// ---------------------------------------------------------------------------

function SourceTypeSelector({
  value,
  onChange,
}: {
  value: ImportSourceType
  onChange: (type: ImportSourceType) => void
}) {
  const options = [
    { type: 'local' as const, icon: FolderOpen, label: 'Local Directory', desc: 'Watch a folder for new files' },
    { type: 'email' as const, icon: Mail, label: 'Email (IMAP)', desc: 'Import attachments from email' },
  ]

  return (
    <div className="space-y-2">
      <label className="text-sm font-medium">Source Type</label>
      <div className="grid grid-cols-2 gap-3">
        {options.map(({ type, icon: Icon, label, desc }) => (
          <button
            key={type}
            type="button"
            onClick={() => onChange(type)}
            className={cn(
              'flex flex-col items-start gap-1 rounded-lg border-2 p-4 text-left transition-colors',
              value === type
                ? 'border-primary bg-primary/5'
                : 'border-muted hover:border-muted-foreground/25',
            )}
          >
            <div className="flex items-center gap-2 font-medium">
              <Icon className="h-4 w-4" />
              {label}
            </div>
            <span className="text-xs text-muted-foreground">{desc}</span>
          </button>
        ))}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// LocalSourceSection – single Card with all local-directory fields
// ---------------------------------------------------------------------------

function LocalSourceSection({ form }: { form: FormInstance }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-lg">
          <FolderOpen className="h-5 w-5" />
          Local Directory Settings
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <form.Field name="local.path" children={(field: { state: { value: string | undefined; meta: { errors?: string[] } }; handleChange: (v: string) => void }) => (
          <PathField
            label="Path"
            value={field.state.value || ''}
            onChange={field.handleChange}
            description="Absolute path to the directory to watch for new files"
            error={field.state.meta.errors?.[0]}
            required
            placeholder="/Users/me/Downloads"
            mode="folder"
          />
        )} />

        <form.Field name="local.recursive" children={(field: { state: { value: boolean | undefined; meta: { errors?: string[] } }; handleChange: (v: boolean) => void }) => (
          <SwitchField
            label="Recursive"
            checked={field.state.value ?? false}
            onChange={field.handleChange}
            description="Watch subdirectories for new files"
          />
        )} />

        <form.Field name="local.pollInterval" children={(field: { state: { value: number | undefined; meta: { errors?: string[] } }; handleChange: (v: number) => void }) => (
          <NumberField
            label="Poll Interval"
            value={field.state.value ?? 60}
            onChange={field.handleChange}
            description="How often to check for new files (in seconds)"
            min={1}
            max={86400}
            error={field.state.meta.errors?.[0]}
          />
        )} />

        <form.Field name="local.filters.include" children={(field: { state: { value: string[] | undefined; meta: { errors?: string[] } }; handleChange: (v: string[]) => void }) => (
          <ArrayField
            label="Include Patterns"
            values={field.state.value || ['*']}
            onChange={field.handleChange}
            description="Glob patterns to match files to include (e.g., *.pdf, *.png)"
            placeholder="*.pdf"
            addLabel="Add Pattern"
            mono
            minItems={1}
          />
        )} />

        <form.Field name="local.filters.exclude" children={(field: { state: { value: string[] | undefined; meta: { errors?: string[] } }; handleChange: (v: string[]) => void }) => (
          <ArrayField
            label="Exclude Patterns"
            values={field.state.value || []}
            onChange={field.handleChange}
            description="Glob patterns to match files to exclude (e.g., *.tmp, .*)"
            placeholder="*.tmp"
            addLabel="Add Pattern"
            mono
          />
        )} />
      </CardContent>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Email provider presets
// ---------------------------------------------------------------------------

const EMAIL_PRESETS = [
  { id: 'gmail',   label: 'Gmail',        host: 'imap.gmail.com',        port: 993, useTls: true },
  { id: 'outlook', label: 'Outlook',      host: 'outlook.office365.com', port: 993, useTls: true },
  { id: 'icloud',  label: 'Apple iCloud', host: 'imap.mail.me.com',     port: 993, useTls: true },
] as const

function EmailProviderPresets({
  form,
  emailHost,
}: {
  form: FormInstance
  emailHost: string | undefined
}) {
  const activePreset = EMAIL_PRESETS.find((p) => p.host === emailHost)?.id ?? 'custom'

  const applyPreset = (preset: (typeof EMAIL_PRESETS)[number]) => {
    form.setFieldValue('email.host', preset.host)
    form.setFieldValue('email.port', preset.port)
    form.setFieldValue('email.useTls', preset.useTls)
  }

  return (
    <div className="space-y-2">
      <label className="text-sm font-medium">Email Provider</label>
      <div className="grid grid-cols-4 gap-2">
        {EMAIL_PRESETS.map((preset) => (
          <button
            key={preset.id}
            type="button"
            onClick={() => applyPreset(preset)}
            className={cn(
              'flex items-center justify-center gap-2 rounded-lg border-2 px-3 py-2 text-sm font-medium transition-colors',
              activePreset === preset.id
                ? 'border-primary bg-primary/5'
                : 'border-muted hover:border-muted-foreground/25',
            )}
          >
            <Mail className="h-4 w-4 shrink-0" />
            {preset.label}
          </button>
        ))}
        <button
          type="button"
          onClick={() => {
            form.setFieldValue('email.host', '')
            form.setFieldValue('email.port', 993)
            form.setFieldValue('email.useTls', true)
          }}
          className={cn(
            'flex items-center justify-center gap-2 rounded-lg border-2 px-3 py-2 text-sm font-medium transition-colors',
            activePreset === 'custom'
              ? 'border-primary bg-primary/5'
              : 'border-muted hover:border-muted-foreground/25',
          )}
        >
          <Settings className="h-4 w-4 shrink-0" />
          Custom
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// EmailSourceSection – Accordion with Server / Auth / Filters / Processing
// ---------------------------------------------------------------------------

function EmailSourceSection({
  form,
  isNew,
  name,
}: {
  form: FormInstance
  isNew?: boolean
  name?: string
}) {
  const emailHost: string | undefined = useStore(form.store, (state) => state.values.email?.host)
  const authType: EmailAuthType = useStore(form.store, (state) => state.values.email?.auth?.type) ?? 'password'
  const oauth2Provider: OAuth2Provider | undefined = useStore(form.store, (state) => state.values.email?.auth?.oauth2?.provider)
  const minAttachmentSize: number = useStore(form.store, (state) => state.values.email?.minAttachmentSize) ?? 0
  const maxAttachmentSize: number = useStore(form.store, (state) => state.values.email?.maxAttachmentSize) ?? 52428800
  const isMinGreaterThanMax = minAttachmentSize > maxAttachmentSize

  const sourceName = isNew ? (name || 'new-source') : (name ?? '')

  const handleAuthTypeChange = (newAuthType: string) => {
    form.setFieldValue('email.auth.type', newAuthType as EmailAuthType)
    if (newAuthType === 'oauth2') {
      form.setFieldValue('email.auth.passwordFile', undefined)
      form.setFieldValue('email.auth.passwordEnvVar', undefined)
      form.setFieldValue('email.auth.passwordInsecure', undefined)
      form.setFieldValue('email.auth.oauth2', { provider: 'gmail' })
    } else {
      form.setFieldValue('email.auth.oauth2', undefined)
    }
  }

  return (
    <div className="space-y-4">
      <EmailProviderPresets form={form} emailHost={emailHost} />

      <Accordion type="multiple" defaultValue={['server', 'auth']} className="w-full">
      {/* ── Server ── */}
      <AccordionItem value="server">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Server className="h-4 w-4" />
            Server
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="email.host" children={(field: { state: { value: string | undefined; meta: { errors?: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Host"
                value={field.state.value || ''}
                onChange={field.handleChange}
                description="IMAP server hostname (e.g., imap.gmail.com)"
                error={field.state.meta.errors?.[0]}
                required
                placeholder="imap.gmail.com"
              />
            )} />

            <div className="grid grid-cols-2 gap-4">
              <form.Field name="email.port" children={(field: { state: { value: number | undefined; meta: { errors?: string[] } }; handleChange: (v: number) => void }) => (
                <NumberField
                  label="Port"
                  value={field.state.value ?? 993}
                  onChange={field.handleChange}
                  description="IMAP server port"
                  min={1}
                  max={65535}
                  error={field.state.meta.errors?.[0]}
                />
              )} />

              <form.Field name="email.useTls" children={(field: { state: { value: boolean | undefined; meta: { errors?: string[] } }; handleChange: (v: boolean) => void }) => (
                <SwitchField
                  label="Use TLS"
                  checked={field.state.value ?? true}
                  onChange={field.handleChange}
                  description="Use secure TLS connection"
                />
              )} />
            </div>

            <form.Field name="email.username" children={(field: { state: { value: string | undefined; meta: { errors?: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Username"
                value={field.state.value || ''}
                onChange={field.handleChange}
                description="Email address or username for authentication"
                error={field.state.meta.errors?.[0]}
                required
                placeholder="documents@example.com"
              />
            )} />

            <form.Field name="email.folder" children={(field: { state: { value: string | undefined; meta: { errors?: string[] } }; handleChange: (v: string) => void }) => (
              <TextField
                label="Folder"
                value={field.state.value || 'INBOX'}
                onChange={field.handleChange}
                description="IMAP folder to scan for emails"
                placeholder="INBOX"
              />
            )} />
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* ── Authentication ── */}
      <AccordionItem value="auth">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Shield className="h-4 w-4" />
            Authentication
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            {emailHost?.toLowerCase().includes('gmail.com') && (
              <Alert variant="info">
                <Info className="h-4 w-4" />
                <AlertTitle>Gmail App Password Required</AlertTitle>
                <AlertDescription className="space-y-2">
                  <p>Gmail requires an App Password for third-party email access.</p>
                  <ol className="list-decimal list-inside text-sm space-y-1">
                    <li>Enable 2-Step Verification on your Google Account</li>
                    <li>Go to{' '}
                      <button
                        type="button"
                        onClick={() => open('https://myaccount.google.com/apppasswords')}
                        className="text-primary underline hover:no-underline"
                      >
                        Google App Passwords
                      </button>
                    </li>
                    <li>Create a new App Password for "Mail"</li>
                    <li>Copy the 16-character password below</li>
                  </ol>
                </AlertDescription>
              </Alert>
            )}

            <form.Field name="email.auth.type" children={(field: { state: { value: EmailAuthType; meta: { errors?: string[] } }; handleChange: (v: EmailAuthType) => void }) => (
              <SelectField
                label="Authentication Type"
                value={field.state.value}
                onChange={(v: string) => handleAuthTypeChange(v)}
                options={[
                  { value: 'password', label: 'Password' },
                  { value: 'oauth2', label: 'OAuth2' },
                ]}
                description="Choose authentication method"
                error={field.state.meta.errors?.[0]}
              />
            )} />

            {authType === 'password' && (
              <form.Field name="email.auth.passwordFile" children={(passwordFileField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                <form.Field name="email.auth.passwordEnvVar" children={(passwordEnvVarField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                  <SecretField
                    label="Password"
                    sourceName={sourceName}
                    secretType="password"
                    filePath={passwordFileField.state.value}
                    envVar={passwordEnvVarField.state.value}
                    onFilePathChange={(v) => {
                      passwordFileField.handleChange(v)
                      if (v !== undefined) passwordEnvVarField.handleChange(undefined)
                      form.setFieldValue('email.auth.passwordInsecure', undefined)
                    }}
                    onEnvVarChange={(v) => {
                      passwordEnvVarField.handleChange(v)
                      if (v !== undefined) passwordFileField.handleChange(undefined)
                      form.setFieldValue('email.auth.passwordInsecure', undefined)
                    }}
                    description="Your email password or app-specific password"
                    required
                  />
                )} />
              )} />
            )}

            {authType === 'oauth2' && (
              <>
                <form.Field name="email.auth.oauth2.provider" children={(field: { state: { value: OAuth2Provider | undefined; meta: { errors?: string[] } }; handleChange: (v: OAuth2Provider) => void }) => (
                  <SelectField
                    label="Provider"
                    value={field.state.value || 'gmail'}
                    onChange={(v: string) => field.handleChange(v as OAuth2Provider)}
                    options={[
                      { value: 'gmail', label: 'Gmail' },
                      { value: 'outlook', label: 'Outlook' },
                      { value: 'custom', label: 'Custom' },
                    ]}
                    description="OAuth2 provider"
                  />
                )} />

                {oauth2Provider === 'custom' && (
                  <form.Field name="email.auth.oauth2.tokenUrl" children={(field: { state: { value: string | undefined; meta: { errors?: string[] } }; handleChange: (v: string) => void }) => (
                    <TextField
                      label="Token URL"
                      value={field.state.value || ''}
                      onChange={field.handleChange}
                      description="OAuth2 token endpoint URL"
                      error={field.state.meta.errors?.[0]}
                      required
                      placeholder="https://oauth2.example.com/token"
                      mono
                    />
                  )} />
                )}

                <form.Field name="email.auth.oauth2.clientIdFile" children={(fileField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                  <form.Field name="email.auth.oauth2.clientIdEnvVar" children={(envField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                    <SecretField
                      label="Client ID"
                      sourceName={sourceName}
                      secretType="client_id"
                      filePath={fileField.state.value}
                      envVar={envField.state.value}
                      onFilePathChange={(v) => {
                        fileField.handleChange(v)
                        if (v !== undefined) envField.handleChange(undefined)
                      }}
                      onEnvVarChange={(v) => {
                        envField.handleChange(v)
                        if (v !== undefined) fileField.handleChange(undefined)
                      }}
                      description="OAuth2 client ID"
                      required
                    />
                  )} />
                )} />

                <form.Field name="email.auth.oauth2.clientSecretFile" children={(fileField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                  <form.Field name="email.auth.oauth2.clientSecretEnvVar" children={(envField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                    <SecretField
                      label="Client Secret"
                      sourceName={sourceName}
                      secretType="client_secret"
                      filePath={fileField.state.value}
                      envVar={envField.state.value}
                      onFilePathChange={(v) => {
                        fileField.handleChange(v)
                        if (v !== undefined) envField.handleChange(undefined)
                      }}
                      onEnvVarChange={(v) => {
                        envField.handleChange(v)
                        if (v !== undefined) fileField.handleChange(undefined)
                      }}
                      description="OAuth2 client secret"
                      required
                    />
                  )} />
                )} />

                <form.Field name="email.auth.oauth2.refreshTokenFile" children={(fileField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                  <form.Field name="email.auth.oauth2.refreshTokenEnvVar" children={(envField: { state: { value: string | undefined }; handleChange: (v: string | undefined) => void }) => (
                    <SecretField
                      label="Refresh Token"
                      sourceName={sourceName}
                      secretType="refresh_token"
                      filePath={fileField.state.value}
                      envVar={envField.state.value}
                      onFilePathChange={(v) => {
                        fileField.handleChange(v)
                        if (v !== undefined) envField.handleChange(undefined)
                      }}
                      onEnvVarChange={(v) => {
                        envField.handleChange(v)
                        if (v !== undefined) fileField.handleChange(undefined)
                      }}
                      description="OAuth2 refresh token"
                      required
                    />
                  )} />
                )} />
              </>
            )}
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* ── Filters ── */}
      <AccordionItem value="filters">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Filter className="h-4 w-4" />
            Attachment Filters
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="email.mimeFilters.include" children={(field: { state: { value: string[] | undefined; meta: { errors?: string[] } }; handleChange: (v: string[]) => void }) => (
              <ArrayField
                label="Include MIME Types"
                values={field.state.value || []}
                onChange={field.handleChange}
                description="MIME types to include (e.g., application/pdf, image/*)"
                placeholder="application/pdf"
                addLabel="Add MIME Type"
                mono
              />
            )} />

            <form.Field name="email.mimeFilters.exclude" children={(field: { state: { value: string[] | undefined; meta: { errors?: string[] } }; handleChange: (v: string[]) => void }) => (
              <ArrayField
                label="Exclude MIME Types"
                values={field.state.value || []}
                onChange={field.handleChange}
                description="MIME types to exclude"
                placeholder="application/zip"
                addLabel="Add MIME Type"
                mono
              />
            )} />

            <form.Field name="email.mimeFilters.filenameInclude" children={(field: { state: { value: string[] | undefined; meta: { errors?: string[] } }; handleChange: (v: string[]) => void }) => (
              <ArrayField
                label="Include Filename Patterns"
                values={field.state.value || []}
                onChange={field.handleChange}
                description="Glob patterns to match filenames to include"
                placeholder="invoice*.pdf"
                addLabel="Add Pattern"
                mono
              />
            )} />

            <form.Field name="email.mimeFilters.filenameExclude" children={(field: { state: { value: string[] | undefined; meta: { errors?: string[] } }; handleChange: (v: string[]) => void }) => (
              <ArrayField
                label="Exclude Filename Patterns"
                values={field.state.value || []}
                onChange={field.handleChange}
                description="Glob patterns to match filenames to exclude"
                placeholder="*.sig"
                addLabel="Add Pattern"
                mono
              />
            )} />

            <div className="grid grid-cols-2 gap-4">
              <form.Field name="email.minAttachmentSize" children={(field: { state: { value: number | undefined; meta: { errors?: string[] } }; handleChange: (v: number) => void }) => (
                <NumberField
                  label="Min Attachment Size (bytes)"
                  value={field.state.value ?? 0}
                  onChange={field.handleChange}
                  description="Minimum file size to process"
                  min={0}
                  error={
                    isMinGreaterThanMax
                      ? 'Min size must be less than or equal to max size'
                      : field.state.meta.errors?.[0]
                  }
                />
              )} />

              <form.Field name="email.maxAttachmentSize" children={(field: { state: { value: number | undefined; meta: { errors?: string[] } }; handleChange: (v: number) => void }) => (
                <NumberField
                  label="Max Attachment Size (bytes)"
                  value={field.state.value ?? 52428800}
                  onChange={field.handleChange}
                  description="Maximum file size to process (default: 50MB)"
                  min={0}
                  error={
                    isMinGreaterThanMax
                      ? 'Max size must be greater than or equal to min size'
                      : field.state.meta.errors?.[0]
                  }
                />
              )} />
            </div>
          </div>
        </AccordionContent>
      </AccordionItem>

      {/* ── Processing ── */}
      <AccordionItem value="processing">
        <AccordionTrigger className="hover:no-underline">
          <div className="flex items-center gap-2">
            <Clock className="h-4 w-4" />
            Processing Settings
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <div className="space-y-4 pt-4">
            <form.Field name="email.sinceDate" children={(field: { state: { value: string | undefined; meta: { errors?: string[] } }; handleChange: (v: string | undefined) => void }) => (
              <DateField
                label="Since Date"
                value={field.state.value || ''}
                onChange={(v: string) => field.handleChange(v || undefined)}
                description="Only process emails received after this date"
              />
            )} />

            <div className="grid grid-cols-2 gap-4">
              <form.Field name="email.pollInterval" children={(field: { state: { value: number | undefined; meta: { errors?: string[] } }; handleChange: (v: number) => void }) => (
                <NumberField
                  label="Poll Interval (seconds)"
                  value={field.state.value ?? 300}
                  onChange={field.handleChange}
                  description="How often to check for new emails"
                  min={60}
                  max={86400}
                  error={field.state.meta.errors?.[0]}
                />
              )} />

              <form.Field name="email.batchSize" children={(field: { state: { value: number | undefined; meta: { errors?: string[] } }; handleChange: (v: number) => void }) => (
                <NumberField
                  label="Batch Size"
                  value={field.state.value ?? 50}
                  onChange={field.handleChange}
                  description="Max emails to process per batch"
                  min={1}
                  max={1000}
                  error={field.state.meta.errors?.[0]}
                />
              )} />
            </div>
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
    </div>
  )
}

// ---------------------------------------------------------------------------
// ImportSourceForm – main export
// ---------------------------------------------------------------------------

export function ImportSourceForm({
  form,
  isNew,
  name,
  onNameChange,
}: ImportSourceFormProps) {
  const sourceType: ImportSourceSpec['type'] = useStore(form.store, (state) => state.values.type)

  const handleTypeChange = (newType: ImportSourceType) => {
    if (newType === form.getFieldValue('type')) return
    form.setFieldValue('type', newType)
    if (newType === 'local') {
      form.setFieldValue('local', {
        path: '',
        recursive: false,
        filters: { include: ['*.pdf', '*.png', '*.jpg'], exclude: ['*.tmp', '.*'] },
        pollInterval: 60,
      })
      form.setFieldValue('email', undefined)
    } else {
      form.setFieldValue('email', createDefaultEmailSourceConfig())
      form.setFieldValue('local', undefined)
    }
  }

  return (
    <div className="space-y-6">
      {isNew && onNameChange && (
        <TextField
          label="Source Name"
          value={name || ''}
          onChange={onNameChange}
          description="Unique identifier for this import source"
          required
          placeholder="local-documents"
        />
      )}

      <form.Field name="enabled" children={(field: { state: { value: boolean; meta: { errors?: string[] } }; handleChange: (v: boolean) => void }) => (
        <SwitchField
          label="Enabled"
          checked={field.state.value}
          onChange={field.handleChange}
          description="Enable or disable this import source"
        />
      )} />

      <SourceTypeSelector value={sourceType} onChange={handleTypeChange} />

      {sourceType === 'local' && <LocalSourceSection form={form} />}
      {sourceType === 'email' && (
        <EmailSourceSection form={form} isNew={isNew} name={name} />
      )}
    </div>
  )
}
