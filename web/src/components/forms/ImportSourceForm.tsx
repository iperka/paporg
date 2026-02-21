import { TextField, NumberField, SwitchField, ArrayField, SelectField, SecretField, DateField, PathField } from '@/components/form'
import {
  type ImportSourceSpec,
  type ImportSourceType,
  type EmailSourceConfig,
  createDefaultImportSourceSpec,
  createDefaultEmailSourceConfig,
} from '@/schemas/resources'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { FolderOpen, Server, Shield, Filter, Clock, Info } from 'lucide-react'

interface ImportSourceFormProps {
  value: ImportSourceSpec
  onChange: (value: ImportSourceSpec) => void
  errors?: Record<string, string>
  isNew?: boolean
  name?: string
  onNameChange?: (name: string) => void
}

export function ImportSourceForm({
  value,
  onChange,
  errors = {},
  isNew,
  name,
  onNameChange,
}: ImportSourceFormProps) {
  const updateField = <K extends keyof ImportSourceSpec>(
    field: K,
    fieldValue: ImportSourceSpec[K]
  ) => {
    onChange({ ...value, [field]: fieldValue })
  }

  const updateLocalConfig = <K extends keyof NonNullable<ImportSourceSpec['local']>>(
    field: K,
    fieldValue: NonNullable<ImportSourceSpec['local']>[K]
  ) => {
    const local = value.local ?? {
      path: '',
      recursive: false,
      filters: { include: ['*'], exclude: [] },
      pollInterval: 60,
    }
    onChange({
      ...value,
      local: { ...local, [field]: fieldValue },
    })
  }

  const updateFilters = <K extends keyof NonNullable<ImportSourceSpec['local']>['filters']>(
    field: K,
    fieldValue: NonNullable<ImportSourceSpec['local']>['filters'][K]
  ) => {
    const local = value.local ?? {
      path: '',
      recursive: false,
      filters: { include: ['*'], exclude: [] },
      pollInterval: 60,
    }
    onChange({
      ...value,
      local: {
        ...local,
        filters: { ...local.filters, [field]: fieldValue },
      },
    })
  }

  const handleTypeChange = (newType: string) => {
    // Validate that the new type is a valid ImportSourceType
    if (newType !== 'local' && newType !== 'email') {
      console.warn(`Invalid import source type: ${newType}. Expected 'local' or 'email'.`)
      return
    }
    const type: ImportSourceType = newType
    const newSpec = createDefaultImportSourceSpec(type)
    onChange({
      ...newSpec,
      enabled: value.enabled,
    })
  }

  const updateEmailConfig = <K extends keyof EmailSourceConfig>(
    field: K,
    fieldValue: EmailSourceConfig[K]
  ) => {
    const email = value.email ?? createDefaultEmailSourceConfig()
    onChange({
      ...value,
      email: { ...email, [field]: fieldValue },
    })
  }

  // Type for valid auth fields that can be updated
  type EmailAuthField = 'passwordEnvVar' | 'passwordFile' | 'passwordInsecure'

  // All mutually exclusive password auth fields
  const PASSWORD_AUTH_FIELDS: EmailAuthField[] = ['passwordEnvVar', 'passwordFile', 'passwordInsecure']

  const updateAuthField = (field: EmailAuthField, fieldValue: string | undefined) => {
    const email = value.email ?? createDefaultEmailSourceConfig()
    // Clear all competing auth fields when setting one
    const updates: Partial<Record<EmailAuthField, string | undefined>> = {}
    for (const authField of PASSWORD_AUTH_FIELDS) {
      if (authField === field) {
        updates[authField] = fieldValue
      } else {
        updates[authField] = undefined
      }
    }
    onChange({
      ...value,
      email: {
        ...email,
        auth: {
          ...email.auth,
          ...updates,
        },
      },
    })
  }

  const updateMimeFilters = <K extends keyof EmailSourceConfig['mimeFilters']>(
    field: K,
    fieldValue: EmailSourceConfig['mimeFilters'][K]
  ) => {
    const email = value.email ?? createDefaultEmailSourceConfig()
    onChange({
      ...value,
      email: {
        ...email,
        mimeFilters: { ...email.mimeFilters, [field]: fieldValue },
      },
    })
  }

  // Compute attachment size validation once
  const minAttachmentSize = value.email?.minAttachmentSize ?? 0
  const maxAttachmentSize = value.email?.maxAttachmentSize ?? 52428800
  const isMinGreaterThanMax = minAttachmentSize > maxAttachmentSize

  return (
    <div className="space-y-6">
      {isNew && onNameChange && (
        <TextField
          label="Source Name"
          value={name || ''}
          onChange={onNameChange}
          description="Unique identifier for this import source"
          error={errors['name']}
          required
          placeholder="local-documents"
        />
      )}

      <SwitchField
        label="Enabled"
        checked={value.enabled}
        onChange={(checked) => updateField('enabled', checked)}
        description="Enable or disable this import source"
      />

      <SelectField
        label="Source Type"
        value={value.type}
        onChange={handleTypeChange}
        options={[
          { value: 'local', label: 'Local Directory' },
          { value: 'email', label: 'Email (IMAP)' },
        ]}
        description="Choose where to import documents from"
      />

      {/* Local source configuration */}
      {value.type === 'local' && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-lg">
              <FolderOpen className="h-5 w-5" />
              Local Directory Settings
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <PathField
              label="Path"
              value={value.local?.path || ''}
              onChange={(v) => updateLocalConfig('path', v)}
              description="Absolute path to the directory to watch for new files"
              error={errors['local.path']}
              required
              placeholder="/Users/me/Downloads"
              mode="folder"
            />

            <SwitchField
              label="Recursive"
              checked={value.local?.recursive ?? false}
              onChange={(checked) => updateLocalConfig('recursive', checked)}
              description="Watch subdirectories for new files"
            />

            <NumberField
              label="Poll Interval"
              value={value.local?.pollInterval ?? 60}
              onChange={(v) => updateLocalConfig('pollInterval', v)}
              description="How often to check for new files (in seconds)"
              min={1}
              max={86400}
            />

            <ArrayField
              label="Include Patterns"
              values={value.local?.filters?.include || ['*']}
              onChange={(v) => updateFilters('include', v)}
              description="Glob patterns to match files to include (e.g., *.pdf, *.png)"
              placeholder="*.pdf"
              addLabel="Add Pattern"
              mono
              minItems={1}
            />

            <ArrayField
              label="Exclude Patterns"
              values={value.local?.filters?.exclude || []}
              onChange={(v) => updateFilters('exclude', v)}
              description="Glob patterns to match files to exclude (e.g., *.tmp, .*)"
              placeholder="*.tmp"
              addLabel="Add Pattern"
              mono
            />
          </CardContent>
        </Card>
      )}

      {/* Email source configuration */}
      {value.type === 'email' && (
        <>
          {/* IMAP Server Settings */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-lg">
                <Server className="h-5 w-5" />
                IMAP Server Settings
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <TextField
                label="Host"
                value={value.email?.host || ''}
                onChange={(v) => updateEmailConfig('host', v)}
                description="IMAP server hostname (e.g., imap.gmail.com)"
                error={errors['email.host']}
                required
                placeholder="imap.gmail.com"
              />

              <div className="grid grid-cols-2 gap-4">
                <NumberField
                  label="Port"
                  value={value.email?.port ?? 993}
                  onChange={(v) => updateEmailConfig('port', v)}
                  description="IMAP server port"
                  min={1}
                  max={65535}
                />

                <SwitchField
                  label="Use TLS"
                  checked={value.email?.useTls ?? true}
                  onChange={(checked) => updateEmailConfig('useTls', checked)}
                  description="Use secure TLS connection"
                />
              </div>

              <TextField
                label="Username"
                value={value.email?.username || ''}
                onChange={(v) => updateEmailConfig('username', v)}
                description="Email address or username for authentication"
                error={errors['email.username']}
                required
                placeholder="documents@example.com"
              />

              <TextField
                label="Folder"
                value={value.email?.folder || 'INBOX'}
                onChange={(v) => updateEmailConfig('folder', v)}
                description="IMAP folder to scan for emails"
                placeholder="INBOX"
              />
            </CardContent>
          </Card>

          {/* Authentication */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-lg">
                <Shield className="h-5 w-5" />
                Authentication
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {/* Show Gmail-specific instructions only when host is Gmail */}
              {value.email?.host?.toLowerCase().includes('gmail.com') && (
                <Alert variant="info">
                  <Info className="h-4 w-4" />
                  <AlertTitle>Gmail App Password Required</AlertTitle>
                  <AlertDescription className="space-y-2">
                    <p>Gmail requires an App Password for third-party email access.</p>
                    <ol className="list-decimal list-inside text-sm space-y-1">
                      <li>Enable 2-Step Verification on your Google Account</li>
                      <li>Go to{' '}
                        <a
                          href="https://myaccount.google.com/apppasswords"
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-primary underline hover:no-underline"
                        >
                          Google App Passwords
                        </a>
                      </li>
                      <li>Create a new App Password for "Mail"</li>
                      <li>Copy the 16-character password below</li>
                    </ol>
                  </AlertDescription>
                </Alert>
              )}

              <SecretField
                label="Password"
                sourceName={isNew ? (name || 'new-source') : (name ?? '')}
                secretType="password"
                filePath={value.email?.auth?.passwordFile}
                envVar={value.email?.auth?.passwordEnvVar}
                onFilePathChange={(v) => updateAuthField('passwordFile', v)}
                onEnvVarChange={(v) => updateAuthField('passwordEnvVar', v)}
                description="Your email password or app-specific password"
                error={errors['email.auth.passwordEnvVar'] || errors['email.auth.passwordFile']}
                required
              />
            </CardContent>
          </Card>

          {/* Attachment Filters */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-lg">
                <Filter className="h-5 w-5" />
                Attachment Filters
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <ArrayField
                label="Include MIME Types"
                values={value.email?.mimeFilters?.include || []}
                onChange={(v) => updateMimeFilters('include', v)}
                description="MIME types to include (e.g., application/pdf, image/*)"
                placeholder="application/pdf"
                addLabel="Add MIME Type"
                mono
              />

              <ArrayField
                label="Exclude MIME Types"
                values={value.email?.mimeFilters?.exclude || []}
                onChange={(v) => updateMimeFilters('exclude', v)}
                description="MIME types to exclude"
                placeholder="application/zip"
                addLabel="Add MIME Type"
                mono
              />

              <ArrayField
                label="Include Filename Patterns"
                values={value.email?.mimeFilters?.filenameInclude || []}
                onChange={(v) => updateMimeFilters('filenameInclude', v)}
                description="Glob patterns to match filenames to include"
                placeholder="invoice*.pdf"
                addLabel="Add Pattern"
                mono
              />

              <ArrayField
                label="Exclude Filename Patterns"
                values={value.email?.mimeFilters?.filenameExclude || []}
                onChange={(v) => updateMimeFilters('filenameExclude', v)}
                description="Glob patterns to match filenames to exclude"
                placeholder="*.sig"
                addLabel="Add Pattern"
                mono
              />

              <div className="grid grid-cols-2 gap-4">
                <NumberField
                  label="Min Attachment Size (bytes)"
                  value={minAttachmentSize}
                  onChange={(v) => updateEmailConfig('minAttachmentSize', v)}
                  description="Minimum file size to process"
                  min={0}
                  error={
                    isMinGreaterThanMax
                      ? 'Min size must be less than or equal to max size'
                      : errors['email.minAttachmentSize']
                  }
                />

                <NumberField
                  label="Max Attachment Size (bytes)"
                  value={maxAttachmentSize}
                  onChange={(v) => updateEmailConfig('maxAttachmentSize', v)}
                  description="Maximum file size to process (default: 50MB)"
                  min={0}
                  error={
                    isMinGreaterThanMax
                      ? 'Max size must be greater than or equal to min size'
                      : errors['email.maxAttachmentSize']
                  }
                />
              </div>
            </CardContent>
          </Card>

          {/* Processing Settings */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-lg">
                <Clock className="h-5 w-5" />
                Processing Settings
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <DateField
                label="Since Date"
                value={value.email?.sinceDate || ''}
                onChange={(v) => updateEmailConfig('sinceDate', v || undefined)}
                description="Only process emails received after this date"
              />

              <div className="grid grid-cols-2 gap-4">
                <NumberField
                  label="Poll Interval (seconds)"
                  value={value.email?.pollInterval ?? 300}
                  onChange={(v) => updateEmailConfig('pollInterval', v)}
                  description="How often to check for new emails"
                  min={60}
                  max={86400}
                />

                <NumberField
                  label="Batch Size"
                  value={value.email?.batchSize ?? 50}
                  onChange={(v) => updateEmailConfig('batchSize', v)}
                  description="Max emails to process per batch"
                  min={1}
                  max={1000}
                />
              </div>
            </CardContent>
          </Card>
        </>
      )}
    </div>
  )
}
