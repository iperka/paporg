import { LogViewer } from '@/components/logs/LogViewer'
import { ScrollText } from 'lucide-react'

export function LogsPage() {
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <ScrollText className="h-8 w-8" />
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Logs</h1>
          <p className="text-muted-foreground">
            Real-time log viewer for document processing
          </p>
        </div>
      </div>

      <LogViewer />
    </div>
  )
}
