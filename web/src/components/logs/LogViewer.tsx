import { useEffect, useRef, useState } from 'react'
import { useSse } from '@/contexts/SseContext'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { LogEntry } from './LogEntry'
import { LogFilter } from './LogFilter'
import {
  Trash2,
  Download,
  Pause,
  Play,
  Search,
  Wifi,
  WifiOff,
  ArrowDown,
} from 'lucide-react'
import type { LogEvent } from '@/types/config'

export function LogViewer() {
  const { logs, isConnected, error, clearLogs } = useSse()
  const [isPaused, setIsPaused] = useState(false)
  const [searchTerm, setSearchTerm] = useState('')
  const [levelFilter, setLevelFilter] = useState<string[]>([])
  const [autoScroll, setAutoScroll] = useState(true)
  const scrollRef = useRef<HTMLDivElement>(null)
  const [displayedLogs, setDisplayedLogs] = useState<LogEvent[]>([])

  // Update displayed logs when not paused
  useEffect(() => {
    if (!isPaused) {
      setDisplayedLogs(logs)
    }
  }, [logs, isPaused])

  // Auto-scroll to bottom
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [displayedLogs, autoScroll])

  // Filter logs
  const filteredLogs = displayedLogs.filter((log) => {
    // Filter by search term
    if (searchTerm) {
      const searchLower = searchTerm.toLowerCase()
      const matches =
        log.message.toLowerCase().includes(searchLower) ||
        log.target.toLowerCase().includes(searchLower)
      if (!matches) return false
    }

    // Filter by level
    if (levelFilter.length > 0 && !levelFilter.includes(log.level)) {
      return false
    }

    return true
  })

  const handleExport = () => {
    const content = filteredLogs
      .map(
        (log) =>
          `[${log.timestamp}] [${log.level}] [${log.target}] ${log.message}`
      )
      .join('\n')

    const blob = new Blob([content], { type: 'text/plain' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `paporg-logs-${new Date().toISOString()}.txt`
    a.click()
    URL.revokeObjectURL(url)
  }

  const handleScrollToBottom = () => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
      setAutoScroll(true)
    }
  }

  const handleScroll = () => {
    if (scrollRef.current) {
      const { scrollTop, scrollHeight, clientHeight } = scrollRef.current
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 50
      setAutoScroll(isAtBottom)
    }
  }

  return (
    <Card>
      <CardHeader className="pb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <CardTitle>Live Logs</CardTitle>
            {isConnected ? (
              <Badge variant="success" className="gap-1">
                <Wifi className="h-3 w-3" />
                Connected
              </Badge>
            ) : (
              <Badge variant="destructive" className="gap-1">
                <WifiOff className="h-3 w-3" />
                {error || 'Disconnected'}
              </Badge>
            )}
          </div>

          <div className="flex items-center gap-2">
            <Badge variant="outline">{filteredLogs.length} entries</Badge>
            <Button
              variant="outline"
              size="icon"
              onClick={() => setIsPaused(!isPaused)}
              title={isPaused ? 'Resume' : 'Pause'}
            >
              {isPaused ? (
                <Play className="h-4 w-4" />
              ) : (
                <Pause className="h-4 w-4" />
              )}
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={handleExport}
              title="Export logs"
            >
              <Download className="h-4 w-4" />
            </Button>
            <Button
              variant="outline"
              size="icon"
              onClick={clearLogs}
              title="Clear logs"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        </div>

        <div className="flex items-center gap-4 mt-4">
          <div className="relative flex-1 max-w-sm">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Search logs..."
              className="pl-9"
            />
          </div>

          <LogFilter
            selectedLevels={levelFilter}
            onChange={setLevelFilter}
          />
        </div>
      </CardHeader>

      <CardContent>
        <div className="relative">
          <ScrollArea
            ref={scrollRef}
            className="h-[500px] rounded-lg border bg-muted/30 p-2 font-mono text-xs"
            onScroll={handleScroll}
          >
            {filteredLogs.length === 0 ? (
              <div className="flex items-center justify-center h-full text-muted-foreground">
                {displayedLogs.length === 0
                  ? 'Waiting for logs...'
                  : 'No logs match the current filter'}
              </div>
            ) : (
              <div className="space-y-0.5">
                {filteredLogs.map((log, index) => (
                  <LogEntry key={index} log={log} searchTerm={searchTerm} />
                ))}
              </div>
            )}
          </ScrollArea>

          {!autoScroll && (
            <Button
              variant="secondary"
              size="sm"
              className="absolute bottom-4 right-4 shadow-lg"
              onClick={handleScrollToBottom}
            >
              <ArrowDown className="h-4 w-4 mr-2" />
              Scroll to bottom
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
