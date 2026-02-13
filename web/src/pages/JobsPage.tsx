import { useMemo, useState, useCallback } from 'react'
import { AlertTriangle, Briefcase, RefreshCw, Wifi, WifiOff, Calendar, Loader2 } from 'lucide-react'
import { PieChart, Pie, Cell, BarChart, Bar, XAxis, YAxis } from 'recharts'
import { useJobsContext } from '@/contexts/JobsContext'
import { JobsTable } from '@/components/jobs/JobsTable'
import { CreateRuleFromJobDialog } from '@/components/jobs/CreateRuleFromJobDialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart'
import { api } from '@/api'
import { toast } from '@/components/ui/use-toast'
import type { StoredJob } from '@/types/jobs'

const statusChartConfig = {
  processing: {
    label: 'Processing',
    color: 'hsl(var(--chart-warning))',
  },
  completed: {
    label: 'Completed',
    color: 'hsl(var(--chart-success))',
  },
  failed: {
    label: 'Failed',
    color: 'hsl(var(--chart-destructive))',
  },
} satisfies ChartConfig

const categoryChartConfig = {
  value: {
    label: 'Jobs',
    color: 'hsl(var(--chart-1))',
  },
} satisfies ChartConfig

type DateFilter = 'all' | 'today' | 'week' | 'month' | 'last_month'

function getDateFilterRange(filter: DateFilter): { from?: Date; to?: Date } {
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  // End of today: 23:59:59.999
  const endOfToday = new Date(today.getTime() + 24 * 60 * 60 * 1000 - 1)

  switch (filter) {
    case 'today':
      return { from: today, to: endOfToday }
    case 'week': {
      const weekAgo = new Date(today)
      weekAgo.setDate(weekAgo.getDate() - 7)
      return { from: weekAgo, to: endOfToday }
    }
    case 'month': {
      const monthStart = new Date(now.getFullYear(), now.getMonth(), 1)
      return { from: monthStart, to: endOfToday }
    }
    case 'last_month': {
      const lastMonthStart = new Date(now.getFullYear(), now.getMonth() - 1, 1)
      // End of last month: last day at 23:59:59.999
      const lastMonthEnd = new Date(now.getFullYear(), now.getMonth(), 0, 23, 59, 59, 999)
      return { from: lastMonthStart, to: lastMonthEnd }
    }
    default:
      return {}
  }
}

function filterJobsByDate(jobs: StoredJob[], filter: DateFilter): StoredJob[] {
  if (filter === 'all') return jobs

  const { from, to } = getDateFilterRange(filter)
  return jobs.filter((job) => {
    const jobDate = new Date(job.startedAt)
    if (from && jobDate < from) return false
    if (to && jobDate > to) return false
    return true
  })
}

export function JobsPage() {
  const { jobs, isConnected, error, reconnect, processingJobs, completedJobs, failedJobs } =
    useJobsContext()

  const [selectedJob, setSelectedJob] = useState<StoredJob | null>(null)
  const [createRuleDialogOpen, setCreateRuleDialogOpen] = useState(false)
  const [dateFilter, setDateFilter] = useState<DateFilter>('all')
  const [rerunningJobIds, setRerunningJobIds] = useState<Set<string>>(new Set())
  const [isBulkRerunning, setIsBulkRerunning] = useState(false)

  const totalJobs = jobs.size

  // Apply date filter to jobs
  const filteredProcessingJobs = useMemo(
    () => filterJobsByDate(processingJobs, dateFilter),
    [processingJobs, dateFilter]
  )
  const filteredCompletedJobs = useMemo(
    () => filterJobsByDate(completedJobs, dateFilter),
    [completedJobs, dateFilter]
  )
  const filteredFailedJobs = useMemo(
    () => filterJobsByDate(failedJobs, dateFilter),
    [failedJobs, dateFilter]
  )

  // Filter unsorted jobs (category is 'unsorted' or ends with '/unsorted')
  const unsortedJobs = useMemo(() => {
    return filteredCompletedJobs.filter((job) => {
      const category = job.category?.toLowerCase() || ''
      return category === 'unsorted' || category.endsWith('/unsorted')
    })
  }, [filteredCompletedJobs])

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
  }, [])

  const handleRerunAllUnsorted = useCallback(async () => {
    setIsBulkRerunning(true)
    try {
      const result = await api.jobs.rerunUnsorted()
      // Defensive check: default to 0 if count is missing or invalid
      const submitted = typeof result?.count === 'number' ? result.count : 0
      toast({
        title: 'Bulk re-run started',
        description: `${submitted} job${submitted !== 1 ? 's' : ''} queued for reprocessing`,
      })
    } catch (e) {
      toast({
        title: 'Failed to re-run unsorted jobs',
        description: e instanceof Error ? e.message : 'An unknown error occurred',
        variant: 'destructive',
      })
    } finally {
      setIsBulkRerunning(false)
    }
  }, [])

  const statusChartData = useMemo(
    () => [
      {
        name: 'Processing',
        value: filteredProcessingJobs.length,
        fill: 'hsl(var(--chart-warning))',
      },
      {
        name: 'Completed',
        value: filteredCompletedJobs.length,
        fill: 'hsl(var(--chart-success))',
      },
      { name: 'Failed', value: filteredFailedJobs.length, fill: 'hsl(var(--chart-destructive))' },
    ],
    [filteredProcessingJobs.length, filteredCompletedJobs.length, filteredFailedJobs.length]
  )

  const categoryChartData = useMemo(() => {
    const categories = new Map<string, number>()
    filteredCompletedJobs.forEach((job) => {
      const cat = job.category || 'Uncategorized'
      categories.set(cat, (categories.get(cat) || 0) + 1)
    })
    return Array.from(categories.entries())
      .map(([name, value]) => ({ name, value }))
      .sort((a, b) => b.value - a.value)
      .slice(0, 5)
  }, [filteredCompletedJobs])

  const totalStatusCount =
    filteredProcessingJobs.length + filteredCompletedJobs.length + filteredFailedJobs.length

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Briefcase className="h-8 w-8" />
          <div>
            <h1 className="text-3xl font-bold tracking-tight">Jobs</h1>
            <p className="text-muted-foreground">Real-time document processing status</p>
          </div>
        </div>

        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <Calendar className="h-4 w-4 text-muted-foreground" />
            <Select value={dateFilter} onValueChange={(v) => setDateFilter(v as DateFilter)}>
              <SelectTrigger className="w-[140px]">
                <SelectValue placeholder="Date filter" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Time</SelectItem>
                <SelectItem value="today">Today</SelectItem>
                <SelectItem value="week">This Week</SelectItem>
                <SelectItem value="month">This Month</SelectItem>
                <SelectItem value="last_month">Last Month</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="flex items-center gap-2">
            {isConnected ? (
              <Badge variant="success" className="gap-1">
                <Wifi className="h-3 w-3" />
                Connected
              </Badge>
            ) : (
              <Badge variant="destructive" className="gap-1">
                <WifiOff className="h-3 w-3" />
                Disconnected
              </Badge>
            )}
          </div>
          {!isConnected && (
            <Button variant="outline" size="sm" onClick={reconnect}>
              <RefreshCw className="h-4 w-4 mr-2" />
              Reconnect
            </Button>
          )}
        </div>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/20 rounded-lg p-4 text-destructive">
          {error}
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Status Overview</CardTitle>
            <CardDescription>Current job distribution</CardDescription>
          </CardHeader>
          <CardContent>
            {totalStatusCount > 0 ? (
              <ChartContainer
                config={statusChartConfig}
                className="mx-auto aspect-square max-h-[250px]"
              >
                <PieChart>
                  <ChartTooltip content={<ChartTooltipContent hideLabel />} />
                  <Pie
                    data={statusChartData}
                    dataKey="value"
                    nameKey="name"
                    innerRadius={60}
                    outerRadius={80}
                    strokeWidth={5}
                  >
                    {statusChartData.map((entry) => (
                      <Cell key={`status-${entry.name}`} fill={entry.fill} />
                    ))}
                  </Pie>
                  <text
                    x="50%"
                    y="50%"
                    textAnchor="middle"
                    dominantBaseline="middle"
                    className="fill-foreground text-3xl font-bold"
                  >
                    {totalStatusCount}
                  </text>
                  <text
                    x="50%"
                    y="58%"
                    textAnchor="middle"
                    dominantBaseline="middle"
                    className="fill-muted-foreground text-sm"
                  >
                    Total
                  </text>
                </PieChart>
              </ChartContainer>
            ) : (
              <div className="flex items-center justify-center h-[250px] text-muted-foreground">
                No jobs yet
              </div>
            )}
            <div className="flex justify-center gap-6 mt-4">
              <div className="flex items-center gap-2">
                <div
                  className="h-3 w-3 rounded-full"
                  style={{ backgroundColor: 'hsl(var(--chart-warning))' }}
                />
                <span className="text-sm text-muted-foreground">
                  Processing ({filteredProcessingJobs.length})
                </span>
              </div>
              <div className="flex items-center gap-2">
                <div
                  className="h-3 w-3 rounded-full"
                  style={{ backgroundColor: 'hsl(var(--chart-success))' }}
                />
                <span className="text-sm text-muted-foreground">
                  Completed ({filteredCompletedJobs.length})
                </span>
              </div>
              <div className="flex items-center gap-2">
                <div
                  className="h-3 w-3 rounded-full"
                  style={{ backgroundColor: 'hsl(var(--chart-destructive))' }}
                />
                <span className="text-sm text-muted-foreground">
                  Failed ({filteredFailedJobs.length})
                </span>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Jobs by Category</CardTitle>
            <CardDescription>Top document categories</CardDescription>
          </CardHeader>
          <CardContent>
            {categoryChartData.length > 0 ? (
              <ChartContainer config={categoryChartConfig} className="h-[250px]">
                <BarChart data={categoryChartData} layout="vertical" margin={{ left: 0, right: 16 }}>
                  <XAxis type="number" hide />
                  <YAxis
                    type="category"
                    dataKey="name"
                    tickLine={false}
                    axisLine={false}
                    width={100}
                    tick={{ fontSize: 12 }}
                  />
                  <ChartTooltip content={<ChartTooltipContent />} />
                  <Bar
                    dataKey="value"
                    fill="hsl(var(--chart-1))"
                    radius={[0, 4, 4, 0]}
                    label={{
                      position: 'right',
                      fill: 'hsl(var(--foreground))',
                      fontSize: 12,
                    }}
                  />
                </BarChart>
              </ChartContainer>
            ) : (
              <div className="flex items-center justify-center h-[250px] text-muted-foreground">
                No categorized jobs yet
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <Tabs defaultValue="all" className="space-y-4">
        <TabsList>
          <TabsTrigger value="all">
            All
            {totalJobs > 0 && (
              <Badge variant="secondary" className="ml-2">
                {totalJobs}
              </Badge>
            )}
          </TabsTrigger>
          <TabsTrigger value="unsorted" className="gap-1">
            <AlertTriangle className="h-4 w-4 text-amber-500" />
            Unsorted
            {unsortedJobs.length > 0 && (
              <Badge className="ml-1 bg-amber-500 text-white hover:bg-amber-600">
                {unsortedJobs.length}
              </Badge>
            )}
          </TabsTrigger>
          <TabsTrigger value="processing">
            Processing
            {filteredProcessingJobs.length > 0 && (
              <Badge variant="warning" className="ml-2">
                {filteredProcessingJobs.length}
              </Badge>
            )}
          </TabsTrigger>
          <TabsTrigger value="completed">
            Completed
            {filteredCompletedJobs.length > 0 && (
              <Badge variant="success" className="ml-2">
                {filteredCompletedJobs.length}
              </Badge>
            )}
          </TabsTrigger>
          <TabsTrigger value="failed">
            Failed
            {filteredFailedJobs.length > 0 && (
              <Badge variant="destructive" className="ml-2">
                {filteredFailedJobs.length}
              </Badge>
            )}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="all">
          <JobsTable
            jobs={[...filteredProcessingJobs, ...filteredCompletedJobs, ...filteredFailedJobs]}
            emptyMessage="No jobs have been processed yet. Drop a document in the input directory to start processing."
            onCreateRule={handleCreateRule}
            onRerun={handleRerunJob}
            rerunningJobIds={rerunningJobIds}
          />
        </TabsContent>

        <TabsContent value="unsorted">
          {unsortedJobs.length > 0 && (
            <div className="mb-4 p-4 bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-900 rounded-lg">
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-start gap-3">
                  <AlertTriangle className="h-5 w-5 text-amber-500 mt-0.5" />
                  <div>
                    <h3 className="font-semibold text-amber-800 dark:text-amber-200">
                      {unsortedJobs.length} document{unsortedJobs.length !== 1 ? 's' : ''} need
                      {unsortedJobs.length === 1 ? 's' : ''} rules
                    </h3>
                    <p className="text-sm text-amber-700 dark:text-amber-300 mt-1">
                      These documents didn't match any existing rules. Click "Create Rule" to create
                      a categorization rule based on the document's content, or "Re-run All" to
                      reprocess them with updated rules.
                    </p>
                  </div>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleRerunAllUnsorted}
                  disabled={isBulkRerunning}
                  className="gap-1 shrink-0"
                >
                  {isBulkRerunning ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <RefreshCw className="h-4 w-4" />
                  )}
                  Re-run All Unsorted
                </Button>
              </div>
            </div>
          )}
          <JobsTable
            jobs={unsortedJobs}
            emptyMessage="No unsorted documents. All documents have been categorized by rules."
            showCreateRuleButton
            showRerunButton
            onCreateRule={handleCreateRule}
            onRerun={handleRerunJob}
            rerunningJobIds={rerunningJobIds}
          />
        </TabsContent>

        <TabsContent value="processing">
          <JobsTable
            jobs={filteredProcessingJobs}
            emptyMessage="No jobs are currently processing."
          />
        </TabsContent>

        <TabsContent value="completed">
          <JobsTable
            jobs={filteredCompletedJobs}
            emptyMessage="No completed jobs."
            onCreateRule={handleCreateRule}
            onRerun={handleRerunJob}
            showRerunButton
            rerunningJobIds={rerunningJobIds}
          />
        </TabsContent>

        <TabsContent value="failed">
          <JobsTable jobs={filteredFailedJobs} emptyMessage="No failed jobs." />
        </TabsContent>
      </Tabs>

      <CreateRuleFromJobDialog
        open={createRuleDialogOpen}
        onClose={handleCloseDialog}
        job={selectedJob}
      />
    </div>
  )
}
