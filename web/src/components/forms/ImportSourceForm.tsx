import { TextField, NumberField, SwitchField, ArrayField, SelectField, SecretField, DateField, PathField } from '@/components/form'
import {
  type ImportSourceSpec,
  type ImportSourceType,
  createDefaultImportSourceSpec,
} from '@/schemas/resources'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { FolderOpen, Server, Shield, Filter, Clock, Info } from 'lucide-react'
import { useStore } from '@tanstack/react-form'
import type { FormInstance } from '@/lib/form-utils'

interface ImportSourceFormProps {
  form: FormInstance
  isNew?: boolean
  name?: string
  onNameChange?: (name: string) => void
}

export function ImportSourceForm({
  form,
  isNew,
  name,
  onNameChange,
}: ImportSourceFormProps) {
  // Subscribe to type field to conditionally render sections
  const sourceType: ImportSourceSpec['type'] = useStore(form.store, (state) => state.values.type)

  // Subscribe to email host for Gmail-specific instructions
  const emailHost: string | undefined = useStore(form.store, (state) => state.values.email?.host)

  // Subscribe to attachment sizes for cross-validation
  const minAttachmentSize: number = useStore(form.store, (state) => state.values.email?.minAttachmentSize) ?? 0
  const maxAttachmentSize: number = useStore(form.store, (state) => state.values.email?.maxAttachmentSize) ?? 52428800
  const isMinGreaterThanMax = minAttachmentSize > maxAttachmentSize

  // Handle type change: reset entire form to new type defaults, preserving 'enabled'
  const handleTypeChange = (newType: string) => {
    if (newType !== 'local' && newType !== 'email') {
      console.warn(`Invalid import source type: ${newType}. Expected 'local' or 'email'.`)
      return
    }
    const currentEnabled: boolean = form.getFieldValue('enabled')
    const newSpec = createDefaultImportSourceSpec(newType as ImportSourceType)
    form.reset({ ...newSpec, enabled: currentEnabled })
  }

  // Subscribe to auth fields for reactive SecretField props
  const passwordFile: string | undefined = useStore(form.store, (state) => state.values.email?.auth?.passwordFile)
  const passwordEnvVar: string | undefined = useStore(form.store, (state) => state.values.email?.auth?.passwordEnvVar)

  // All mutually exclusive password auth fields
  type EmailAuthField = 'passwordEnvVar' | 'passwordFile' | 'passwordInsecure'
  const PASSWORD_AUTH_FIELDS: EmailAuthField[] = ['passwordEnvVar', 'passwordFile', 'passwordInsecure']

  const updateAuthField = (field: EmailAuthField, fieldValue: string | undefined) => {
    // Clear all competing auth fields when setting one
    for (const authField of PASSWORD_AUTH_FIELDS) {
      if (authField === field) {
        form.setFieldValue(`email.auth.${authField}`, fieldValue)
      } else {
        form.setFieldValue(`email.auth.${authField}`, undefined)
      }
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

      <form.Field name="type" children={(field: { state: { value: ImportSourceType; meta: { errors?: string[] } }; handleChange: (v: ImportSourceType) => void }) => (
        <SelectField
          label="Source Type"
          value={field.state.value}
          onChange={(v: string) => {
            handleTypeChange(v)
          }}
          options={[
            { value: 'local', label: 'Local Directory' },
            { value: 'email', label: 'Email (IMAP)' },
          ]}
          description="Choose where to import documents from"
          error={field.state.meta.errors?.[0]}
        />
      )} />

      {/* Local source configuration */}
      {sourceType === 'local' && (
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
      )}

      {/* Email source configuration */}
      {sourceType === 'email' && (
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
              {emailHost?.toLowerCase().includes('gmail.com') && (
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
                filePath={passwordFile}
                envVar={passwordEnvVar}
                onFilePathChange={(v) => updateAuthField('passwordFile', v)}
                onEnvVarChange={(v) => updateAuthField('passwordEnvVar', v)}
                description="Your email password or app-specific password"
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
            </CardContent>
          </Card>
        </>
      )}
    </div>
  )
}
