// GitOps resource types following Kubernetes conventions

export const API_VERSION = 'paporg.io/v1'

export type ResourceKind = 'Settings' | 'Variable' | 'Rule' | 'ImportSource'

export interface ObjectMeta {
  name: string
  labels?: Record<string, string>
  annotations?: Record<string, string>
}

// Base resource type
export interface Resource<T> {
  apiVersion: string
  kind: ResourceKind
  metadata: ObjectMeta
  spec: T
}

// Settings Resource
export interface SettingsSpec {
  inputDirectory: string
  outputDirectory: string
  workerCount: number
  ocr: OcrSettings
  defaults: DefaultOutputSettings
  git: GitSettings
}

export interface OcrSettings {
  enabled: boolean
  languages: string[]
  dpi: number
}

export interface DefaultOutputSettings {
  output: OutputSettings
}

export interface GitSettings {
  enabled: boolean
  repository: string
  branch: string
  syncInterval: number
  auth: GitAuthSettings
}

export interface GitAuthSettings {
  type: 'none' | 'token' | 'ssh-key'
  tokenEnvVar: string
  sshKeyPath: string
}

export type SettingsResource = Resource<SettingsSpec>

// Variable Resource
export interface VariableSpec {
  pattern: string
  transform?: 'slugify' | 'uppercase' | 'lowercase' | 'trim'
  default?: string
}

export type VariableResource = Resource<VariableSpec>

// Rule Resource
export interface RuleSpec {
  priority: number
  category: string
  match: MatchCondition
  output: OutputSettings
  symlinks?: SymlinkSettings[]
}

export type MatchCondition = SimpleMatch | CompoundMatch

export interface SimpleMatch {
  contains?: string
  containsAny?: string[]
  containsAll?: string[]
  pattern?: string
}

export interface CompoundMatch {
  all?: MatchCondition[]
  any?: MatchCondition[]
  not?: MatchCondition
}

export interface OutputSettings {
  directory: string
  filename: string
}

export interface SymlinkSettings {
  target: string
}

export type RuleResource = Resource<RuleSpec>

// ImportSource Resource
export interface ImportSourceSpec {
  type: 'local' | 'email'
  enabled: boolean
  local?: LocalSourceConfig
  email?: EmailSourceConfig
}

export interface LocalSourceConfig {
  path: string
  recursive: boolean
  filters: FileFilters
  pollInterval: number
}

export interface FileFilters {
  include: string[]
  exclude: string[]
}

export interface EmailSourceConfig {
  host: string
  port: number
  useTls: boolean
  username: string
  auth: EmailAuthSettings
  folder: string
  sinceDate?: string
  mimeFilters: AttachmentFilters
  minAttachmentSize: number
  maxAttachmentSize: number
  pollInterval: number
  batchSize: number
}

export interface EmailAuthSettings {
  type: 'password' | 'oauth2'
  passwordEnvVar?: string
  passwordInsecure?: string
  passwordFile?: string
  oauth2?: OAuth2Settings
}

export interface OAuth2Settings {
  provider?: 'gmail' | 'outlook' | 'custom'
  clientIdEnvVar?: string
  clientSecretEnvVar?: string
  refreshTokenEnvVar?: string
  clientIdInsecure?: string
  clientSecretInsecure?: string
  refreshTokenInsecure?: string
  clientIdFile?: string
  clientSecretFile?: string
  refreshTokenFile?: string
  tokenUrl?: string
}

export interface AttachmentFilters {
  include: string[]
  exclude: string[]
  filenameInclude: string[]
  filenameExclude: string[]
}

export type ImportSourceResource = Resource<ImportSourceSpec>

// Any resource union type
export type AnyResource = SettingsResource | VariableResource | RuleResource | ImportSourceResource

// Re-export ApiResponse from shared module
export type { ApiResponse } from './api'

export interface ResourceListResponse {
  kind: string
  items: ResourceSummary[]
}

export interface ResourceSummary {
  name: string
  path: string
  labels?: Record<string, string>
}

export interface ResourceDetail {
  name: string
  path: string
  yaml: string
}

// File Tree types
export interface FileTreeNode {
  name: string
  path: string
  isDirectory: boolean
  children: FileTreeNode[]
  resource?: ResourceInfo
}

export interface ResourceInfo {
  kind: ResourceKind
  name: string
}

// Git types
export interface FileStatus {
  path: string
  status: 'staged' | 'modified' | 'untracked' | 'deleted' | 'added'
}

export interface GitFileStatus {
  path: string
  status: string // 'M' (modified), 'A' (added), 'D' (deleted), '?' (untracked), 'R' (renamed)
  staged: boolean
}

export interface GitStatus {
  isRepo: boolean
  branch?: string
  isClean: boolean
  ahead: number
  behind: number
  modifiedFiles: string[]
  untrackedFiles: string[]
  files: GitFileStatus[]
}

export interface BranchInfo {
  name: string
  isCurrent: boolean
  isRemote: boolean
}

export interface PullResult {
  success: boolean
  message: string
  filesChanged: number
}

export interface CommitResult {
  success: boolean
  message: string
  commitHash?: string
  pushed?: boolean
}

export interface MergeStatus {
  canFastForward: boolean
  ahead: number
  behind: number
  hasConflicts: boolean
  conflictingFiles: string[]
}

export interface MergeResult {
  success: boolean
  message: string
  mergedFiles: number
  conflictingFiles: string[]
}

export interface InitializeResult {
  initialized: boolean
  merged: boolean
  message: string
  conflictingFiles: string[]
}

// Config Change Event (from SSE)
export interface ConfigChangeEvent {
  changeType: 'created' | 'modified' | 'deleted' | 'renamed' | 'reloaded'
  path: string
  resourceKind?: ResourceKind
  resourceName?: string
}

// Git Progress Types (from SSE)
export type GitOperationType = 'commit' | 'push' | 'pull' | 'fetch' | 'merge' | 'checkout' | 'initialize'

export type GitOperationPhase =
  | 'starting'
  | 'staging_files'
  | 'committing'
  | 'counting'
  | 'compressing'
  | 'writing'
  | 'receiving'
  | 'resolving'
  | 'unpacking'
  | 'pushing'
  | 'pulling'
  | 'fetching'
  | 'merging'
  | 'checking_out'
  | 'completed'
  | 'failed'

export interface GitProgressEvent {
  operationId: string
  operationType: GitOperationType
  phase: GitOperationPhase
  message: string
  progress?: number // 0-100 percentage
  current?: number
  total?: number
  bytesTransferred?: number
  transferSpeed?: number
  rawOutput?: string
  error?: string
  timestamp: string
}

// Helper to get human-readable phase label
export function getPhaseLabel(phase: GitOperationPhase): string {
  switch (phase) {
    case 'starting':
      return 'Starting...'
    case 'staging_files':
      return 'Staging files...'
    case 'committing':
      return 'Committing...'
    case 'counting':
      return 'Counting objects...'
    case 'compressing':
      return 'Compressing objects...'
    case 'writing':
      return 'Writing objects...'
    case 'receiving':
      return 'Receiving objects...'
    case 'resolving':
      return 'Resolving deltas...'
    case 'unpacking':
      return 'Unpacking objects...'
    case 'pushing':
      return 'Pushing...'
    case 'pulling':
      return 'Pulling...'
    case 'fetching':
      return 'Fetching...'
    case 'merging':
      return 'Merging...'
    case 'checking_out':
      return 'Checking out...'
    case 'completed':
      return 'Completed'
    case 'failed':
      return 'Failed'
    default:
      return phase
  }
}

// Helper to get operation icon name
export function getOperationIcon(type: GitOperationType): string {
  switch (type) {
    case 'commit':
      return 'git-commit'
    case 'push':
      return 'arrow-up'
    case 'pull':
      return 'arrow-down'
    case 'fetch':
      return 'download'
    case 'merge':
      return 'git-merge'
    case 'checkout':
      return 'git-branch'
    case 'initialize':
      return 'folder-plus'
    default:
      return 'git-commit'
  }
}

// Helper to format bytes
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

// Helper to format speed
export function formatSpeed(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`
}

// File operations
export interface FileResponse {
  success: boolean
  message?: string
  error?: string
}

// Helper functions
export function isSimpleMatch(condition: MatchCondition): condition is SimpleMatch {
  return (
    'contains' in condition ||
    'containsAny' in condition ||
    'containsAll' in condition ||
    'pattern' in condition
  )
}

export function isCompoundMatch(condition: MatchCondition): condition is CompoundMatch {
  return 'all' in condition || 'any' in condition || 'not' in condition
}

export function getMatchConditionType(
  condition: MatchCondition
): 'contains' | 'containsAny' | 'containsAll' | 'pattern' | 'all' | 'any' | 'not' {
  if ('all' in condition) return 'all'
  if ('any' in condition) return 'any'
  if ('not' in condition) return 'not'
  if ('contains' in condition) return 'contains'
  if ('containsAny' in condition) return 'containsAny'
  if ('containsAll' in condition) return 'containsAll'
  if ('pattern' in condition) return 'pattern'
  return 'contains'
}

export function createDefaultResource(kind: ResourceKind): AnyResource {
  const base = {
    apiVersion: API_VERSION,
    kind,
    metadata: {
      name: '',
      labels: {},
      annotations: {},
    },
  }

  switch (kind) {
    case 'Settings':
      return {
        ...base,
        kind: 'Settings',
        spec: {
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
          },
        },
      } as SettingsResource

    case 'Variable':
      return {
        ...base,
        kind: 'Variable',
        spec: {
          pattern: '(?P<value>\\w+)',
          transform: undefined,
          default: undefined,
        },
      } as VariableResource

    case 'Rule':
      return {
        ...base,
        kind: 'Rule',
        spec: {
          priority: 0,
          category: '',
          match: { contains: '' },
          output: {
            directory: '',
            filename: '$original',
          },
          symlinks: [],
        },
      } as RuleResource

    case 'ImportSource':
      return {
        ...base,
        kind: 'ImportSource',
        spec: {
          type: 'local',
          enabled: true,
          local: {
            path: '',
            recursive: false,
            filters: {
              include: ['*.pdf', '*.png', '*.jpg'],
              exclude: ['*.tmp', '.*'],
            },
            pollInterval: 60,
          },
        },
      } as ImportSourceResource
  }
}

export function resourceKindToIcon(kind: ResourceKind): string {
  switch (kind) {
    case 'Settings':
      return 'settings'
    case 'Variable':
      return 'variable'
    case 'Rule':
      return 'rule'
    case 'ImportSource':
      return 'folder-input'
  }
}

export function validateResourceName(name: string): string | null {
  if (!name) {
    return 'Name is required'
  }
  if (!/^[a-zA-Z_][a-zA-Z0-9_-]*$/.test(name)) {
    return 'Name must start with a letter or underscore, and contain only letters, numbers, underscores, and hyphens'
  }
  return null
}
