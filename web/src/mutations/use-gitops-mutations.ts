import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/api'
import yaml from 'js-yaml'
import type { ResourceKind, InitializeResult } from '@/types/gitops'

// Shared invalidation key sets
const FILE_TREE_KEYS = ['gitops', 'file-tree'] as const
const GIT_STATUS_KEYS = ['git', 'status'] as const
const BRANCHES_KEYS = ['git', 'branches'] as const
const SETTINGS_KEYS = ['gitops', 'settings'] as const
const RESOURCES_KEYS = ['gitops', 'resources'] as const

export function useCreateResource() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({
      kind,
      yamlContent,
      path,
    }: {
      kind: ResourceKind
      yamlContent: string
      path?: string
    }) => {
      const parsed = yaml.load(yamlContent) as { metadata?: { name?: string } }
      const name = parsed?.metadata?.name
      if (!name) throw new Error('Resource must have a metadata.name field')
      await api.gitops.createResource(kind, name, yamlContent, path)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
      qc.invalidateQueries({ queryKey: [...RESOURCES_KEYS] })
    },
  })
}

export function useUpdateResource() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({
      kind,
      name,
      yamlContent,
    }: {
      kind: string
      name: string
      yamlContent: string
    }) => {
      await api.gitops.updateResource(kind, name, yamlContent)
    },
    onSuccess: (_data, variables) => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
      qc.invalidateQueries({ queryKey: [...RESOURCES_KEYS] })
      qc.invalidateQueries({
        queryKey: ['gitops', 'resource', variables.kind, variables.name],
      })
    },
  })
}

export function useDeleteResource() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ kind, name }: { kind: string; name: string }) => {
      await api.gitops.deleteResource(kind, name)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
      qc.invalidateQueries({ queryKey: [...RESOURCES_KEYS] })
    },
  })
}

export function useGitCommit() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({
      message,
      files,
    }: {
      message: string
      files?: string[]
    }) => {
      return api.git.commit(message, files)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
    },
  })
}

export function useGitPull() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async () => {
      return api.git.pull()
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
    },
  })
}

export function useCheckoutBranch() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ branch }: { branch: string }) => {
      await api.git.checkout(branch)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
      qc.invalidateQueries({ queryKey: [...BRANCHES_KEYS] })
    },
  })
}

export function useCreateBranch() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ name }: { name: string }) => {
      await api.git.createBranch(name, true)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
      qc.invalidateQueries({ queryKey: [...BRANCHES_KEYS] })
    },
  })
}

export function useInitializeGit() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (): Promise<InitializeResult> => {
      return api.git.initialize()
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
      qc.invalidateQueries({ queryKey: [...BRANCHES_KEYS] })
      qc.invalidateQueries({ queryKey: [...SETTINGS_KEYS] })
    },
  })
}

export function useMoveFile() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({
      source,
      destination,
    }: {
      source: string
      destination: string
    }) => {
      const result = await api.files.move(source, destination)
      if (!result.success) throw new Error(result.error || 'Failed to move file')
      return result
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
    },
  })
}

export function useCreateDirectory() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ path }: { path: string }) => {
      const result = await api.files.createDirectory(path)
      if (!result.success)
        throw new Error(result.error || 'Failed to create directory')
      return result
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
    },
  })
}

export function useDeleteFile() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ path }: { path: string }) => {
      const result = await api.files.delete(path)
      if (!result.success)
        throw new Error(result.error || 'Failed to delete file')
      return result
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...FILE_TREE_KEYS] })
      qc.invalidateQueries({ queryKey: [...GIT_STATUS_KEYS] })
    },
  })
}
