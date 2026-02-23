import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useUpdateChecker } from '@/hooks/use-update-checker'
import { useToast } from '@/components/ui/use-toast'
import { useEffect } from 'react'
import { Loader2 } from 'lucide-react'

export function UpdateDialog() {
  const { updateInfo, status, error, downloadAndInstall, dismiss } = useUpdateChecker()
  const { toast } = useToast()

  useEffect(() => {
    if (error) {
      toast({
        title: 'Update Error',
        description: error,
        variant: 'destructive',
      })
    }
  }, [error, toast])

  const isDownloading = status === 'downloading'

  return (
    <Dialog open={updateInfo.available} onOpenChange={(open) => { if (!open) dismiss() }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Update Available</DialogTitle>
          <DialogDescription>
            A new version of Paporg is available: <strong>v{updateInfo.version}</strong>
          </DialogDescription>
        </DialogHeader>

        {updateInfo.body && (
          <div className="max-h-60 overflow-y-auto rounded-md border border-black/5 dark:border-white/10 bg-white/40 dark:bg-neutral-900/40 p-3 text-sm whitespace-pre-wrap">
            {updateInfo.body}
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={dismiss} disabled={isDownloading}>
            Later
          </Button>
          <Button onClick={downloadAndInstall} disabled={isDownloading}>
            {isDownloading ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Updating...
              </>
            ) : (
              'Update Now'
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
