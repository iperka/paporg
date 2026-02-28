import Database from '@tauri-apps/plugin-sql';
import { invoke } from '@tauri-apps/api/core';
import type { ApiResponse, StoredJob, JobQueryParams, JobListResponse } from '@/api/index';

let db: Database | null = null;
let dbPromise: Promise<Database> | null = null;

async function resolveDatabasePath(): Promise<string> {
  const response = await invoke<ApiResponse<{ path: string }>>('get_database_path');
  if (!response.success || !response.data) {
    throw new Error(response.error || 'Could not determine database path');
  }
  return response.data.path;
}

export async function getDatabase(): Promise<Database> {
  // Return existing instance if available
  if (db) return db;

  // Return in-flight promise if loading is in progress (prevents race conditions)
  if (dbPromise) return dbPromise;

  // Resolve the database path from the backend, then load
  dbPromise = resolveDatabasePath().then((dbPath) =>
    Database.load(`sqlite:${dbPath}`)
  );

  try {
    db = await dbPromise;
    return db;
  } finally {
    dbPromise = null;
  }
}

export async function closeDatabase(): Promise<void> {
  // Wait for any in-flight load to complete
  if (dbPromise) {
    try {
      await dbPromise;
    } catch {
      // Ignore errors during close
    }
  }

  if (db) {
    await db.close();
    db = null;
  }
}

// Raw database row types (snake_case from SQLite)
interface JobRow {
  id: string;
  filename: string;
  source_path: string;
  source_name: string | null;
  mime_type: string | null;
  status: string;
  category: string | null;
  output_path: string | null;
  archive_path: string | null;
  symlinks: string | null;
  error: string | null;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
  current_phase: string | null;
  message: string | null;
}

interface OAuthTokenRow {
  source_name: string;
  provider: string;
  access_token: string;
  refresh_token: string | null;
  expires_at: string;
  created_at: string;
  updated_at: string;
}

interface ProcessedEmailRow {
  id: string;
  source_name: string;
  uidvalidity: number;
  uid: number;
  message_id: string | null;
  processed_at: string;
}

// Safely parse JSON with a default fallback
function parseJsonSafe<T>(json: string | null, defaultValue: T): T {
  if (!json) return defaultValue;
  try {
    return JSON.parse(json) as T;
  } catch (e) {
    console.warn('Failed to parse JSON:', e, 'Value:', json);
    return defaultValue;
  }
}

// Transform database row to StoredJob
function rowToStoredJob(row: JobRow): StoredJob {
  return {
    id: row.id,
    filename: row.filename,
    sourcePath: row.source_path,
    sourceName: row.source_name,
    mimeType: row.mime_type,
    status: row.status,
    category: row.category,
    outputPath: row.output_path,
    archivePath: row.archive_path,
    symlinks: parseJsonSafe<string[]>(row.symlinks, []),
    errorMessage: row.error,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

// =============================================================================
// Job Queries
// =============================================================================

export async function getAllJobs(): Promise<StoredJob[]> {
  const database = await getDatabase();
  const rows = await database.select<JobRow[]>(
    'SELECT * FROM jobs ORDER BY created_at DESC'
  );
  return rows.map(rowToStoredJob);
}

export async function getJob(jobId: string): Promise<StoredJob | null> {
  const database = await getDatabase();
  const rows = await database.select<JobRow[]>(
    'SELECT * FROM jobs WHERE id = $1',
    [jobId]
  );
  return rows.length > 0 ? rowToStoredJob(rows[0]) : null;
}

export async function queryJobs(params: JobQueryParams): Promise<JobListResponse> {
  const database = await getDatabase();

  const conditions: string[] = [];
  const queryParams: (string | number)[] = [];
  let paramIndex = 1;

  if (params.status) {
    conditions.push(`status = $${paramIndex}`);
    queryParams.push(params.status);
    paramIndex++;
  }

  if (params.category) {
    conditions.push(`category = $${paramIndex}`);
    queryParams.push(params.category);
    paramIndex++;
  }

  if (params.search) {
    conditions.push(`(filename LIKE $${paramIndex} OR source_path LIKE $${paramIndex})`);
    queryParams.push(`%${params.search}%`);
    paramIndex++;
  }

  const whereClause = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  // Get total count
  const countResult = await database.select<{ count: number }[]>(
    `SELECT COUNT(*) as count FROM jobs ${whereClause}`,
    queryParams
  );
  const total = countResult[0]?.count ?? 0;

  // Build ORDER BY clause
  const sortBy = params.sortBy || 'created_at';
  const sortOrder = params.sortOrder === 'asc' ? 'ASC' : 'DESC';
  const validColumns = ['created_at', 'updated_at', 'filename', 'status', 'category'];
  const orderColumn = validColumns.includes(sortBy) ? sortBy : 'created_at';

  // Pagination
  const page = params.page ?? 1;
  const pageSize = params.pageSize ?? 20;
  const offset = (page - 1) * pageSize;

  const rows = await database.select<JobRow[]>(
    `SELECT * FROM jobs ${whereClause} ORDER BY ${orderColumn} ${sortOrder} LIMIT $${paramIndex} OFFSET $${paramIndex + 1}`,
    [...queryParams, pageSize, offset]
  );

  return {
    jobs: rows.map(rowToStoredJob),
    total,
    page,
    pageSize,
  };
}

export async function insertJob(job: StoredJob): Promise<void> {
  const database = await getDatabase();
  await database.execute(
    `INSERT INTO jobs (id, filename, source_path, source_name, mime_type, status, category, output_path, archive_path, symlinks, error, created_at, updated_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)`,
    [
      job.id,
      job.filename,
      job.sourcePath,
      job.sourceName,
      job.mimeType,
      job.status,
      job.category,
      job.outputPath,
      job.archivePath,
      JSON.stringify(job.symlinks),
      job.errorMessage,
      job.createdAt,
      job.updatedAt,
    ]
  );
}

export async function updateJob(jobId: string, updates: Partial<StoredJob>): Promise<void> {
  const database = await getDatabase();
  const setClauses: string[] = [];
  const queryParams: (string | number | null)[] = [];
  let paramIndex = 1;

  if (updates.status !== undefined) {
    setClauses.push(`status = $${paramIndex}`);
    queryParams.push(updates.status);
    paramIndex++;
  }
  if (updates.category !== undefined) {
    setClauses.push(`category = $${paramIndex}`);
    queryParams.push(updates.category);
    paramIndex++;
  }
  if (updates.outputPath !== undefined) {
    setClauses.push(`output_path = $${paramIndex}`);
    queryParams.push(updates.outputPath);
    paramIndex++;
  }
  if (updates.archivePath !== undefined) {
    setClauses.push(`archive_path = $${paramIndex}`);
    queryParams.push(updates.archivePath);
    paramIndex++;
  }
  if (updates.symlinks !== undefined) {
    setClauses.push(`symlinks = $${paramIndex}`);
    queryParams.push(JSON.stringify(updates.symlinks));
    paramIndex++;
  }
  if (updates.errorMessage !== undefined) {
    setClauses.push(`error = $${paramIndex}`);
    queryParams.push(updates.errorMessage);
    paramIndex++;
  }
  if (updates.mimeType !== undefined) {
    setClauses.push(`mime_type = $${paramIndex}`);
    queryParams.push(updates.mimeType);
    paramIndex++;
  }

  // Return early if no actual updates (only updated_at would be pointless)
  if (setClauses.length === 0) return;

  // Always update updated_at when there are actual changes
  setClauses.push(`updated_at = $${paramIndex}`);
  queryParams.push(new Date().toISOString());
  paramIndex++;

  queryParams.push(jobId);
  await database.execute(
    `UPDATE jobs SET ${setClauses.join(', ')} WHERE id = $${paramIndex}`,
    queryParams
  );
}

export async function deleteJob(jobId: string): Promise<void> {
  const database = await getDatabase();
  await database.execute('DELETE FROM jobs WHERE id = $1', [jobId]);
}

// =============================================================================
// OAuth Token Queries
// =============================================================================

export interface OAuthToken {
  sourceName: string;
  provider: string;
  accessToken: string;
  refreshToken: string | null;
  expiresAt: string;
  createdAt: string;
  updatedAt: string;
}

function rowToOAuthToken(row: OAuthTokenRow): OAuthToken {
  return {
    sourceName: row.source_name,
    provider: row.provider,
    accessToken: row.access_token,
    refreshToken: row.refresh_token,
    expiresAt: row.expires_at,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

export async function getOAuthToken(sourceName: string): Promise<OAuthToken | null> {
  const database = await getDatabase();
  const rows = await database.select<OAuthTokenRow[]>(
    'SELECT * FROM oauth_tokens WHERE source_name = $1',
    [sourceName]
  );
  return rows.length > 0 ? rowToOAuthToken(rows[0]) : null;
}

export async function upsertOAuthToken(token: OAuthToken): Promise<void> {
  const database = await getDatabase();
  await database.execute(
    `INSERT INTO oauth_tokens (source_name, provider, access_token, refresh_token, expires_at, created_at, updated_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7)
     ON CONFLICT(source_name) DO UPDATE SET
       provider = $2,
       access_token = $3,
       refresh_token = $4,
       expires_at = $5,
       updated_at = $7`,
    [
      token.sourceName,
      token.provider,
      token.accessToken,
      token.refreshToken,
      token.expiresAt,
      token.createdAt,
      token.updatedAt,
    ]
  );
}

export async function deleteOAuthToken(sourceName: string): Promise<void> {
  const database = await getDatabase();
  await database.execute('DELETE FROM oauth_tokens WHERE source_name = $1', [sourceName]);
}

// =============================================================================
// Processed Email Queries
// =============================================================================

export interface ProcessedEmail {
  id: string;
  sourceName: string;
  uidvalidity: number;
  uid: number;
  messageId: string | null;
  processedAt: string;
}

function rowToProcessedEmail(row: ProcessedEmailRow): ProcessedEmail {
  return {
    id: row.id,
    sourceName: row.source_name,
    uidvalidity: row.uidvalidity,
    uid: row.uid,
    messageId: row.message_id,
    processedAt: row.processed_at,
  };
}

export async function isEmailProcessed(
  sourceName: string,
  uidvalidity: number,
  uid: number
): Promise<boolean> {
  const database = await getDatabase();
  const rows = await database.select<{ count: number }[]>(
    'SELECT COUNT(*) as count FROM processed_emails WHERE source_name = $1 AND uidvalidity = $2 AND uid = $3',
    [sourceName, uidvalidity, uid]
  );
  return (rows[0]?.count ?? 0) > 0;
}

export async function markEmailProcessed(email: ProcessedEmail): Promise<void> {
  const database = await getDatabase();
  await database.execute(
    `INSERT INTO processed_emails (id, source_name, uidvalidity, uid, message_id, processed_at)
     VALUES ($1, $2, $3, $4, $5, $6)
     ON CONFLICT(source_name, uidvalidity, uid) DO NOTHING`,
    [
      email.id,
      email.sourceName,
      email.uidvalidity,
      email.uid,
      email.messageId,
      email.processedAt,
    ]
  );
}

export async function getProcessedEmails(sourceName: string): Promise<ProcessedEmail[]> {
  const database = await getDatabase();
  const rows = await database.select<ProcessedEmailRow[]>(
    'SELECT * FROM processed_emails WHERE source_name = $1 ORDER BY processed_at DESC',
    [sourceName]
  );
  return rows.map(rowToProcessedEmail);
}
