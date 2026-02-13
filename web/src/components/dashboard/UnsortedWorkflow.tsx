import { useState, useCallback } from 'react'
import { Link } from '@tanstack/react-router'
import {
  AlertTriangle,
  CheckCircle2,
  EyeOff,
  FileText,
  Loader2,
  Plus,
  RefreshCw,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { CreateRuleFromJobDialog } from '@/components/jobs/CreateRuleFromJobDialog'
import { api } from '@/api'
import { toast } from '@/components/ui/use-toast'
import type { StoredJob } from '@/types/jobs'

interface UnsortedWorkflowProps {
  jobs: StoredJob[]
  rulesCount: number
  hasDocuments: boolean
  /** Callback when a job is successfully re-run */
  onJobRerun?: (jobId: string) => void
  /** Callback when a job is successfully ignored */
  onJobIgnored?: (jobId: string) => void
}

export function UnsortedWorkflow({ jobs, rulesCount, hasDocuments, onJobRerun, onJobIgnored }: UnsortedWorkflowProps) {
  const [selectedJob, setSelectedJob] = useState<StoredJob | null>(null)
  const [createRuleDialogOpen, setCreateRuleDialogOpen] = useState(false)
  const [rerunningJobIds, setRerunningJobIds] = useState<Set<string>>(new Set())
  const [ignoringJobIds, setIgnoringJobIds] = useState<Set<string>>(new Set())

  const handleCreateRule = (job: StoredJob) => {
    setSelectedJob(job)
    setCreateRuleDialogOpen(true)
  }

  const handleCloseDialog = () => {
    setCreateRuleDialogOpen(false)
    setSelectedJob(null)
  }

  const handleRerunJob = useCallback(async (job: StoredJob) => {
    setRerunningJobIds((prev) => new Set(prev).add(job.jobId))

    try {
      await api.jobs.rerun(job.jobId)
      toast({
        title: 'Job queued for re-run',
        description: `${job.filename} will be reprocessed`,
      })
      onJobRerun?.(job.jobId)
    } catch (e) {
      toast({
        title: 'Failed to re-run job',
        description: e instanceof Error ? e.message : 'An unknown error occurred',
        variant: 'destructive',
      })
    } finally {
      setRerunningJobIds((prev) => {
        const next = new Set(prev)
        next.delete(job.jobId)
        return next
      })
    }
  }, [onJobRerun])

  const handleIgnoreJob = useCallback(async (job: StoredJob) => {
    setIgnoringJobIds((prev) => new Set(prev).add(job.jobId))

    try {
      await api.jobs.ignore(job.jobId)
      toast({ title: 'Job ignored' })
      onJobIgnored?.(job.jobId)
    } catch (e) {
      toast({
        title: 'Failed to ignore job',
        description: e instanceof Error ? e.message : 'An unknown error occurred',
        variant: 'destructive',
      })
    } finally {
      setIgnoringJobIds((prev) => {
        const next = new Set(prev)
        next.delete(job.jobId)
        return next
      })
    }
  }, [onJobIgnored])

  // No documents at all
  if (!hasDocuments) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FileText className="h-5 w-5" />
            Unsorted Documents
          </CardTitle>
          <CardDescription>Documents that need rules for organization</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col items-center justify-center py-8 text-center">
            <FileText className="h-12 w-12 text-muted-foreground/50 mb-4" />
            <h3 className="font-semibold text-lg mb-1">No documents yet</h3>
            <p className="text-muted-foreground text-sm max-w-md">
              Drop files in your input directory to start processing. Documents will appear here
              once they've been scanned.
            </p>
          </div>
        </CardContent>
      </Card>
    )
  }

  // No rules exist
  if (rulesCount === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <AlertTriangle className="h-5 w-5 text-amber-500" />
            Create Your First Rule
          </CardTitle>
          <CardDescription>Rules tell paporg how to organize your documents</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col items-center justify-center py-8 text-center">
            <Plus className="h-12 w-12 text-muted-foreground/50 mb-4" />
            <h3 className="font-semibold text-lg mb-1">No rules configured</h3>
            <p className="text-muted-foreground text-sm max-w-md mb-4">
              Create rules to automatically categorize and organize your documents based on their
              content.
            </p>
            <Button asChild>
              <Link to="/rules/$name" params={{ name: 'new' }}>
                <Plus className="h-4 w-4 mr-2" />
                Create First Rule
              </Link>
            </Button>
          </div>
        </CardContent>
      </Card>
    )
  }

  // All sorted - success state
  if (jobs.length === 0) {
    return (
      <Card className="border-green-200 dark:border-green-800">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <CheckCircle2 className="h-5 w-5 text-green-500" />
            All Organized!
          </CardTitle>
          <CardDescription>All your documents have been categorized by rules</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col items-center justify-center py-8 text-center">
            <CheckCircle2 className="h-12 w-12 text-green-500/50 mb-4" />
            <h3 className="font-semibold text-lg mb-1">Everything is sorted</h3>
            <p className="text-muted-foreground text-sm max-w-md">
              Great job! All your documents match existing rules. New unsorted documents will appear
              here when they arrive.
            </p>
          </div>
        </CardContent>
      </Card>
    )
  }

  // Has unsorted documents
  return (
    <Card className="border-amber-200 dark:border-amber-800">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <AlertTriangle className="h-5 w-5 text-amber-500" />
          Documents Need Attention
        </CardTitle>
        <CardDescription>
          {jobs.length} document{jobs.length !== 1 ? 's' : ''} didn't match any rules
        </CardDescription>
      </CardHeader>
      <CardContent>
        <ScrollArea className="h-[300px]">
          <div className="space-y-2">
            {jobs.map((job) => {
              const isRerunning = rerunningJobIds.has(job.jobId)
              const isIgnoring = ignoringJobIds.has(job.jobId)
              const isDisabled = isRerunning || isIgnoring

              return (
                <div
                  key={job.jobId}
                  className="flex items-center justify-between p-3 rounded-lg border bg-card hover:bg-muted/50 transition-colors"
                >
                  <div className="flex items-center gap-3 min-w-0 flex-1">
                    <FileText className="h-5 w-5 text-muted-foreground shrink-0" />
                    <div className="min-w-0">
                      <p className="font-medium truncate">{job.filename}</p>
                      <p className="text-xs text-muted-foreground">
                        {new Date(job.startedAt).toLocaleDateString()}
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <Button
                      variant="default"
                      size="sm"
                      onClick={() => handleCreateRule(job)}
                      disabled={isDisabled}
                    >
                      <Plus className="h-4 w-4 mr-1" />
                      Create Rule
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleRerunJob(job)}
                      disabled={isDisabled}
                    >
                      {isRerunning ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <RefreshCw className="h-4 w-4" />
                      )}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleIgnoreJob(job)}
                      disabled={isDisabled}
                      title="Ignore this document"
                    >
                      {isIgnoring ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <EyeOff className="h-4 w-4" />
                      )}
                    </Button>
                  </div>
                </div>
              )
            })}
          </div>
        </ScrollArea>

        <div className="mt-4 pt-4 border-t flex items-center justify-between">
          <p className="text-sm text-muted-foreground">
            Create rules to organize these documents, or ignore them if they don't need sorting.
          </p>
          <Badge variant="secondary">{rulesCount} rule{rulesCount !== 1 ? 's' : ''} active</Badge>
        </div>
      </CardContent>

      <CreateRuleFromJobDialog
        open={createRuleDialogOpen}
        onClose={handleCloseDialog}
        job={selectedJob}
      />
    </Card>
  )
}
