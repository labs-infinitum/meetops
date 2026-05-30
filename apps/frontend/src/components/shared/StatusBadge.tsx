import type { BotRunStatus } from "@/types"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

const statusConfig: Record<
  BotRunStatus,
  { label: string; variant: "default" | "secondary" | "destructive" | "outline" }
> = {
  queued: { label: "Queued", variant: "secondary" },
  joining: { label: "Joining", variant: "outline" },
  in_meeting: { label: "In meeting", variant: "outline" },
  recording: { label: "Recording", variant: "default" },
  completed: { label: "Completed", variant: "secondary" },
  failed: { label: "Failed", variant: "destructive" },
}

export function StatusBadge({
  status,
  className,
}: {
  status: BotRunStatus
  className?: string
}) {
  const config = statusConfig[status]
  return (
    <Badge variant={config.variant} className={cn(className)}>
      {status === "recording" && (
        <span className="mr-1.5 size-1.5 animate-pulse rounded-full bg-current" />
      )}
      {config.label}
    </Badge>
  )
}
