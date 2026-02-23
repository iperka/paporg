import { useEffect, useState, useMemo, useRef, useCallback } from 'react'
import { GitBranch, RefreshCw, Check, AlertCircle, Loader2, CloudOff, ExternalLink, FolderDown, Plus, ArrowRight, CheckCircle2, FileEdit, FilePlus, FileX, GitCommitHorizontal, ChevronRight, ChevronDown, Folder, FolderOpen, Settings, FileText, Variable, FolderInput, History } from 'lucide-react'
import { useGitProgressContext } from '@/contexts/GitProgressContext'
import { useToast } from '@/components/ui/use-toast'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { SecretField } from '@/components/form'
import { BranchSelector } from '@/components/gitops/BranchSelector'
import { CommitDialog } from '@/components/gitops/CommitDialog'
import { api, type CommitInfo } from '@/api'
import {
  type SettingsSpec,
  type SettingsResource,
  createDefaultSettingsSpec,
  settingsSpecSchema,
} from '@/schemas/resources'
import { type FileTreeNode } from '@/types/gitops'
import yaml from 'js-yaml'
import { useQueryClient } from '@tanstack/react-query'
import { useFileTree } from '@/queries/use-file-tree'
import { useGitStatus } from '@/queries/use-git-status'
import { useResource } from '@/queries/use-resource'
import { useUpdateResource, useGitPull, useInitializeGit } from '@/mutations/use-gitops-mutations'

type SetupMode = 'choose' | 'new' | 'existing' | 'configured'

interface GitFileStatus {
  path: string
  status: string
  staged: boolean
}

interface TreeNode {
  name: string
  path: string
  type: 'folder' | 'file'
  status?: string
  staged?: boolean
  children: TreeNode[]
}

function buildFileTree(files: GitFileStatus[]): TreeNode[] {
  const root: TreeNode[] = []

  for (const file of files) {
    // Clean up the path - remove trailing slashes
    const cleanPath = file.path.replace(/\/+$/, '')
    const parts = cleanPath.split('/').filter(p => p.length > 0)

    if (parts.length === 0) continue

    let currentLevel = root

    // Check if this is a directory entry (path ended with /)
    const isDirectoryEntry = file.path.endsWith('/')

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      const isLastPart = i === parts.length - 1
      const currentPath = parts.slice(0, i + 1).join('/')

      let existing = currentLevel.find(n => n.name === part)

      if (!existing) {
        // If it's a directory entry from git, treat the whole thing as a folder
        const isFolder = !isLastPart || isDirectoryEntry

        existing = {
          name: part,
          path: currentPath,
          type: isFolder ? 'folder' : 'file',
          status: isLastPart ? file.status : undefined,
          staged: isLastPart ? file.staged : undefined,
          children: [],
        }
        currentLevel.push(existing)
      }

      if (!isLastPart) {
        currentLevel = existing.children
      }
    }
  }

  // Sort: folders first, then alphabetically
  const sortNodes = (nodes: TreeNode[]): TreeNode[] => {
    return nodes
      .sort((a, b) => {
        if (a.type !== b.type) {
          return a.type === 'folder' ? -1 : 1
        }
        return a.name.localeCompare(b.name)
      })
      .map(node => ({
        ...node,
        children: sortNodes(node.children),
      }))
  }

  return sortNodes(root)
}

function getCategoryIcon(name: string) {
  const lowerName = name.toLowerCase()
  if (lowerName === 'rules' || lowerName.endsWith('.rule.yaml')) {
    return <FileText className="h-4 w-4 text-blue-500" />
  }
  if (lowerName === 'sources' || lowerName.endsWith('.source.yaml')) {
    return <FolderInput className="h-4 w-4 text-purple-500" />
  }
  if (lowerName === 'variables' || lowerName.endsWith('.variable.yaml')) {
    return <Variable className="h-4 w-4 text-orange-500" />
  }
  if (lowerName === 'settings.yaml' || lowerName === 'settings') {
    return <Settings className="h-4 w-4 text-gray-500" />
  }
  return null
}

function getStatusIcon(status?: string) {
  if (!status) return null
  // Handle combined statuses like "MM", "AM", etc.
  const s = status.trim().charAt(0) || status.trim().charAt(1)
  switch (s) {
    case 'M':
      return <FileEdit className="h-3.5 w-3.5 text-yellow-500 shrink-0" />
    case 'A':
    case '?':
      return <FilePlus className="h-3.5 w-3.5 text-green-500 shrink-0" />
    case 'D':
      return <FileX className="h-3.5 w-3.5 text-red-500 shrink-0" />
    case 'R':
      return <FileEdit className="h-3.5 w-3.5 text-blue-500 shrink-0" />
    case 'C':
      return <FilePlus className="h-3.5 w-3.5 text-blue-500 shrink-0" />
    case 'U':
      return <AlertCircle className="h-3.5 w-3.5 text-red-500 shrink-0" />
    default:
      return <FileEdit className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
  }
}

function getStatusLabel(status?: string) {
  if (!status) return null
  const s = status.trim().charAt(0) || status.trim().charAt(1)
  switch (s) {
    case 'M':
      return 'Modified'
    case 'A':
      return 'Added'
    case '?':
      return 'New'
    case 'D':
      return 'Deleted'
    case 'R':
      return 'Renamed'
    case 'C':
      return 'Copied'
    case 'U':
      return 'Conflict'
    default:
      return 'Changed'
  }
}

function TreeNodeItem({ node, level = 0 }: { node: TreeNode; level?: number }) {
  const [expanded, setExpanded] = useState(true)
  const hasChildren = node.children.length > 0
  const categoryIcon = getCategoryIcon(node.name)
  const statusLabel = getStatusLabel(node.status)

  if (node.type === 'folder') {
    const folderStatusLabel = getStatusLabel(node.status)
    const folderStatusChar = node.status?.trim().charAt(0) || ''

    return (
      <div>
        <button
          onClick={() => hasChildren ? setExpanded(!expanded) : undefined}
          className="flex items-center gap-1.5 w-full py-1.5 px-1 rounded hover:bg-muted/50 text-sm text-left"
          style={{ paddingLeft: `${level * 12 + 4}px` }}
        >
          {hasChildren ? (
            expanded ? (
              <ChevronDown className="h-4 w-4 text-muted-foreground shrink-0" />
            ) : (
              <ChevronRight className="h-4 w-4 text-muted-foreground shrink-0" />
            )
          ) : (
            <span className="w-4 shrink-0" />
          )}
          {categoryIcon || (expanded ? (
            <FolderOpen className="h-4 w-4 text-amber-500 shrink-0" />
          ) : (
            <Folder className="h-4 w-4 text-amber-500 shrink-0" />
          ))}
          <span className="font-medium flex-1">{node.name}</span>
          {hasChildren && (
            <span className="text-xs text-muted-foreground">
              {node.children.length} {node.children.length === 1 ? 'file' : 'files'}
            </span>
          )}
          {folderStatusLabel && !hasChildren && (
            <Badge
              variant="outline"
              className={`text-xs ${
                folderStatusChar === 'M' ? 'border-yellow-500/50 text-yellow-600' :
                folderStatusChar === 'D' ? 'border-red-500/50 text-red-600' :
                'border-green-500/50 text-green-600'
              }`}
            >
              {folderStatusLabel}
            </Badge>
          )}
        </button>
        {expanded && hasChildren && (
          <div>
            {node.children.map((child) => (
              <TreeNodeItem key={child.path} node={child} level={level + 1} />
            ))}
          </div>
        )}
      </div>
    )
  }

  const statusChar = node.status?.trim().charAt(0) || ''

  return (
    <div
      className="flex items-center gap-2 py-1.5 px-1 rounded hover:bg-muted/50 text-sm"
      style={{ paddingLeft: `${level * 12 + 24}px` }}
    >
      <span className="shrink-0">{getStatusIcon(node.status)}</span>
      <span className="shrink-0">
        {categoryIcon || <FileText className="h-4 w-4 text-muted-foreground" />}
      </span>
      <span className="flex-1 min-w-0 truncate font-mono text-xs">{node.name}</span>
      {statusLabel && (
        <Badge
          variant="outline"
          className={`shrink-0 text-xs ${
            statusChar === 'M' ? 'border-yellow-500/50 text-yellow-600' :
            statusChar === 'D' ? 'border-red-500/50 text-red-600' :
            statusChar === 'U' ? 'border-red-500/50 text-red-600' :
            'border-green-500/50 text-green-600'
          }`}
        >
          {statusLabel}
        </Badge>
      )}
    </div>
  )
}

function buildFullTree(
  fileTree: FileTreeNode | null,
  gitFiles: GitFileStatus[],
): TreeNode[] {
  if (!fileTree) return buildFileTree(gitFiles)

  // Create a map of git statuses by path
  const statusMap = new Map<string, GitFileStatus>()
  for (const file of gitFiles) {
    const cleanPath = file.path.replace(/\/+$/, '')
    statusMap.set(cleanPath, file)
  }

  // Check if a path or any parent is marked as untracked directory
  const getEffectiveStatus = (path: string): GitFileStatus | undefined => {
    // Direct match
    if (statusMap.has(path)) {
      return statusMap.get(path)
    }
    // Check if any parent directory is untracked
    const parts = path.split('/')
    for (let i = 1; i <= parts.length; i++) {
      const parentPath = parts.slice(0, i).join('/')
      const parentStatus = statusMap.get(parentPath)
      if (parentStatus && parentStatus.status === '?') {
        return { path, status: '?', staged: false }
      }
    }
    return undefined
  }

  const processNode = (node: FileTreeNode): TreeNode | null => {
    const relativePath = node.path.startsWith('/')
      ? node.path.slice(1)
      : node.path

    const status = getEffectiveStatus(relativePath)

    // For directories, process children
    if (node.isDirectory) {
      const children = node.children
        .map(child => processNode(child))
        .filter((n): n is TreeNode => n !== null)

      // Only include directories that have changed files or are themselves changed
      if (children.length === 0 && !status) {
        return null
      }

      return {
        name: node.name,
        path: relativePath,
        type: 'folder',
        status: status?.status,
        staged: status?.staged,
        children,
      }
    }

    // For files, only include if they have a status
    if (!status) {
      return null
    }

    return {
      name: node.name,
      path: relativePath,
      type: 'file',
      status: status.status,
      staged: status.staged,
      children: [],
    }
  }

  // Process root children (skip the root node itself)
  const result = fileTree.children
    .map(child => processNode(child))
    .filter((n): n is TreeNode => n !== null)
    .sort((a, b) => {
      if (a.type !== b.type) {
        return a.type === 'folder' ? -1 : 1
      }
      return a.name.localeCompare(b.name)
    })

  return result
}

function ChangesTree({ files, fileTree }: { files: GitFileStatus[]; fileTree: FileTreeNode | null }) {
  const tree = useMemo(() => buildFullTree(fileTree, files), [fileTree, files])

  if (files.length === 0) {
    return (
      <p className="text-sm text-muted-foreground text-center py-4">
        No changes to commit
      </p>
    )
  }

  return (
    <div className="max-h-72 overflow-y-auto -mx-2">
      {tree.map((node) => (
        <TreeNodeItem key={node.path} node={node} />
      ))}
    </div>
  )
}

function CommitHistoryCard() {
  const [commits, setCommits] = useState<CommitInfo[]>([])
  const [isLoadingCommits, setIsLoadingCommits] = useState(false)

  const loadCommits = useCallback(async () => {
    setIsLoadingCommits(true)
    try {
      const result = await api.git.log(10)
      setCommits(result)
    } catch {
      // Silently fail — history is non-critical
    } finally {
      setIsLoadingCommits(false)
    }
  }, [])

  useEffect(() => {
    loadCommits()
  }, [loadCommits])

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">Commit History</CardTitle>
            <CardDescription>Recent commits in this repository</CardDescription>
          </div>
          <Button variant="ghost" size="sm" onClick={loadCommits} disabled={isLoadingCommits}>
            <RefreshCw className={`h-4 w-4 ${isLoadingCommits ? 'animate-spin' : ''}`} />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {isLoadingCommits && commits.length === 0 ? (
          <div className="flex items-center justify-center py-6">
            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
          </div>
        ) : commits.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">No commits yet</p>
        ) : (
          <div className="space-y-1 max-h-64 overflow-y-auto -mx-2">
            {commits.map((commit) => (
              <div key={commit.hash} className="flex items-start gap-3 px-2 py-2 rounded hover:bg-muted/50">
                <History className="h-4 w-4 mt-0.5 text-muted-foreground shrink-0" />
                <div className="flex-1 min-w-0">
                  <p className="text-sm truncate">{commit.message}</p>
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <code className="text-xs">{commit.hash.slice(0, 7)}</code>
                    <span>&middot;</span>
                    <span>{commit.author}</span>
                    <span>&middot;</span>
                    <span>{new Date(commit.date).toLocaleDateString()}</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

export function GitSyncPage() {
  const qc = useQueryClient()
  const { data: fileTree } = useFileTree()
  const { data: gitStatus } = useGitStatus()
  const { hasActiveOperations, activeOperations } = useGitProgressContext()
  const { toast } = useToast()

  // Derive the Settings resource name from the file tree
  const settingsName = useMemo(() => {
    const findSettingsName = (node: FileTreeNode | null): string | null => {
      if (!node) return null
      if (node.resource?.kind === 'Settings') {
        return node.resource.name
      }
      for (const child of node.children) {
        const found = findSettingsName(child)
        if (found) return found
      }
      return null
    }
    return findSettingsName(fileTree)
  }, [fileTree])

  // Load the Settings resource YAML by kind+name
  const { data: resourceData } = useResource('Settings', settingsName ?? '', !!settingsName)

  // Mutation hooks
  const updateResourceMut = useUpdateResource()
  const gitPullMut = useGitPull()
  const initializeGitMut = useInitializeGit()

  // Compute derived state
  const initialLoadComplete = fileTree !== null || gitStatus !== null

  const [formData, setFormData] = useState<SettingsSpec>(createDefaultSettingsSpec())
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [initialData, setInitialData] = useState<SettingsSpec | null>(null)
  const [remoteBranches, setRemoteBranches] = useState<string[]>([])
  const [isFetchingBranches, setIsFetchingBranches] = useState(false)
  const [connectionStatus, setConnectionStatus] = useState<'unknown' | 'success' | 'error'>('unknown')
  const [setupMode, setSetupMode] = useState<SetupMode>('choose')
  const [currentStep, setCurrentStep] = useState(1)
  const [isInitializing, setIsInitializing] = useState(false)
  const [commitDialogOpen, setCommitDialogOpen] = useState(false)
  const [statusChecked, setStatusChecked] = useState(false)

  const needsInitialization = Boolean(
    initialLoadComplete &&
    formData.git.enabled &&
    formData.git.repository &&
    gitStatus &&
    !gitStatus.isRepo
  )

  const isLoading = gitPullMut.isPending || initializeGitMut.isPending || updateResourceMut.isPending

  // Refresh git status when configured mode is shown, track completion
  useEffect(() => {
    if (setupMode !== 'configured') return
    let active = true
    setStatusChecked(false)
    qc.invalidateQueries({ queryKey: ['git', 'status'] }).finally(() => {
      if (active) setStatusChecked(true)
    })
    return () => { active = false }
  }, [setupMode, qc])

  // Track if initial load has determined the mode
  const hasSetInitialModeRef = useRef(false)

  const isBusy = isLoading || hasActiveOperations
  const isConfigured = formData.git.enabled && formData.git.repository

  // Determine initial mode based on current state - only on first meaningful load
  useEffect(() => {
    // Skip if we've already determined the initial mode
    if (hasSetInitialModeRef.current) return

    // Wait for settings to be loaded (initialData is set when settings are parsed)
    if (!initialData) return

    // Wait for context initial load to complete so gitStatus is fresh
    if (!initialLoadComplete) return

    // Mark that we've checked the initial state
    hasSetInitialModeRef.current = true

    // If git is already configured, show the configured view
    if (initialData.git.enabled && initialData.git.repository) {
      setSetupMode('configured')
    }
  }, [initialData, initialLoadComplete])

  useEffect(() => {
    if (!resourceData?.yaml) return

    try {
      const parsed = yaml.load(resourceData.yaml) as SettingsResource
      if (parsed?.spec) {
        const defaults = createDefaultSettingsSpec()

        // Handle legacy 'ssh-agent' type that might exist in old config files
        const rawAuthType = (parsed.spec.git?.auth?.type as string) || 'none'
        const authType: 'none' | 'token' | 'ssh-key' =
          rawAuthType === 'ssh-agent' ? 'none' : (rawAuthType as 'none' | 'token' | 'ssh-key')

        const mergedSpec: SettingsSpec = {
          ...defaults,
          ...parsed.spec,
          ocr: { ...defaults.ocr, ...parsed.spec.ocr },
          defaults: {
            ...defaults.defaults,
            output: { ...defaults.defaults.output, ...parsed.spec.defaults?.output },
          },
          git: {
            ...defaults.git,
            ...parsed.spec.git,
            auth: {
              ...defaults.git.auth,
              ...parsed.spec.git?.auth,
              type: authType as 'none' | 'token' | 'ssh-key',
            },
          },
        }
        setFormData(mergedSpec)
        setInitialData(JSON.parse(JSON.stringify(mergedSpec)))
        setError(null)

        // Only set configured mode on initial load, not on subsequent updates
        // (e.g., when testing connection saves settings temporarily)
      }
    } catch {
      setError('Failed to parse settings YAML')
    }
  }, [resourceData])


  const updateGit = <K extends keyof SettingsSpec['git']>(
    field: K,
    fieldValue: SettingsSpec['git'][K]
  ) => {
    setFormData({
      ...formData,
      git: { ...formData.git, [field]: fieldValue },
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
    setFormData({
      ...formData,
      git: {
        ...formData.git,
        auth: { ...formData.git.auth, ...updates },
      },
    })
  }

  const handleSave = async () => {
    // Enable git when saving (completing setup)
    const dataToSave = {
      ...formData,
      git: { ...formData.git, enabled: true },
    }

    const validation = settingsSpecSchema.safeParse(dataToSave)
    if (!validation.success) {
      const firstError = validation.error.errors[0]
      setError(`Validation error: ${firstError.path.join('.')}: ${firstError.message}`)
      return
    }

    setIsSaving(true)
    setError(null)

    try {
      const existingMetadata = resourceData?.yaml
        ? (yaml.load(resourceData.yaml) as SettingsResource)?.metadata
        : null

      const metadata = existingMetadata || { name: 'settings', labels: {}, annotations: {} }
      const resource: SettingsResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Settings',
        metadata,
        spec: dataToSave,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })

      await updateResourceMut.mutateAsync({ kind: 'Settings', name: metadata.name, yamlContent: newYaml })

      // Reload config so backend picks up the new settings
      await api.config.reload()

      setFormData(dataToSave)
      setInitialData(JSON.parse(JSON.stringify(dataToSave)))
      setSetupMode('configured')
      toast({
        title: 'Settings saved',
        description: 'Git sync has been configured successfully.',
      })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save settings')
    } finally {
      setIsSaving(false)
    }
  }

  const handleSync = async () => {
    try {
      await gitPullMut.mutateAsync()
      await qc.invalidateQueries({ queryKey: ['git', 'status'] })
      toast({
        title: 'Sync complete',
        description: 'Successfully synced with remote.',
      })
    } catch (err) {
      toast({
        variant: 'destructive',
        title: 'Sync failed',
        description: err instanceof Error ? err.message : 'Failed to sync',
      })
    }
  }

  const handleTestConnection = async (): Promise<boolean> => {
    if (!formData.git.repository) {
      setError('Please enter a repository URL first')
      return false
    }

    setIsFetchingBranches(true)
    setError(null)
    setConnectionStatus('unknown')

    try {
      const existingMetadata = resourceData?.yaml
        ? (yaml.load(resourceData.yaml) as SettingsResource)?.metadata
        : null

      // Save settings with enabled: true so backend can connect to the repo
      const metadata = existingMetadata || { name: 'settings', labels: {}, annotations: {} }
      const resource: SettingsResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Settings',
        metadata,
        spec: { ...formData, git: { ...formData.git, enabled: true } },
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })
      await updateResourceMut.mutateAsync({ kind: 'Settings', name: metadata.name, yamlContent: newYaml })

      // Reload config so backend picks up the new settings
      await api.config.reload()

      // Initialize (clone) the repository so we can fetch branches
      await initializeGitMut.mutateAsync()

      const branches = await api.git.getBranches()
      const branchNames = branches
        .filter(b => b.isRemote)
        .map(b => b.name.replace(/^origin\//, ''))
        .filter((name, index, arr) => arr.indexOf(name) === index)

      if (branchNames.length === 0) {
        const localBranches = branches.filter(b => !b.isRemote).map(b => b.name)
        setRemoteBranches(localBranches.length > 0 ? localBranches : ['main'])
      } else {
        setRemoteBranches(branchNames)
      }

      setConnectionStatus('success')
      toast({
        title: 'Connection successful',
        description: `Found ${branchNames.length || 1} branch(es)`,
      })
      return true
    } catch (err) {
      setConnectionStatus('error')
      setError(err instanceof Error ? err.message : 'Failed to connect to repository')
      return false
    } finally {
      setIsFetchingBranches(false)
    }
  }

  const handleDisable = async () => {
    // Reset all git settings to defaults
    const resetGit = {
      enabled: false,
      repository: '',
      branch: 'main',
      syncInterval: 300,
      auth: {
        type: 'none' as const,
        tokenEnvVar: '',
        sshKeyPath: '',
      },
      userName: 'Paporg',
      userEmail: 'paporg@localhost',
    }

    const newFormData = {
      ...formData,
      git: resetGit,
    }

    // Save the reset settings to disk
    try {
      const existingMetadata = resourceData?.yaml
        ? (yaml.load(resourceData.yaml) as SettingsResource)?.metadata
        : null

      const metadata = existingMetadata || { name: 'settings', labels: {}, annotations: {} }
      const resource: SettingsResource = {
        apiVersion: 'paporg.io/v1',
        kind: 'Settings',
        metadata,
        spec: newFormData,
      }
      const newYaml = yaml.dump(resource, { lineWidth: -1 })
      await updateResourceMut.mutateAsync({ kind: 'Settings', name: metadata.name, yamlContent: newYaml })

      // Reload config so backend picks up the reset settings
      await api.config.reload()
    } catch (e) {
      console.error('Failed to save reset settings:', e)
    }

    setFormData(newFormData)
    // Also reset initialData so the mode check doesn't redirect back
    setInitialData({
      ...formData,
      git: resetGit,
    })
    // Reset the ref so auto-switch works on next page load after reconfiguring
    hasSetInitialModeRef.current = false
    setSetupMode('choose')
    setCurrentStep(1)
    setConnectionStatus('unknown')
    setRemoteBranches([])
    setError(null)
  }

  const handleInitialize = async () => {
    setIsInitializing(true)
    setError(null)

    try {
      // Ensure config is loaded before initializing
      await api.config.reload()

      const result = await initializeGitMut.mutateAsync()
      if (result) {
        // Reload config after initialization to pick up any merged changes
        await api.config.reload()

        toast({
          title: 'Repository initialized',
          description: 'Git sync is now active and your configuration is being synced.',
        })
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to initialize repository')
      toast({
        variant: 'destructive',
        title: 'Initialization failed',
        description: err instanceof Error ? err.message : 'Failed to initialize repository',
      })
    } finally {
      setIsInitializing(false)
    }
  }

  const startNewSetup = () => {
    setSetupMode('new')
    setCurrentStep(1)
  }

  const startExistingSetup = () => {
    setSetupMode('existing')
    setCurrentStep(1)
  }

  // Render pending initialization state — wait for fresh status check
  if (setupMode === 'configured' && isConfigured && needsInitialization) {
    if (!statusChecked) {
      return (
        <div className="flex items-center justify-center py-16" role="status" aria-label="Checking repository status">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          <span className="sr-only">Checking repository status</span>
        </div>
      )
    }
    return (
      <div className="space-y-6 max-w-2xl mx-auto">
        {/* Header */}
        <div className="text-center space-y-2">
          <div className="h-12 w-12 rounded-full bg-amber-500/10 flex items-center justify-center mx-auto">
            <CloudOff className="h-6 w-6 text-amber-500" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight">Repository Not Synced</h1>
          <p className="text-muted-foreground">
            Git sync is configured but the repository needs to be initialized
          </p>
        </div>

        {/* Error display */}
        {error && (
          <div className="flex items-center gap-2 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
            <AlertCircle className="h-4 w-4 shrink-0" />
            {error}
          </div>
        )}

        {/* Main card */}
        <Card>
          <CardHeader>
            <CardTitle>Complete Setup</CardTitle>
            <CardDescription>
              Initialize the repository to start syncing your configuration
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="p-4 rounded-lg bg-muted/50 space-y-2">
              <p className="text-xs text-muted-foreground">Repository</p>
              <p className="text-sm font-mono truncate">{formData.git.repository}</p>
            </div>

            <div className="text-sm text-muted-foreground space-y-2">
              <p>Clicking "Initialize & Sync" will:</p>
              <ul className="list-disc list-inside space-y-1 text-xs">
                <li>Clone the remote repository</li>
                <li>Merge any existing local configuration</li>
                <li>Start automatic synchronization</li>
              </ul>
            </div>

            <Button
              type="button"
              onClick={handleInitialize}
              disabled={isInitializing || isBusy}
              className="w-full"
            >
              {isInitializing ? (
                <>
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  Initializing...
                </>
              ) : (
                <>
                  <GitBranch className="h-4 w-4 mr-2" />
                  Initialize & Sync
                </>
              )}
            </Button>
          </CardContent>
        </Card>

        {/* Option to change settings */}
        <div className="flex justify-center">
          <Button variant="ghost" size="sm" onClick={handleDisable}>
            Change Repository Settings
          </Button>
        </div>
      </div>
    )
  }

  // Render configured state
  if (setupMode === 'configured' && isConfigured) {
    const totalChanges = (gitStatus?.modifiedFiles.length || 0) + (gitStatus?.untrackedFiles.length || 0)
    const hasChanges = totalChanges > 0

    return (
      <div className="space-y-6">
        {/* Header */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="h-10 w-10 rounded-full bg-green-500/10 flex items-center justify-center">
              <CheckCircle2 className="h-5 w-5 text-green-500" />
            </div>
            <div>
              <h1 className="text-2xl font-bold tracking-tight">Git Sync Active</h1>
              <p className="text-sm text-muted-foreground">
                Your configuration is synced with a remote repository
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button variant="outline" onClick={handleSync} disabled={isBusy} size="sm">
              <RefreshCw className={`h-4 w-4 mr-2 ${isBusy ? 'animate-spin' : ''}`} />
              Sync Now
            </Button>
          </div>
        </div>

        {/* Error */}
        {error && (
          <div className="flex items-center gap-2 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
            <AlertCircle className="h-4 w-4 shrink-0" />
            {error}
          </div>
        )}

        {/* Active operation progress */}
        {hasActiveOperations && (() => {
          const ops = Array.from(activeOperations.values())
          const activeOp = ops.find(op => op.phase !== 'completed' && op.phase !== 'failed') || ops[0]
          if (!activeOp) return null
          return (
            <div className="flex items-center gap-3 p-3 rounded-md bg-blue-500/10 border border-blue-500/20 text-sm">
              <Loader2 className="h-4 w-4 animate-spin text-blue-500 shrink-0" />
              <div className="flex-1 min-w-0">
                <p className="font-medium capitalize">{activeOp.operationType}</p>
                <p className="text-xs text-muted-foreground truncate">{activeOp.message}</p>
              </div>
              {activeOp.progress !== undefined && (
                <div className="w-24">
                  <div className="h-1.5 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-full bg-blue-500 transition-all duration-300"
                      style={{ width: `${activeOp.progress}%` }}
                    />
                  </div>
                </div>
              )}
            </div>
          )
        })()}

        {/* Status */}
        <Card>
          <CardContent className="pt-6">
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
              <div>
                <p className="text-xs text-muted-foreground mb-1">Repository</p>
                <p className="text-sm font-mono truncate">{formData.git.repository}</p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground mb-1">Branch</p>
                <BranchSelector />
              </div>
              <div>
                <p className="text-xs text-muted-foreground mb-1">Status</p>
                {gitStatus?.isClean ? (
                  <Badge variant="outline" className="gap-1">
                    <Check className="h-3 w-3" />
                    Clean
                  </Badge>
                ) : (
                  <Badge variant="secondary">
                    {totalChanges} changes
                  </Badge>
                )}
              </div>
              <div>
                <p className="text-xs text-muted-foreground mb-1">Auto-sync</p>
                <p className="text-sm">
                  {formData.git.syncInterval > 0 ? `Every ${formData.git.syncInterval}s` : 'Disabled'}
                </p>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Changes */}
        <Card>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <div>
                <CardTitle className="text-base">Uncommitted Changes</CardTitle>
                <CardDescription>
                  {hasChanges
                    ? `${totalChanges} file${totalChanges !== 1 ? 's' : ''} modified locally`
                    : 'All changes have been committed'
                  }
                </CardDescription>
              </div>
              {hasChanges && (
                <Button onClick={() => setCommitDialogOpen(true)} size="sm">
                  <GitCommitHorizontal className="h-4 w-4 mr-2" />
                  Commit & Push
                </Button>
              )}
            </div>
          </CardHeader>
          <CardContent>
            {hasChanges ? (
              <ChangesTree files={gitStatus?.files || []} fileTree={fileTree} />
            ) : (
              <div className="flex flex-col items-center justify-center py-8 text-center">
                <div className="h-12 w-12 rounded-full bg-green-500/10 flex items-center justify-center mb-3">
                  <CheckCircle2 className="h-6 w-6 text-green-500" />
                </div>
                <p className="text-sm font-medium">Everything is up to date</p>
                <p className="text-xs text-muted-foreground mt-1">
                  Your local configuration matches the remote repository
                </p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Commit History */}
        <CommitHistoryCard />

        {/* Commit Dialog */}
        <CommitDialog
          open={commitDialogOpen}
          onOpenChange={(open) => {
            setCommitDialogOpen(open)
            // Refresh status when dialog closes
            if (!open) {
              qc.invalidateQueries({ queryKey: ['git', 'status'] })
            }
          }}
        />

        {/* Actions */}
        <div className="flex justify-end">
          <Button variant="ghost" size="sm" onClick={handleDisable}>
            Disconnect Repository
          </Button>
        </div>
      </div>
    )
  }

  // Render setup wizard
  return (
    <div className="space-y-6 max-w-2xl mx-auto">
      {/* Header */}
      <div className="text-center space-y-2">
        <div className="h-12 w-12 rounded-full bg-primary/10 flex items-center justify-center mx-auto">
          <GitBranch className="h-6 w-6 text-primary" />
        </div>
        <h1 className="text-2xl font-bold tracking-tight">Git Sync Setup</h1>
        <p className="text-muted-foreground">
          Keep your Paporg configuration backed up and synced across devices
        </p>
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-2 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {/* Choose Mode */}
      {setupMode === 'choose' && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <Card
            className="cursor-pointer transition-all hover:ring-2 hover:ring-primary/50"
            onClick={startNewSetup}
          >
            <CardHeader className="text-center pb-2">
              <div className="h-10 w-10 rounded-full bg-blue-500/10 flex items-center justify-center mx-auto mb-2">
                <Plus className="h-5 w-5 text-blue-500" />
              </div>
              <CardTitle className="text-lg">I'm new to Paporg</CardTitle>
            </CardHeader>
            <CardContent className="text-center">
              <CardDescription>
                Create a new repository from our template to get started with best practices
              </CardDescription>
            </CardContent>
          </Card>

          <Card
            className="cursor-pointer transition-all hover:ring-2 hover:ring-primary/50"
            onClick={startExistingSetup}
          >
            <CardHeader className="text-center pb-2">
              <div className="h-10 w-10 rounded-full bg-green-500/10 flex items-center justify-center mx-auto mb-2">
                <FolderDown className="h-5 w-5 text-green-500" />
              </div>
              <CardTitle className="text-lg">I have a config repo</CardTitle>
            </CardHeader>
            <CardContent className="text-center">
              <CardDescription>
                Connect to your existing Paporg configuration repository
              </CardDescription>
            </CardContent>
          </Card>
        </div>
      )}

      {/* New User Flow */}
      {setupMode === 'new' && (
        <div className="space-y-6">
          {/* Progress */}
          <div className="flex items-center justify-center gap-2">
            {[1, 2, 3].map((step) => (
              <div key={step} className="flex items-center">
                <div className={`h-8 w-8 rounded-full flex items-center justify-center text-sm font-medium ${
                  currentStep >= step ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'
                }`}>
                  {currentStep > step ? <Check className="h-4 w-4" /> : step}
                </div>
                {step < 3 && (
                  <div className={`w-12 h-0.5 ${currentStep > step ? 'bg-primary' : 'bg-muted'}`} />
                )}
              </div>
            ))}
          </div>

          {/* Step 1: Create Repo */}
          {currentStep === 1 && (
            <Card>
              <CardHeader>
                <CardTitle>Step 1: Create a Repository</CardTitle>
                <CardDescription>
                  First, create a new Git repository to store your configuration
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="p-4 rounded-lg bg-muted/50 space-y-3">
                  <p className="text-sm font-medium">Option A: Use our template (Recommended)</p>
                  <p className="text-sm text-muted-foreground">
                    Our template includes example rules, variables, and best practices to help you get started quickly.
                  </p>
                  <Button asChild>
                    <a href="https://github.com/iperka/paporg-config-template" target="_blank" rel="noopener noreferrer">
                      <ExternalLink className="h-4 w-4 mr-2" />
                      Use Template on GitHub
                    </a>
                  </Button>
                </div>

                <div className="relative">
                  <div className="absolute inset-0 flex items-center">
                    <span className="w-full border-t" />
                  </div>
                  <div className="relative flex justify-center text-xs uppercase">
                    <span className="bg-background px-2 text-muted-foreground">or</span>
                  </div>
                </div>

                <div className="p-4 rounded-lg bg-muted/50 space-y-3">
                  <p className="text-sm font-medium">Option B: Create an empty repository</p>
                  <p className="text-sm text-muted-foreground">
                    Create a new empty repository on GitHub, GitLab, or any Git hosting service.
                  </p>
                </div>

                <div className="flex justify-between pt-4">
                  <Button type="button" variant="ghost" onClick={() => setSetupMode('choose')}>
                    Back
                  </Button>
                  <Button type="button" onClick={() => setCurrentStep(2)}>
                    I've created a repository
                    <ArrowRight className="h-4 w-4 ml-2" />
                  </Button>
                </div>
              </CardContent>
            </Card>
          )}

          {/* Step 2 & 3: Same as existing flow */}
          {currentStep >= 2 && (
            <>
              {renderConnectionStep()}
              {currentStep === 3 && renderBranchStep()}
            </>
          )}
        </div>
      )}

      {/* Existing User Flow */}
      {setupMode === 'existing' && (
        <div className="space-y-6">
          {/* Progress */}
          <div className="flex items-center justify-center gap-2">
            {[1, 2].map((step) => (
              <div key={step} className="flex items-center">
                <div className={`h-8 w-8 rounded-full flex items-center justify-center text-sm font-medium ${
                  currentStep >= step ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'
                }`}>
                  {currentStep > step ? <Check className="h-4 w-4" /> : step}
                </div>
                {step < 2 && (
                  <div className={`w-12 h-0.5 ${currentStep > step ? 'bg-primary' : 'bg-muted'}`} />
                )}
              </div>
            ))}
          </div>

          {currentStep === 1 && renderConnectionStep()}
          {currentStep === 2 && renderBranchStep()}
        </div>
      )}
    </div>
  )

  function renderConnectionStep() {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{setupMode === 'new' ? 'Step 2: Connect Repository' : 'Step 1: Connect Repository'}</CardTitle>
          <CardDescription>
            Enter your repository URL and authentication details
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="repository">Repository URL</Label>
            <Input
              id="repository"
              value={formData.git.repository}
              onChange={(e) => {
                updateGit('repository', e.target.value)
                setConnectionStatus('unknown')
                setRemoteBranches([])
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault()
                }
              }}
              placeholder="https://github.com/username/paporg-config.git"
              className="font-mono text-sm"
            />
            <p className="text-xs text-muted-foreground">
              Supports HTTPS and SSH URLs (e.g., git@github.com:user/repo.git)
            </p>
          </div>

          <div className="space-y-2">
            <Label>Authentication</Label>
            <Select
              value={formData.git.auth.type}
              onValueChange={(v) => {
                updateGitAuth('type', v as 'none' | 'token' | 'ssh-key')
                setConnectionStatus('unknown')
              }}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">
                  <div>
                    <p>None / SSH Agent</p>
                    <p className="text-xs text-muted-foreground">For public repos or if you have SSH keys set up</p>
                  </div>
                </SelectItem>
                <SelectItem value="token">
                  <div>
                    <p>Access Token</p>
                    <p className="text-xs text-muted-foreground">GitHub/GitLab personal access token</p>
                  </div>
                </SelectItem>
                <SelectItem value="ssh-key">
                  <div>
                    <p>SSH Key File</p>
                    <p className="text-xs text-muted-foreground">Specify path to a private key</p>
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          {formData.git.auth.type === 'token' && (
            <SecretField
              label="Access Token"
              sourceName="git"
              secretType="token"
              filePath={formData.git.auth.tokenFile}
              envVar={formData.git.auth.tokenEnvVar}
              onFilePathChange={(v) => updateGitAuth('tokenFile', v, 'tokenEnvVar')}
              onEnvVarChange={(v) => updateGitAuth('tokenEnvVar', v, 'tokenFile')}
              description="Create a token with 'repo' scope at GitHub → Settings → Developer settings → Personal access tokens"
            />
          )}

          {formData.git.auth.type === 'ssh-key' && (
            <div className="space-y-2">
              <Label htmlFor="sshKeyPath">SSH Key Path</Label>
              <Input
                id="sshKeyPath"
                value={formData.git.auth.sshKeyPath || ''}
                onChange={(e) => updateGitAuth('sshKeyPath', e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault()
                  }
                }}
                placeholder="~/.ssh/id_rsa"
                className="font-mono"
              />
            </div>
          )}

          {connectionStatus === 'success' && (
            <div className="flex items-center gap-2 p-3 rounded-md bg-green-500/10 text-green-600 dark:text-green-400 text-sm">
              <CheckCircle2 className="h-4 w-4 shrink-0" />
              Connected successfully! Found {remoteBranches.length} branch(es).
            </div>
          )}

          <div className="flex justify-between pt-4">
            <Button type="button" variant="ghost" onClick={() => {
              if (setupMode === 'new' && currentStep === 2) {
                setCurrentStep(1)
              } else {
                setSetupMode('choose')
              }
            }}>
              Back
            </Button>
            <Button
              type="button"
              onClick={async () => {
                const success = await handleTestConnection()
                if (success) {
                  setCurrentStep(setupMode === 'new' ? 3 : 2)
                }
              }}
              disabled={!formData.git.repository || isFetchingBranches}
            >
              {isFetchingBranches ? (
                <>
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  Testing...
                </>
              ) : connectionStatus === 'success' ? (
                <>
                  Continue
                  <ArrowRight className="h-4 w-4 ml-2" />
                </>
              ) : (
                <>
                  Test Connection
                  <ArrowRight className="h-4 w-4 ml-2" />
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>
    )
  }

  function renderBranchStep() {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{setupMode === 'new' ? 'Step 3: Configure Sync' : 'Step 2: Configure Sync'}</CardTitle>
          <CardDescription>
            Choose your branch and sync preferences
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label>Branch</Label>
            <Select
              value={formData.git.branch}
              onValueChange={(v) => updateGit('branch', v)}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select a branch" />
              </SelectTrigger>
              <SelectContent>
                {remoteBranches.map((branch) => (
                  <SelectItem key={branch} value={branch}>
                    {branch}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label>Auto-sync</Label>
            <Select
              value={formData.git.syncInterval.toString()}
              onValueChange={(v) => updateGit('syncInterval', parseInt(v))}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="0">Disabled (manual sync only)</SelectItem>
                <SelectItem value="60">Every minute</SelectItem>
                <SelectItem value="300">Every 5 minutes</SelectItem>
                <SelectItem value="900">Every 15 minutes</SelectItem>
                <SelectItem value="3600">Every hour</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              How often Paporg checks for updates from the remote repository
            </p>
          </div>

          <div className="flex justify-between pt-4">
            <Button type="button" variant="ghost" onClick={() => setCurrentStep(setupMode === 'new' ? 2 : 1)}>
              Back
            </Button>
            <Button type="button" onClick={handleSave} disabled={isSaving}>
              {isSaving ? (
                <>
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  Saving...
                </>
              ) : (
                <>
                  <Check className="h-4 w-4 mr-2" />
                  Complete Setup
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>
    )
  }
}
