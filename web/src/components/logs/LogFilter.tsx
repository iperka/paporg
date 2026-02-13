import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

interface LogFilterProps {
  selectedLevels: string[]
  onChange: (levels: string[]) => void
}

const LEVELS = [
  { value: 'ERROR', label: 'Error', className: 'bg-red-500/10 text-red-500 hover:bg-red-500/20' },
  { value: 'WARN', label: 'Warn', className: 'bg-yellow-500/10 text-yellow-500 hover:bg-yellow-500/20' },
  { value: 'INFO', label: 'Info', className: 'bg-blue-500/10 text-blue-500 hover:bg-blue-500/20' },
  { value: 'DEBUG', label: 'Debug', className: 'bg-gray-500/10 text-gray-500 hover:bg-gray-500/20' },
]

export function LogFilter({ selectedLevels, onChange }: LogFilterProps) {
  const toggleLevel = (level: string) => {
    if (selectedLevels.includes(level)) {
      onChange(selectedLevels.filter((l) => l !== level))
    } else {
      onChange([...selectedLevels, level])
    }
  }

  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-muted-foreground">Filter:</span>
      {LEVELS.map((level) => {
        const isActive = selectedLevels.length === 0 || selectedLevels.includes(level.value)
        return (
          <Badge
            key={level.value}
            variant="outline"
            className={cn(
              'cursor-pointer transition-all',
              isActive && level.className,
              !isActive && 'opacity-40 hover:opacity-70'
            )}
            onClick={() => toggleLevel(level.value)}
          >
            {level.label}
          </Badge>
        )
      })}
      {selectedLevels.length > 0 && (
        <button
          onClick={() => onChange([])}
          className="text-xs text-muted-foreground hover:text-foreground underline"
        >
          Clear
        </button>
      )}
    </div>
  )
}
