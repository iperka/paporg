import React, { createContext, useContext, useEffect, useState, useCallback, useRef, useMemo } from 'react'
import yaml from 'js-yaml'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { api } from '@/api'
import type {
  BranchInfo,
  ConfigChangeEvent,
  FileStatus,
  FileTreeNode,
  GitStatus,
  InitializeResult,
  ResourceKind,
  ResourceDetail,
  SettingsResource,
} from '@/types/gitops'

interface GitOpsContextValue {
  // State
  fileTree: FileTreeNode | null
  gitStatus: GitStatus | null
  branches: BranchInfo[]
  selectedPath: string | null
  selectedResource: ResourceDetail | null
  isLoading: boolean
  error: string | null
  isConnected: boolean
  settings: SettingsResource | null
  needsInitialization: boolean
  initialLoadComplete: boolean

  // Actions
  refreshTree: () => Promise<void>
  refreshGitStatus: () => Promise<void>
  refreshBranches: () => Promise<void>
  selectFile: (path: string) => Promise<void>
  createResource: (kind: ResourceKind, yaml: string, path?: string) => Promise<boolean>
  updateResource: (kind: ResourceKind, name: string, yaml: string) => Promise<boolean>
  deleteResource: (kind: ResourceKind, name: string) => Promise<boolean>
  gitPull: () => Promise<boolean>
  gitCommit: (message: string, files?: string[]) => Promise<boolean>
  checkoutBranch: (branch: string) => Promise<boolean>
  createBranch: (name: string) => Promise<boolean>
  moveFile: (source: string, destination: string) => Promise<boolean>
  createDirectory: (path: string) => Promise<boolean>
  deleteFile: (path: string) => Promise<boolean>
  initializeGit: () => Promise<InitializeResult | null>

  // Helpers
  getFileStatus: (path: string) => FileStatus | undefined
}

const GitOpsContext = createContext<GitOpsContextValue | null>(null)

export function useGitOps(): GitOpsContextValue {
  const context = useContext(GitOpsContext)
  if (!context) {
    throw new Error('useGitOps must be used within a GitOpsProvider')
  }
  return context
}

interface GitOpsProviderProps {
  children: React.ReactNode
}

export function GitOpsProvider({ children }: GitOpsProviderProps) {
  const [fileTree, setFileTree] = useState<FileTreeNode | null>(null)
  const [gitStatus, setGitStatus] = useState<GitStatus | null>(null)
  const [branches, setBranches] = useState<BranchInfo[]>([])
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [selectedResource, setSelectedResource] = useState<ResourceDetail | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [isConnected, setIsConnected] = useState(true) // Always connected in Tauri
  const [settings, setSettings] = useState<SettingsResource | null>(null)
  const [initialLoadComplete, setInitialLoadComplete] = useState(false)

  const unlistenRef = useRef<UnlistenFn | null>(null)

  // Auto-clear errors after 10 seconds
  useEffect(() => {
    if (!error) return
    const timer = setTimeout(() => setError(null), 10000)
    return () => clearTimeout(timer)
  }, [error])

  // Only claim "needs initialization" when all initial data is loaded
  const needsInitialization = Boolean(
    initialLoadComplete &&
    settings?.spec.git.enabled &&
    settings?.spec.git.repository &&
    gitStatus &&
    !gitStatus.isRepo
  )

  // Define callback functions first (before refs that reference them)
  const refreshTree = useCallback(async () => {
    try {
      const data = await api.gitops.getFileTree()
      setFileTree(data as FileTreeNode)
    } catch (e) {
      console.error('Failed to load file tree:', e)
    }
  }, [])

  const refreshGitStatus = useCallback(async () => {
    try {
      const data = await api.git.getStatus()
      setGitStatus(data as GitStatus)
    } catch (e) {
      console.error('Failed to load git status:', e)
    }
  }, [])

  const refreshBranches = useCallback(async () => {
    try {
      const data = await api.git.getBranches()
      setBranches(data as BranchInfo[])
    } catch (e) {
      console.error('Failed to load branches:', e)
    }
  }, [])

  // Refs to hold latest callback functions for event listener
  const refreshTreeRef = useRef(refreshTree)
  const refreshGitStatusRef = useRef(refreshGitStatus)

  // Keep refs updated when callbacks change
  useEffect(() => {
    refreshTreeRef.current = refreshTree
    refreshGitStatusRef.current = refreshGitStatus
  }, [refreshTree, refreshGitStatus])

  // Listen for Tauri config change events
  useEffect(() => {
    const setupListener = async () => {
      try {
        const unlisten = await listen<ConfigChangeEvent>('paporg://config-changed', (event) => {
          console.log('Config changed:', event.payload)
          // Refresh on config changes using refs to get latest callbacks
          refreshTreeRef.current()
          refreshGitStatusRef.current()
        })
        unlistenRef.current = unlisten
        setIsConnected(true)
      } catch (e) {
        console.error('Failed to set up event listener:', e)
        setIsConnected(false)
      }
    }

    setupListener()

    return () => {
      unlistenRef.current?.()
    }
  }, [])

  // Initial load — await all before marking complete
  useEffect(() => {
    const init = async () => {
      // Ensure backend config is loaded
      try {
        await api.config.reload()
      } catch (e) {
        console.warn('Failed to reload config on init:', e)
      }

      // Then refresh all data and wait for completion
      await Promise.allSettled([
        refreshTree(),
        refreshGitStatus(),
        refreshBranches(),
        refreshSettings(),
      ])
      setInitialLoadComplete(true)
    }
    init()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const refreshSettings = useCallback(async () => {
    try {
      // Get all settings resources
      const resources = await api.gitops.listResources('Settings')
      if (resources.length > 0) {
        const settingsResource = await api.gitops.getResource('Settings', resources[0].name)
        // Parse the YAML to get settings
        try {
          const parsed = yaml.load(settingsResource.yaml) as SettingsResource
          setSettings(parsed)
        } catch (e) {
          console.error('Failed to parse settings YAML:', e)
        }
      }
    } catch (e) {
      console.error('Failed to load settings:', e)
    }
  }, [])

  const selectFile = useCallback(async (path: string) => {
    setSelectedPath(path)
    setIsLoading(true)
    setError(null)

    try {
      // Find the resource info from the tree
      const findResource = (node: FileTreeNode): { kind: ResourceKind; name: string } | null => {
        if (node.path === path && node.resource) {
          return { kind: node.resource.kind, name: node.resource.name }
        }
        if (node.children) {
          for (const child of node.children) {
            const found = findResource(child)
            if (found) return found
          }
        }
        return null
      }

      if (!fileTree) {
        setError('File tree not loaded')
        return
      }

      const resourceInfo = findResource(fileTree)
      if (!resourceInfo) {
        // Not a resource file, try to read raw
        try {
          const content = await api.files.readRaw(path)
          setSelectedResource({
            name: path.split('/').pop() || path,
            path,
            yaml: content,
          })
        } catch {
          setError('File not found or not a valid resource')
        }
        return
      }

      try {
        const data = await api.gitops.getResource(resourceInfo.kind, resourceInfo.name)
        setSelectedResource({
          name: data.name,
          path: data.path,
          yaml: data.yaml,
        })
      } catch {
        // If getResource fails (e.g., config not loaded), try reading raw file
        try {
          const content = await api.files.readRaw(path)
          setSelectedResource({
            name: resourceInfo.name,
            path,
            yaml: content,
          })
        } catch {
          setError('Failed to load resource')
        }
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load resource')
    } finally {
      setIsLoading(false)
    }
  }, [fileTree])

  const createResource = useCallback(async (kind: ResourceKind, yamlContent: string, path?: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      // Extract name from the YAML
      const parsed = yaml.load(yamlContent) as { metadata?: { name?: string } }
      const name = parsed?.metadata?.name
      if (!name) {
        setError('Resource must have a metadata.name field')
        return false
      }

      await api.gitops.createResource(kind, name, yamlContent, path)
      await refreshTree()
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to create resource')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus])

  const updateResource = useCallback(async (kind: ResourceKind, name: string, yamlContent: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      await api.gitops.updateResource(kind, name, yamlContent)
      // Refresh the selected resource
      const data = await api.gitops.getResource(kind, name)
      setSelectedResource({
        name: data.name,
        path: data.path,
        yaml: data.yaml,
      })
      await refreshTree()
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to update resource')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus])

  const deleteResourceAction = useCallback(async (kind: ResourceKind, name: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      await api.gitops.deleteResource(kind, name)
      setSelectedPath(null)
      setSelectedResource(null)
      await refreshTree()
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete resource')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus])

  const gitPullAction = useCallback(async (): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      await api.git.pull()
      await refreshTree()
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Git pull failed')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus])

  const gitCommitAction = useCallback(async (message: string, files?: string[]): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      await api.git.commit(message, files)
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Git commit failed')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshGitStatus])

  const checkoutBranchAction = useCallback(async (branch: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      await api.git.checkout(branch)
      await refreshTree()
      await refreshGitStatus()
      await refreshBranches()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Branch checkout failed')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus, refreshBranches])

  const createBranchAction = useCallback(async (name: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      await api.git.createBranch(name, true)
      await refreshTree()
      await refreshGitStatus()
      await refreshBranches()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Branch creation failed')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus, refreshBranches])

  const initializeGitAction = useCallback(async (): Promise<InitializeResult | null> => {
    setIsLoading(true)
    setError(null)

    try {
      const result = await api.git.initialize()
      await refreshTree()
      await refreshGitStatus()
      await refreshBranches()
      return result as InitializeResult
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Git initialization failed')
      return null
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus, refreshBranches])

  // Memoize file status lookup data to avoid O(n²) rebuilds
  const fileStatusMap = useMemo(() => {
    if (!gitStatus?.files) return new Map<string, FileStatus>()

    const map = new Map<string, FileStatus>()
    for (const f of gitStatus.files) {
      let status: FileStatus['status'] = 'modified'
      if (f.status === '?') status = 'untracked'
      else if (f.status === 'A') status = 'added'
      else if (f.status === 'D') status = 'deleted'
      else if (f.staged) status = 'staged'
      map.set(f.path, { path: f.path, status })
    }
    return map
  }, [gitStatus])

  const getFileStatus = useCallback((path: string): FileStatus | undefined => {
    if (!gitStatus) return undefined

    // Try exact match first using the memoized map
    const exactMatch = fileStatusMap.get(path)
    if (exactMatch) return exactMatch

    // Fallback: Try matching with path ending (for relative paths)
    for (const [filePath, status] of fileStatusMap) {
      if (filePath.endsWith(path) || path.endsWith(filePath)) {
        return status
      }
    }
    return undefined
  }, [gitStatus, fileStatusMap])

  const moveFileAction = useCallback(async (source: string, destination: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      const result = await api.files.move(source, destination)
      if (!result.success) {
        setError(result.error || 'Failed to move file')
        return false
      }
      await refreshTree()
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to move file')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus])

  const createDirectoryAction = useCallback(async (path: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      const result = await api.files.createDirectory(path)
      if (!result.success) {
        setError(result.error || 'Failed to create directory')
        return false
      }
      await refreshTree()
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to create directory')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [refreshTree, refreshGitStatus])

  const deleteFileAction = useCallback(async (path: string): Promise<boolean> => {
    setIsLoading(true)
    setError(null)

    try {
      const result = await api.files.delete(path)
      if (!result.success) {
        setError(result.error || 'Failed to delete file')
        return false
      }
      if (selectedPath === path) {
        setSelectedPath(null)
        setSelectedResource(null)
      }
      await refreshTree()
      await refreshGitStatus()
      return true
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete file')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [selectedPath, refreshTree, refreshGitStatus])

  const value: GitOpsContextValue = {
    fileTree,
    gitStatus,
    branches,
    selectedPath,
    selectedResource,
    isLoading,
    error,
    isConnected,
    settings,
    needsInitialization,
    initialLoadComplete,
    refreshTree,
    refreshGitStatus,
    refreshBranches,
    selectFile,
    createResource,
    updateResource,
    deleteResource: deleteResourceAction,
    gitPull: gitPullAction,
    gitCommit: gitCommitAction,
    checkoutBranch: checkoutBranchAction,
    createBranch: createBranchAction,
    moveFile: moveFileAction,
    createDirectory: createDirectoryAction,
    deleteFile: deleteFileAction,
    initializeGit: initializeGitAction,
    getFileStatus,
  }

  return <GitOpsContext.Provider value={value}>{children}</GitOpsContext.Provider>
}
