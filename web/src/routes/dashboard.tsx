import {
  AlertTriangle,
  CheckCircle2,
  FileText,
  FolderOpen,
  Loader2,
  RefreshCw,
  Wifi,
  WifiOff,
  XCircle,
} from 'lucide-react'
import { AreaChart, Area, XAxis, YAxis, PieChart, Pie, Cell } from 'recharts'
import { useJobsContext } from '@/contexts/JobsContext'
import { useStats } from '@/hooks/useStats'
import { StatCard } from '@/components/dashboard/StatCard'
import { UnsortedWorkflow } from '@/components/dashboard/UnsortedWorkflow'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart'

const timelineChartConfig = {
  count: {
    label: 'Documents',
    color: 'hsl(var(--chart-1))',
  },
} satisfies ChartConfig

const sourceChartConfig = {
  value: {
    label: 'Documents',
  },
} satisfies ChartConfig

export function DashboardPage() {
  const { reconnect } = useJobsContext()

  const {
    totalDocuments,
    processingCount,
    completedCount,
    failedCount,
    rulesCount,
    unsortedCount,
    unsortedJobs,
    isJobsConnected,
    jobsError,
    timelineData,
    sourceData,
  } = useStats()

  // Account for error state and initial load - don't show loading spinner if there's an error
  const isLoading = !isJobsConnected && totalDocuments === 0 && !jobsError
  const hasTimelineData = timelineData.some((d) => d.count > 0)
  const hasSourceData = sourceData.length > 0

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <FolderOpen className="h-8 w-8" />
          <div>
            <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
            <p className="text-muted-foreground">Overview of your document processing</p>
          </div>
        </div>

        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            {isJobsConnected ? (
              <Badge variant="success" className="gap-1">
                <Wifi className="h-3 w-3" />
                Connected
              </Badge>
            ) : (
              <Badge variant="warning" className="gap-1">
                <WifiOff className="h-3 w-3" />
                Reconnecting...
              </Badge>
            )}
          </div>
          {!isJobsConnected && (
            <Button variant="outline" size="sm" onClick={reconnect}>
              <RefreshCw className="h-4 w-4 mr-2" />
              Reconnect
            </Button>
          )}
        </div>
      </div>

      {/* Error Banner */}
      {jobsError && (
        <div className="bg-destructive/10 border border-destructive/20 rounded-lg p-4 text-destructive flex items-center gap-2">
          <XCircle className="h-5 w-5 shrink-0" />
          <p>{jobsError}</p>
          <Button variant="outline" size="sm" onClick={reconnect} className="ml-auto">
            <RefreshCw className="h-4 w-4 mr-2" />
            Retry
          </Button>
        </div>
      )}

      {/* Statistics Grid */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
        <StatCard
          title="Total Documents"
          value={totalDocuments}
          description="All processed documents"
          icon={FileText}
          loading={isLoading}
        />
        <StatCard
          title="Processing"
          value={processingCount}
          description={processingCount > 0 ? 'Currently running' : 'Idle'}
          icon={Loader2}
          loading={isLoading}
          className={processingCount > 0 ? '[&_svg]:animate-spin' : ''}
        />
        <StatCard
          title="Completed"
          value={completedCount}
          description="Successfully processed"
          icon={CheckCircle2}
          loading={isLoading}
        />
        <StatCard
          title="Failed"
          value={failedCount}
          description={failedCount > 0 ? 'Need attention' : 'No failures'}
          icon={XCircle}
          loading={isLoading}
        />
        <StatCard
          title="Rules"
          value={rulesCount}
          description="Classification rules"
          icon={FileText}
          loading={isLoading}
        />
        <StatCard
          title="Unsorted"
          value={unsortedCount}
          description={unsortedCount > 0 ? 'Need rules' : 'All sorted'}
          icon={AlertTriangle}
          loading={isLoading}
        />
      </div>

      {/* Charts Row */}
      <div className="grid gap-4 md:grid-cols-3">
        {/* Processing Timeline - Full Width on smaller screens, 2/3 on larger */}
        <Card className="md:col-span-2">
          <CardHeader>
            <CardTitle>Processing Activity</CardTitle>
            <CardDescription>Documents processed over the last 14 days</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <div className="flex items-center justify-center h-[200px]">
                <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
              </div>
            ) : hasTimelineData ? (
              <ChartContainer config={timelineChartConfig} className="h-[200px] w-full">
                <AreaChart data={timelineData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                  <defs>
                    <linearGradient id="colorCount" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="5%" stopColor="hsl(var(--chart-1))" stopOpacity={0.3} />
                      <stop offset="95%" stopColor="hsl(var(--chart-1))" stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <XAxis
                    dataKey="label"
                    tickLine={false}
                    axisLine={false}
                    tick={{ fontSize: 12 }}
                    tickMargin={8}
                  />
                  <YAxis
                    tickLine={false}
                    axisLine={false}
                    tick={{ fontSize: 12 }}
                    width={30}
                    allowDecimals={false}
                  />
                  <ChartTooltip
                    content={<ChartTooltipContent labelKey="label" />}
                  />
                  <Area
                    type="monotone"
                    dataKey="count"
                    stroke="hsl(var(--chart-1))"
                    strokeWidth={2}
                    fill="url(#colorCount)"
                  />
                </AreaChart>
              </ChartContainer>
            ) : (
              <div className="flex flex-col items-center justify-center h-[200px] text-muted-foreground">
                <FileText className="h-8 w-8 mb-2 opacity-50" />
                <p className="text-sm">No documents processed yet</p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Source Distribution - 1/3 width */}
        <Card>
          <CardHeader>
            <CardTitle>By Source</CardTitle>
            <CardDescription>Document distribution by import source</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <div className="flex items-center justify-center h-[200px]">
                <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
              </div>
            ) : hasSourceData ? (
              <div className="flex flex-col">
                <ChartContainer config={sourceChartConfig} className="h-[140px] mx-auto">
                  <PieChart>
                    <ChartTooltip content={<ChartTooltipContent hideLabel />} />
                    <Pie
                      data={sourceData}
                      dataKey="value"
                      nameKey="name"
                      innerRadius={40}
                      outerRadius={60}
                      strokeWidth={2}
                    >
                      {sourceData.map((entry) => (
                        <Cell key={entry.name} fill={entry.fill} />
                      ))}
                    </Pie>
                  </PieChart>
                </ChartContainer>
                <div className="flex flex-wrap justify-center gap-3 mt-2">
                  {sourceData.map((item) => (
                    <div key={item.name} className="flex items-center gap-1.5">
                      <div
                        className="h-2.5 w-2.5 rounded-full"
                        style={{ backgroundColor: item.fill }}
                      />
                      <span className="text-xs text-muted-foreground">
                        {item.name} ({item.value})
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center h-[200px] text-muted-foreground">
                <FileText className="h-8 w-8 mb-2 opacity-50" />
                <p className="text-sm">No sources yet</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Unsorted Workflow Section */}
      <UnsortedWorkflow
        jobs={unsortedJobs}
        rulesCount={rulesCount}
        hasDocuments={totalDocuments > 0}
      />
    </div>
  )
}
