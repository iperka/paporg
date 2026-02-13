import { useMemo } from 'react'
import { AlertTriangle, CheckCircle2, XCircle, Loader2, Link2, Clock, Plus, RefreshCw, FileType } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import type { StoredJob } from '@/types/jobs'
import { getPhaseLabel, getMimeTypeLabel } from '@/types/jobs'
import { isUnsortedJob } from '@/utils/jobs'

/** Shared empty Set to avoid creating new instances on each render */
const EMPTY_RERUNNING_SET: Set<string> = new Set()

interface JobsTableProps {
  jobs: StoredJob[]
  emptyMessage?: string
  showCreateRuleButton?: boolean
  showRerunButton?: boolean
  onCreateRule?: (job: StoredJob) => void
  onRerun?: (job: StoredJob) => void
  rerunningJobIds?: Set<string>
}

function StatusBadge({ job }: { job: StoredJob }) {
  switch (job.status) {
    case 'processing':
      return (
        <Badge variant="warning" className="gap-1">
          <Loader2 className="h-3 w-3 animate-spin" />
          Processing
        </Badge>
      )
    case 'completed':
      return (
        <Badge variant="success" className="gap-1">
          <CheckCircle2 className="h-3 w-3" />
          Completed
        </Badge>
      )
    case 'failed':
      return (
        <Badge variant="destructive" className="gap-1">
          <XCircle className="h-3 w-3" />
          Failed
        </Badge>
      )
    case 'superseded':
      return (
        <Badge variant="outline" className="gap-1 text-muted-foreground">
          Superseded
        </Badge>
      )
    default:
      return <Badge variant="outline">{job.status}</Badge>
  }
}

function ResultCell({ job }: { job: StoredJob }) {
  if (job.status === 'processing') {
    return (
      <span className="text-muted-foreground text-sm">{getPhaseLabel(job.currentPhase)}</span>
    )
  }

  if (job.status === 'failed') {
    return <span className="text-destructive text-sm">{job.error || 'Unknown error'}</span>
  }

  if (job.status === 'completed') {
    const unsorted = isUnsortedJob(job)
    return (
      <div className="space-y-1">
        {job.outputPath && (
          <div className="text-sm font-mono truncate max-w-md" title={job.outputPath}>
            {job.outputPath}
          </div>
        )}
        {job.category && (
          unsorted ? (
            <Badge className="text-xs gap-1 bg-amber-500 text-white hover:bg-amber-600">
              <AlertTriangle className="h-3 w-3" />
              Needs Rule
            </Badge>
          ) : (
            <Badge variant="outline" className="text-xs">
              {job.category}
            </Badge>
          )
        )}
      </div>
    )
  }

  return null
}

function SymlinksCell({ job }: { job: StoredJob }) {
  if (!job.symlinks || job.symlinks.length === 0) {
    return <span className="text-muted-foreground">-</span>
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className="flex items-center gap-1 cursor-help">
          <Link2 className="h-4 w-4 text-muted-foreground" />
          <span>{job.symlinks.length}</span>
        </div>
      </TooltipTrigger>
      <TooltipContent side="left" className="max-w-lg">
        <div className="space-y-1">
          <div className="font-semibold">Symlinks</div>
          {job.symlinks.map((link, i) => (
            <div key={i} className="font-mono text-xs break-all">
              {link}
            </div>
          ))}
        </div>
      </TooltipContent>
    </Tooltip>
  )
}

function TimeCell({ job }: { job: StoredJob }) {
  const date = new Date(job.startedAt)
  const timeStr = date.toLocaleTimeString()
  const dateStr = date.toLocaleDateString()

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className="flex items-center gap-1 text-sm text-muted-foreground cursor-help">
          <Clock className="h-4 w-4" />
          {timeStr}
        </div>
      </TooltipTrigger>
      <TooltipContent>
        <div>{dateStr}</div>
      </TooltipContent>
    </Tooltip>
  )
}

export function JobsTable({
  jobs,
  emptyMessage = 'No jobs to display',
  showCreateRuleButton = false,
  showRerunButton = false,
  onCreateRule,
  onRerun,
  rerunningJobIds = EMPTY_RERUNNING_SET,
}: JobsTableProps) {
  // Move useMemo before early return to ensure hooks are always called in the same order
  const showActionsColumn = useMemo(
    () => showCreateRuleButton || showRerunButton || jobs.some(job => isUnsortedJob(job)),
    [showCreateRuleButton, showRerunButton, jobs]
  )

  if (jobs.length === 0) {
    return (
      <div className="text-center py-8 text-muted-foreground border rounded-lg">{emptyMessage}</div>
    )
  }

  return (
    <TooltipProvider>
      <div className="border rounded-lg">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[200px]">Filename</TableHead>
              <TableHead className="w-[80px]">Type</TableHead>
              <TableHead className="w-[120px]">Status</TableHead>
              <TableHead>Result / Step</TableHead>
              <TableHead className="w-[80px]">Symlinks</TableHead>
              <TableHead className="w-[100px]">Time</TableHead>
              {showActionsColumn && <TableHead className="w-[200px]">Actions</TableHead>}
            </TableRow>
          </TableHeader>
          <TableBody>
            {jobs.map((job) => {
              const unsorted = isUnsortedJob(job)
              const canCreateRule = job.status === 'completed' && unsorted && onCreateRule
              const canRerun = job.status === 'completed' && job.archivePath && onRerun
              const isRerunning = rerunningJobIds.has(job.jobId)

              return (
                <TableRow
                  key={job.jobId}
                  className={unsorted ? 'bg-amber-50/50 dark:bg-amber-950/20' : undefined}
                >
                  <TableCell className="font-medium truncate max-w-[200px]" title={job.filename}>
                    {job.filename}
                  </TableCell>
                  <TableCell>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <div className="flex items-center gap-1 text-sm text-muted-foreground cursor-help">
                          <FileType className="h-4 w-4" />
                          {getMimeTypeLabel(job.mimeType)}
                        </div>
                      </TooltipTrigger>
                      <TooltipContent>
                        <div>{job.mimeType || 'Unknown'}</div>
                      </TooltipContent>
                    </Tooltip>
                  </TableCell>
                <TableCell>
                  <StatusBadge job={job} />
                </TableCell>
                <TableCell>
                  <ResultCell job={job} />
                </TableCell>
                <TableCell>
                  <SymlinksCell job={job} />
                </TableCell>
                <TableCell>
                  <TimeCell job={job} />
                </TableCell>
                {showActionsColumn && (
                  <TableCell>
                    <div className="flex gap-2">
                      {canRerun && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => onRerun(job)}
                          disabled={isRerunning}
                          className="gap-1"
                        >
                          <RefreshCw className={`h-3 w-3 ${isRerunning ? 'animate-spin' : ''}`} />
                          Re-run
                        </Button>
                      )}
                      {canCreateRule && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => onCreateRule(job)}
                          className="gap-1"
                        >
                          <Plus className="h-3 w-3" />
                          Create Rule
                        </Button>
                      )}
                    </div>
                  </TableCell>
                )}
              </TableRow>
            )
          })}
          </TableBody>
        </Table>
      </div>
    </TooltipProvider>
  )
}
