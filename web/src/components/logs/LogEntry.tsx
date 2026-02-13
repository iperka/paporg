import { cn } from '@/lib/utils'
import type { LogEvent } from '@/types/config'

interface LogEntryProps {
  log: LogEvent
  searchTerm?: string
}

export function LogEntry({ log, searchTerm }: LogEntryProps) {
  const levelClass = {
    ERROR: 'text-red-500',
    WARN: 'text-yellow-500',
    INFO: 'text-blue-500',
    DEBUG: 'text-gray-500',
    TRACE: 'text-gray-400',
  }[log.level] || 'text-foreground'

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp)
      const time = date.toLocaleTimeString('en-US', {
        hour12: false,
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })
      const ms = date.getMilliseconds().toString().padStart(3, '0')
      return `${time}.${ms}`
    } catch {
      return timestamp
    }
  }

  const highlightText = (text: string, term: string) => {
    if (!term) return text

    const parts = text.split(new RegExp(`(${escapeRegex(term)})`, 'gi'))
    return parts.map((part, i) =>
      part.toLowerCase() === term.toLowerCase() ? (
        <mark key={i} className="bg-yellow-200 dark:bg-yellow-800 rounded px-0.5">
          {part}
        </mark>
      ) : (
        part
      )
    )
  }

  return (
    <div className="log-entry flex gap-2 py-0.5 px-1 rounded hover:bg-muted/50">
      <span className="text-muted-foreground shrink-0">
        {formatTimestamp(log.timestamp)}
      </span>
      <span className={cn('w-12 shrink-0 font-semibold', levelClass)}>
        {log.level.padEnd(5)}
      </span>
      <span className="text-muted-foreground shrink-0 max-w-48 truncate" title={log.target}>
        [{log.target}]
      </span>
      <span className="flex-1 break-all">
        {searchTerm ? highlightText(log.message, searchTerm) : log.message}
      </span>
    </div>
  )
}

function escapeRegex(string: string) {
  return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
