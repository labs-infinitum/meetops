import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

export function Spinner({ className }: { className?: string }) {
  return <Loader2 className={cn("text-muted-foreground size-5 animate-spin", className)} />;
}

export function PageLoader() {
  return (
    <div className="flex min-h-[40vh] items-center justify-center">
      <div className="text-muted-foreground flex items-center gap-2 text-sm">
        <Loader2 className="size-5 animate-spin" />
        Loading…
      </div>
    </div>
  );
}
