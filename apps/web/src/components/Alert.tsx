import { Alert as UiAlert, AlertDescription } from "@/components/ui/alert";
import { cn } from "@/lib/utils";

export function Alert({
  variant = "error",
  children,
}: {
  variant?: "error" | "success" | "info";
  children: React.ReactNode;
}) {
  return (
    <UiAlert
      variant={variant === "error" ? "destructive" : "default"}
      className={cn(
        variant === "success" && "border-green-200 bg-green-50 text-green-800",
        variant === "info" && "border-border bg-muted/50",
      )}
    >
      <AlertDescription>{children}</AlertDescription>
    </UiAlert>
  );
}
