import { z } from 'zod'

// ============================================
// Shared / Common Schemas
// ============================================

export const outputSettingsSchema = z.object({
  directory: z.string().min(1, 'Directory is required'),
  filename: z.string().min(1, 'Filename is required'),
})

export type OutputSettings = z.infer<typeof outputSettingsSchema>

// ============================================
// Settings Resource Schema
// ============================================

export const ocrSettingsSchema = z.object({
  enabled: z.boolean(),
  languages: z.array(z.string()).min(1, 'At least one language is required'),
  dpi: z.number().min(72).max(600),
})

export type OcrSettings = z.infer<typeof ocrSettingsSchema>

export const gitAuthSettingsSchema = z.object({
  type: z.enum(['none', 'token', 'ssh-key']),
  tokenEnvVar: z.string().optional().default(''),
  // Renamed to match backend (token_insecure in Rust)
  tokenInsecure: z.string().optional(),
  // Renamed to match backend (token_file in Rust)
  tokenFile: z.string().optional(),
  sshKeyPath: z.string().optional().default(''),
})

export type GitAuthSettings = z.infer<typeof gitAuthSettingsSchema>

export const gitSettingsSchema = z.object({
  enabled: z.boolean(),
  repository: z.string(),
  branch: z.string().default('main'),
  syncInterval: z.number().min(0).default(300),
  auth: gitAuthSettingsSchema,
  userName: z.string().default('Paporg'),
  userEmail: z.string().default('paporg@localhost'),
})

export type GitSettings = z.infer<typeof gitSettingsSchema>

export const defaultOutputSettingsSchema = z.object({
  output: outputSettingsSchema,
})

export type DefaultOutputSettings = z.infer<typeof defaultOutputSettingsSchema>

export const settingsSpecSchema = z.object({
  inputDirectory: z.string().min(1, 'Input directory is required'),
  outputDirectory: z.string().min(1, 'Output directory is required'),
  workerCount: z.number().min(1).max(32).default(4),
  ocr: ocrSettingsSchema,
  defaults: defaultOutputSettingsSchema,
  git: gitSettingsSchema,
})

export type SettingsSpec = z.infer<typeof settingsSpecSchema>

// ============================================
// Variable Resource Schema
// ============================================

export const variableTransformSchema = z.enum(['slugify', 'uppercase', 'lowercase', 'trim']).optional()

export type VariableTransform = z.infer<typeof variableTransformSchema>

export const variableSpecSchema = z.object({
  pattern: z.string().min(1, 'Pattern is required').refine(
    (val) => validateRegexPattern(val).valid,
    { message: 'Invalid regex pattern' }
  ),
  transform: variableTransformSchema,
  default: z.string().optional(),
})

export type VariableSpec = z.infer<typeof variableSpecSchema>

// ============================================
// Rule Resource Schema (with recursive match)
// ============================================

// Base match condition types
export const simpleMatchSchema = z.object({
  contains: z.string().optional(),
  containsAny: z.array(z.string()).optional(),
  containsAll: z.array(z.string()).optional(),
  pattern: z.string().optional(),
  caseSensitive: z.boolean().optional(),
}).refine(
  (data) => {
    const keys = Object.keys(data).filter(k => k !== 'caseSensitive' && data[k as keyof typeof data] !== undefined)
    return keys.length === 1
  },
  { message: 'Exactly one match type must be specified' }
)

// Recursive match condition schema using z.lazy
export type MatchCondition =
  | { contains: string; caseSensitive?: boolean }
  | { containsAny: string[]; caseSensitive?: boolean }
  | { containsAll: string[]; caseSensitive?: boolean }
  | { pattern: string; caseSensitive?: boolean }
  | { all: MatchCondition[]; caseSensitive?: boolean }
  | { any: MatchCondition[]; caseSensitive?: boolean }
  | { not: MatchCondition; caseSensitive?: boolean }

export const matchConditionSchema: z.ZodType<MatchCondition> = z.lazy(() =>
  z.union([
    z.object({ contains: z.string(), caseSensitive: z.boolean().optional() }),
    z.object({ containsAny: z.array(z.string()).min(1), caseSensitive: z.boolean().optional() }),
    z.object({ containsAll: z.array(z.string()).min(1), caseSensitive: z.boolean().optional() }),
    z.object({ pattern: z.string(), caseSensitive: z.boolean().optional() }),
    z.object({ all: z.array(matchConditionSchema).min(1), caseSensitive: z.boolean().optional() }),
    z.object({ any: z.array(matchConditionSchema).min(1), caseSensitive: z.boolean().optional() }),
    z.object({ not: matchConditionSchema, caseSensitive: z.boolean().optional() }),
  ])
)

export const symlinkSettingsSchema = z.object({
  target: z.string().min(1, 'Target is required'),
})

export type SymlinkSettings = z.infer<typeof symlinkSettingsSchema>

export const ruleSpecSchema = z.object({
  priority: z.number().int().default(0),
  category: z.string().min(1, 'Category is required'),
  match: matchConditionSchema,
  output: outputSettingsSchema,
  symlinks: z.array(symlinkSettingsSchema).optional().default([]),
})

export type RuleSpec = z.infer<typeof ruleSpecSchema>

// ============================================
// Full Resource Schemas (with metadata)
// ============================================

export const objectMetaSchema = z.object({
  name: z.string()
    .min(1, 'Name is required')
    .regex(/^[a-zA-Z_][a-zA-Z0-9_-]*$/, 'Name must start with a letter or underscore, and contain only letters, numbers, underscores, and hyphens'),
  labels: z.record(z.string()).optional().default({}),
  annotations: z.record(z.string()).optional().default({}),
})

export type ObjectMeta = z.infer<typeof objectMetaSchema>

export const settingsResourceSchema = z.object({
  apiVersion: z.literal('paporg.io/v1'),
  kind: z.literal('Settings'),
  metadata: objectMetaSchema,
  spec: settingsSpecSchema,
})

export type SettingsResource = z.infer<typeof settingsResourceSchema>

export const variableResourceSchema = z.object({
  apiVersion: z.literal('paporg.io/v1'),
  kind: z.literal('Variable'),
  metadata: objectMetaSchema,
  spec: variableSpecSchema,
})

export type VariableResource = z.infer<typeof variableResourceSchema>

export const ruleResourceSchema = z.object({
  apiVersion: z.literal('paporg.io/v1'),
  kind: z.literal('Rule'),
  metadata: objectMetaSchema,
  spec: ruleSpecSchema,
})

export type RuleResource = z.infer<typeof ruleResourceSchema>

// ============================================
// ImportSource Resource Schema
// ============================================

export const fileFiltersSchema = z.object({
  include: z.array(z.string()).default(['*']),
  exclude: z.array(z.string()).default([]),
})

export type FileFilters = z.infer<typeof fileFiltersSchema>

export const localSourceConfigSchema = z.object({
  path: z.string().min(1, 'Path is required'),
  recursive: z.boolean().default(false),
  filters: fileFiltersSchema.default({ include: ['*'], exclude: [] }),
  pollInterval: z.number().int().min(1).max(86400).default(60),
})

export type LocalSourceConfig = z.infer<typeof localSourceConfigSchema>

// ============================================
// Email Source Schemas
// ============================================

export const emailAuthTypeSchema = z.enum(['password', 'oauth2'])

export type EmailAuthType = z.infer<typeof emailAuthTypeSchema>

export const oauth2ProviderSchema = z.enum(['gmail', 'outlook', 'custom'])

export type OAuth2Provider = z.infer<typeof oauth2ProviderSchema>

export const oauth2SettingsSchema = z.object({
  provider: oauth2ProviderSchema.optional(),
  // Environment variable references
  clientIdEnvVar: z.string().optional(),
  clientSecretEnvVar: z.string().optional(),
  refreshTokenEnvVar: z.string().optional(),
  // Direct values (for local development)
  clientId: z.string().optional(),
  clientSecret: z.string().optional(),
  refreshToken: z.string().optional(),
  // File references (for Docker secrets)
  clientIdFile: z.string().optional(),
  clientSecretFile: z.string().optional(),
  refreshTokenFile: z.string().optional(),
  tokenUrl: z.string().optional(),
})

export type OAuth2Settings = z.infer<typeof oauth2SettingsSchema>

export const emailAuthSettingsSchema = z.object({
  type: emailAuthTypeSchema,
  // Environment variable reference
  passwordEnvVar: z.string().optional(),
  // Direct value (for local development) - WARNING: insecure
  passwordInsecure: z.string().optional(),
  // File reference (for Docker secrets)
  passwordFile: z.string().optional(),
  oauth2: oauth2SettingsSchema.optional(),
}).superRefine((data, ctx) => {
  if (data.type === 'password') {
    // Accept any of: passwordInsecure, passwordFile, or passwordEnvVar
    const hasPassword =
      (data.passwordInsecure !== undefined && data.passwordInsecure.length > 0) ||
      (data.passwordFile !== undefined && data.passwordFile.length > 0) ||
      (data.passwordEnvVar !== undefined && data.passwordEnvVar.length > 0)
    if (!hasPassword) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'Password auth requires passwordInsecure, passwordFile, or passwordEnvVar.',
        path: ['passwordEnvVar'], // Point to a relevant field, not type
      })
    }
  }
  if (data.type === 'oauth2') {
    if (data.oauth2 === undefined) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'OAuth2 auth requires oauth2 settings.',
        path: ['oauth2'],
      })
    }
  }
})

export type EmailAuthSettings = z.infer<typeof emailAuthSettingsSchema>

export const attachmentFiltersSchema = z.object({
  include: z.array(z.string()).default([]),
  exclude: z.array(z.string()).default([]),
  filenameInclude: z.array(z.string()).default([]),
  filenameExclude: z.array(z.string()).default([]),
})

export type AttachmentFilters = z.infer<typeof attachmentFiltersSchema>

export const emailSourceConfigSchema = z.object({
  host: z.string().min(1, 'IMAP host is required'),
  port: z.number().int().min(1).max(65535).default(993),
  useTls: z.boolean().default(true),
  username: z.string().min(1, 'Username is required'),
  auth: emailAuthSettingsSchema,
  folder: z.string().default('INBOX'),
  sinceDate: z.string().optional(),
  mimeFilters: attachmentFiltersSchema.default({
    include: [],
    exclude: [],
    filenameInclude: [],
    filenameExclude: [],
  }),
  minAttachmentSize: z.number().int().min(0).default(0),
  maxAttachmentSize: z.number().int().min(0).default(52428800),
  pollInterval: z.number().int().min(60).max(86400).default(300),
  batchSize: z.number().int().min(1).max(1000).default(50),
}).refine(
  (data) => data.minAttachmentSize <= data.maxAttachmentSize,
  {
    message: 'Minimum attachment size must be less than or equal to maximum attachment size',
    path: ['minAttachmentSize'],
  }
)

export type EmailSourceConfig = z.infer<typeof emailSourceConfigSchema>

export const importSourceTypeSchema = z.enum(['local', 'email'])

export type ImportSourceType = z.infer<typeof importSourceTypeSchema>

export const importSourceSpecSchema = z.object({
  type: importSourceTypeSchema,
  enabled: z.boolean().default(true),
  local: localSourceConfigSchema.optional(),
  email: emailSourceConfigSchema.optional(),
}).refine(
  (data) => {
    if (data.type === 'local') {
      return data.local !== undefined
    }
    if (data.type === 'email') {
      return data.email !== undefined
    }
    return true
  },
  {
    message: 'Configuration is required for the selected source type',
    path: ['type'],
  }
)

export type ImportSourceSpec = z.infer<typeof importSourceSpecSchema>

export const importSourceResourceSchema = z.object({
  apiVersion: z.literal('paporg.io/v1'),
  kind: z.literal('ImportSource'),
  metadata: objectMetaSchema,
  spec: importSourceSpecSchema,
})

export type ImportSourceResource = z.infer<typeof importSourceResourceSchema>

// ============================================
// Helper functions for creating defaults
// ============================================

export function createDefaultSettingsSpec(): SettingsSpec {
  return {
    inputDirectory: '/data/inbox',
    outputDirectory: '/data/documents',
    workerCount: 4,
    ocr: {
      enabled: true,
      languages: ['eng'],
      dpi: 300,
    },
    defaults: {
      output: {
        directory: '$y/unsorted',
        filename: '$original_$timestamp',
      },
    },
    git: {
      enabled: false,
      repository: '',
      branch: 'main',
      syncInterval: 300,
      auth: {
        type: 'none',
        tokenEnvVar: '',
        sshKeyPath: '',
      },
      userName: 'Paporg',
      userEmail: 'paporg@localhost',
    },
  }
}

export function createDefaultVariableSpec(): VariableSpec {
  return {
    pattern: '(?P<value>\\w+)',
    transform: undefined,
    default: undefined,
  }
}

export function createDefaultRuleSpec(): RuleSpec {
  return {
    priority: 0,
    category: '',
    match: { contains: '' },
    output: {
      directory: '',
      filename: '$original',
    },
    symlinks: [],
  }
}

export function createDefaultEmailSourceConfig(): EmailSourceConfig {
  // Return a full config with password auth as default
  // The form will prompt the user to fill in the credentials
  return {
    host: '',
    port: 993,
    useTls: true,
    username: '',
    auth: {
      type: 'password',
      // Note: passwordEnvVar is undefined - user needs to configure credentials
      // This is valid for the form but will fail validation on submit if not filled
    },
    folder: 'INBOX',
    sinceDate: undefined,
    mimeFilters: {
      include: ['application/pdf', 'image/*'],
      exclude: [],
      filenameInclude: [],
      filenameExclude: [],
    },
    minAttachmentSize: 0,
    maxAttachmentSize: 52428800,
    pollInterval: 300,
    batchSize: 50,
  }
}

export function createDefaultImportSourceSpec(type: ImportSourceType = 'local'): ImportSourceSpec {
  if (type === 'email') {
    return {
      type: 'email',
      enabled: true,
      email: createDefaultEmailSourceConfig(),
    }
  }
  return {
    type: 'local',
    enabled: true,
    local: {
      path: '',
      recursive: false,
      filters: { include: ['*.pdf', '*.png', '*.jpg'], exclude: ['*.tmp', '.*'] },
      pollInterval: 60,
    },
  }
}

export function createDefaultMatchCondition(): MatchCondition {
  return { contains: '' }
}

// ============================================
// Match condition type helpers
// ============================================

export type MatchConditionType = 'contains' | 'containsAny' | 'containsAll' | 'pattern' | 'all' | 'any' | 'not'

export function getMatchConditionType(condition: MatchCondition): MatchConditionType {
  if ('contains' in condition) return 'contains'
  if ('containsAny' in condition) return 'containsAny'
  if ('containsAll' in condition) return 'containsAll'
  if ('pattern' in condition) return 'pattern'
  if ('all' in condition) return 'all'
  if ('any' in condition) return 'any'
  if ('not' in condition) return 'not'
  return 'contains'
}

export function createMatchConditionOfType(type: MatchConditionType): MatchCondition {
  switch (type) {
    case 'contains':
      return { contains: '' }
    case 'containsAny':
      return { containsAny: [''] }
    case 'containsAll':
      return { containsAll: [''] }
    case 'pattern':
      return { pattern: '' }
    case 'all':
      return { all: [{ contains: '' }] }
    case 'any':
      return { any: [{ contains: '' }] }
    case 'not':
      return { not: { contains: '' } }
  }
}

export function isSimpleMatch(condition: MatchCondition): boolean {
  return 'contains' in condition || 'containsAny' in condition || 'containsAll' in condition || 'pattern' in condition
}

export function isCompoundMatch(condition: MatchCondition): boolean {
  return 'all' in condition || 'any' in condition || 'not' in condition
}

// ============================================
// Validation helpers
// ============================================

/** Convert Rust-style named groups `(?P<name>...)` to JS-style `(?<name>...)` for validation. */
function toJsRegex(pattern: string): string {
  return pattern.replace(/\(\?P</g, '(?<')
}

export function validateRegexPattern(pattern: string): { valid: boolean; error?: string } {
  try {
    new RegExp(toJsRegex(pattern))
    return { valid: true }
  } catch (e) {
    return { valid: false, error: e instanceof Error ? e.message : 'Invalid regex' }
  }
}
