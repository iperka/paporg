/**
 * Unified Tauri API client for the Paporg desktop application.
 *
 * This is the main entry point for all API calls. Since we're now Tauri-only,
 * all calls go through Tauri IPC commands.
 */

import { invoke } from '@tauri-apps/api/core';

// =============================================================================
// Response Types
// =============================================================================

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

// Config types
export interface ConfigSummary {
  configDir: string | null;
  inputDirectory: string | null;
  outputDirectory: string | null;
  workerCount: number | null;
  rulesCount: number;
  importSourcesCount: number;
  ocrEnabled: boolean;
}

export interface HealthStatus {
  status: string;
  version: string;
  configLoaded: boolean;
  workersRunning: boolean;
}

// Worker types
export interface WorkerStatus {
  running: boolean;
  workerCount: number;
}

// Job types
export interface StoredJob {
  id: string;
  filename: string;
  sourcePath: string;
  sourceName: string | null;
  mimeType: string | null;
  status: string;
  category: string | null;
  outputPath: string | null;
  archivePath: string | null;
  symlinks: string[];
  errorMessage: string | null;
  ocrText: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface JobListResponse {
  jobs: StoredJob[];
  total: number;
  page: number;
  pageSize: number;
}

export interface JobQueryParams {
  page?: number;
  pageSize?: number;
  status?: string;
  category?: string;
  search?: string;
  sortBy?: string;
  sortOrder?: string;
}

export interface OcrResponse {
  text: string;
  cached: boolean;
}

export interface RerunResponse {
  success: boolean;
  newJobId: string;
  message: string;
}

// GitOps types
export interface FileTreeNode {
  name: string;
  path: string;
  isDirectory: boolean;
  children?: FileTreeNode[];
}

export interface ResourceInfo {
  kind: string;
  name: string;
  path: string;
  yaml: string;
}

export interface ValidationResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
}

export interface SimulationResult {
  category: string;
  outputDirectory: string;
  outputFilename: string;
  symlinks: string[];
  matchedRule: string | null;
}

// Git types
export interface GitFileStatus {
  path: string;
  status: string; // 'M' (modified), 'A' (added), 'D' (deleted), '?' (untracked), 'R' (renamed)
  staged: boolean;
}

export interface GitStatus {
  isRepo: boolean;
  branch?: string;
  isClean: boolean;
  ahead: number;
  behind: number;
  modifiedFiles: string[];
  untrackedFiles: string[];
  files: GitFileStatus[];
}

export interface PullResult {
  success: boolean;
  filesChanged: number;
  message: string;
}

export interface CommitResult {
  success: boolean;
  commitHash: string | null;
  pushed: boolean;
  message: string;
}

export interface BranchInfo {
  name: string;
  isCurrent: boolean;
  isRemote: boolean;
}

export interface CommitInfo {
  hash: string;
  author: string;
  date: string;
  message: string;
}

export interface MergeStatus {
  ahead: number;
  behind: number;
  hasConflicts: boolean;
  conflictingFiles: string[];
}

export interface InitializeResult {
  initialized: boolean;
  merged: boolean;
  message: string;
  conflictingFiles: string[];
}

// Email OAuth types
export interface DeviceCodeResponse {
  userCode: string;
  verificationUri: string;
  verificationUriComplete: string | null;
  expiresIn: number;
  interval: number;
}

export interface AuthorizationStatusResponse {
  status: string;
  message: string;
}

export interface TokenStatusResponse {
  hasToken: boolean;
  expiresAt: string | null;
  isValid: boolean;
  provider: string | null;
}

// Secret types
export interface WriteSecretResponse {
  filePath: string;
}

// AI types
export interface AiStatusResponse {
  available: boolean;
  modelLoaded: boolean;
  modelName: string | null;
  error: string | null;
}

export interface DownloadProgress {
  status: string;
  progress: number | null;
  message: string;
}

export interface RuleSuggestion {
  name: string;
  category: string;
  pattern: string;
  confidence: number;
  explanation: string;
}

// File types
export interface FileResponse {
  success: boolean;
  path: string;
  error: string | null;
}

export interface UploadResult {
  success: boolean;
  filesUploaded: number;
  uploadedPaths: string[];
  errors: string[];
}

// =============================================================================
// Helper Functions
// =============================================================================

/**
 * Helper to unwrap API responses and throw on errors.
 */
async function unwrap<T>(response: ApiResponse<T>): Promise<T> {
  if (!response.success) {
    throw new Error(response.error || 'Unknown error');
  }
  return response.data as T;
}

// =============================================================================
// API Client
// =============================================================================

export const api = {
  // ---------------------------------------------------------------------------
  // Config
  // ---------------------------------------------------------------------------
  config: {
    get: async (): Promise<ConfigSummary> => {
      const response = await invoke<ApiResponse<ConfigSummary>>('get_config');
      return unwrap(response);
    },

    reload: async (): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('reload_config');
      return unwrap(response);
    },

    selectDirectory: async (): Promise<string | null> => {
      const response = await invoke<ApiResponse<string | null>>('select_config_directory');
      return unwrap(response);
    },

    healthCheck: async (): Promise<HealthStatus> => {
      const response = await invoke<ApiResponse<HealthStatus>>('health_check');
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Workers
  // ---------------------------------------------------------------------------
  workers: {
    getStatus: async (): Promise<WorkerStatus> => {
      const response = await invoke<ApiResponse<WorkerStatus>>('get_worker_status');
      return unwrap(response);
    },

    start: async (): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('start_workers');
      return unwrap(response);
    },

    stop: async (): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('stop_workers');
      return unwrap(response);
    },

    triggerProcessing: async (): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('trigger_processing');
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Jobs
  // ---------------------------------------------------------------------------
  jobs: {
    getAll: async (): Promise<StoredJob[]> => {
      const response = await invoke<ApiResponse<StoredJob[]>>('get_jobs');
      return unwrap(response);
    },

    query: async (params: JobQueryParams): Promise<JobListResponse> => {
      const response = await invoke<ApiResponse<JobListResponse>>('query_jobs', { params });
      return unwrap(response);
    },

    get: async (jobId: string): Promise<StoredJob> => {
      const response = await invoke<ApiResponse<StoredJob>>('get_job', { jobId });
      return unwrap(response);
    },

    getOcr: async (jobId: string): Promise<OcrResponse> => {
      const response = await invoke<ApiResponse<OcrResponse>>('get_job_ocr', { jobId });
      return unwrap(response);
    },

    rerun: async (jobId: string, sourceName?: string): Promise<RerunResponse> => {
      const response = await invoke<ApiResponse<RerunResponse>>('rerun_job', { jobId, sourceName });
      return unwrap(response);
    },

    ignore: async (jobId: string): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('ignore_job', { jobId });
      return unwrap(response);
    },

    rerunUnsorted: async (): Promise<{ count: number }> => {
      const response = await invoke<ApiResponse<{ count: number }>>('rerun_unsorted');
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // GitOps Resources
  // ---------------------------------------------------------------------------
  gitops: {
    getFileTree: async (): Promise<FileTreeNode> => {
      const response = await invoke<ApiResponse<FileTreeNode>>('get_file_tree');
      return unwrap(response);
    },

    listResources: async (kind?: string): Promise<ResourceInfo[]> => {
      const response = await invoke<ApiResponse<ResourceInfo[]>>('list_gitops_resources', { kind });
      return unwrap(response);
    },

    getResource: async (kind: string, name: string): Promise<ResourceInfo> => {
      const response = await invoke<ApiResponse<ResourceInfo>>('get_gitops_resource', { kind, name });
      return unwrap(response);
    },

    createResource: async (kind: string, _name: string, yaml: string, path?: string): Promise<void> => {
      // Note: backend extracts name from YAML, _name kept for interface consistency
      const response = await invoke<ApiResponse<void>>('create_gitops_resource', { kind, yaml, path });
      return unwrap(response);
    },

    updateResource: async (kind: string, name: string, yaml: string): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('update_gitops_resource', { kind, name, yaml });
      return unwrap(response);
    },

    deleteResource: async (kind: string, name: string): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('delete_gitops_resource', { kind, name });
      return unwrap(response);
    },

    simulateRule: async (text: string, filename: string): Promise<SimulationResult> => {
      const response = await invoke<ApiResponse<SimulationResult>>('simulate_rule', { text, filename });
      return unwrap(response);
    },

    validateConfig: async (): Promise<ValidationResult> => {
      const response = await invoke<ApiResponse<ValidationResult>>('validate_config');
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Git Operations
  // ---------------------------------------------------------------------------
  git: {
    getStatus: async (): Promise<GitStatus> => {
      const response = await invoke<ApiResponse<GitStatus>>('git_status');
      return unwrap(response);
    },

    pull: async (): Promise<PullResult> => {
      const response = await invoke<ApiResponse<PullResult>>('git_pull');
      return unwrap(response);
    },

    commit: async (message: string, files?: string[]): Promise<CommitResult> => {
      const response = await invoke<ApiResponse<CommitResult>>('git_commit', { message, files });
      return unwrap(response);
    },

    getBranches: async (): Promise<BranchInfo[]> => {
      const response = await invoke<ApiResponse<BranchInfo[]>>('git_branches');
      return unwrap(response);
    },

    checkout: async (branch: string): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('git_checkout', { branch });
      return unwrap(response);
    },

    createBranch: async (name: string, checkout: boolean = false): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('git_create_branch', { name, checkout });
      return unwrap(response);
    },

    getMergeStatus: async (): Promise<MergeStatus> => {
      const response = await invoke<ApiResponse<MergeStatus>>('git_merge_status');
      return unwrap(response);
    },

    diff: async (file?: string, cached?: boolean): Promise<string> => {
      const response = await invoke<ApiResponse<string>>('git_diff', { file, cached });
      return unwrap(response);
    },

    log: async (limit?: number): Promise<CommitInfo[]> => {
      const response = await invoke<ApiResponse<CommitInfo[]>>('git_log', { limit });
      return unwrap(response);
    },

    cancelOperation: async (operationId: string): Promise<boolean> => {
      const response = await invoke<ApiResponse<boolean>>('git_cancel_operation', { operationId });
      return unwrap(response);
    },

    initialize: async (): Promise<InitializeResult> => {
      const response = await invoke<ApiResponse<InitializeResult>>('git_initialize');
      return unwrap(response);
    },

    disconnect: async (): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('git_disconnect');
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Email OAuth
  // ---------------------------------------------------------------------------
  email: {
    startAuthorization: async (sourceName: string): Promise<DeviceCodeResponse> => {
      const response = await invoke<ApiResponse<DeviceCodeResponse>>('start_email_authorization', { sourceName });
      return unwrap(response);
    },

    checkStatus: async (sourceName: string): Promise<AuthorizationStatusResponse> => {
      const response = await invoke<ApiResponse<AuthorizationStatusResponse>>('check_authorization_status', { sourceName });
      return unwrap(response);
    },

    getTokenStatus: async (sourceName: string): Promise<TokenStatusResponse> => {
      const response = await invoke<ApiResponse<TokenStatusResponse>>('get_token_status', { sourceName });
      return unwrap(response);
    },

    revokeToken: async (sourceName: string): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('revoke_token', { sourceName });
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Secrets
  // ---------------------------------------------------------------------------
  secrets: {
    write: async (sourceName: string, secretType: string, value: string): Promise<WriteSecretResponse> => {
      const response = await invoke<ApiResponse<WriteSecretResponse>>('write_secret', { sourceName, secretType, value });
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // AI
  // ---------------------------------------------------------------------------
  ai: {
    getStatus: async (): Promise<AiStatusResponse> => {
      const response = await invoke<ApiResponse<AiStatusResponse>>('ai_status');
      return unwrap(response);
    },

    downloadModel: async (modelId: string): Promise<DownloadProgress> => {
      const response = await invoke<ApiResponse<DownloadProgress>>('download_ai_model', { modelId });
      return unwrap(response);
    },

    suggestRule: async (ocrText: string, filename: string): Promise<RuleSuggestion> => {
      const response = await invoke<ApiResponse<RuleSuggestion>>('suggest_rule', { ocrText, filename });
      return unwrap(response);
    },

    suggestCommitMessage: async (files: [string, string][], diff: string): Promise<string> => {
      const response = await invoke<ApiResponse<string>>('suggest_commit_message', { files, diff });
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Analytics
  // ---------------------------------------------------------------------------
  analytics: {
    track: async (
      name: string,
      properties?: Record<string, string | number | boolean | null | undefined>,
    ): Promise<void> => {
      const response = await invoke<ApiResponse<void>>('track_event', { name, properties });
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Files
  // ---------------------------------------------------------------------------
  files: {
    move: async (source: string, destination: string): Promise<FileResponse> => {
      const response = await invoke<ApiResponse<FileResponse>>('move_file', { source, destination });
      return unwrap(response);
    },

    createDirectory: async (path: string): Promise<FileResponse> => {
      const response = await invoke<ApiResponse<FileResponse>>('create_directory', { path });
      return unwrap(response);
    },

    delete: async (path: string): Promise<FileResponse> => {
      const response = await invoke<ApiResponse<FileResponse>>('delete_file', { path });
      return unwrap(response);
    },

    readRaw: async (path: string): Promise<string> => {
      const response = await invoke<ApiResponse<string>>('read_raw_file', { path });
      return unwrap(response);
    },

    writeRaw: async (path: string, content: string): Promise<FileResponse> => {
      const response = await invoke<ApiResponse<FileResponse>>('write_raw_file', { path, content });
      return unwrap(response);
    },

    pickFolder: async (): Promise<string | null> => {
      const response = await invoke<ApiResponse<string | null>>('pick_folder');
      return unwrap(response);
    },

    pickFile: async (): Promise<string | null> => {
      const response = await invoke<ApiResponse<string | null>>('pick_file');
      return unwrap(response);
    },
  },

  // ---------------------------------------------------------------------------
  // Upload
  // ---------------------------------------------------------------------------
  upload: {
    uploadFiles: async (filePaths: string[]): Promise<UploadResult> => {
      const response = await invoke<ApiResponse<UploadResult>>('upload_files', { filePaths });
      return unwrap(response);
    },

    pickAndUpload: async (): Promise<UploadResult> => {
      const response = await invoke<ApiResponse<UploadResult>>('pick_and_upload_files');
      return unwrap(response);
    },
  },
};

export default api;
