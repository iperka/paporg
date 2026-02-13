import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Copy, Share2 } from 'lucide-react'

interface ShareRuleDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  shareText: string
  onCopy: () => Promise<void>
}

export function ShareRuleDialog({
  open,
  onOpenChange,
  shareText,
  onCopy,
}: ShareRuleDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Share2 className="h-5 w-5" />
            Share Rule
          </DialogTitle>
          <DialogDescription>
            Copy this snippet and send it to a teammate to reuse your rule.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-3">
          <Textarea
            value={shareText}
            readOnly
            className="min-h-[220px] font-mono text-xs"
          />
          <p className="text-xs text-muted-foreground">
            Tip: Use Git or a shared config repo for team-wide reuse.
          </p>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
          <Button onClick={onCopy}>
            <Copy className="h-4 w-4 mr-2" />
            Copy share text
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
