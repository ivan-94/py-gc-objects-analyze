import { AlertTriangle } from "lucide-react";

import { apiErrorParts } from "@/lib/errors";
import { cn } from "@/lib/utils";

export function Skeleton({ title, className }: { title: string; className?: string }) {
  return (
    <div className={cn("rounded-lg border border-border bg-background p-5 text-sm text-muted-foreground", className)}>
      <div className="mb-3 h-4 w-36 rounded bg-muted" />
      <div className="h-16 rounded bg-muted/70" />
      <span className="sr-only">Loading {title}</span>
    </div>
  );
}

export function EmptyState({ label, className }: { label: string; className?: string }) {
  return <div className={cn("rounded-lg border border-border bg-background p-5 text-sm text-muted-foreground", className)}>{label}</div>;
}

export function ErrorState({ error }: { error: unknown }) {
  const { message, code, nextStep } = apiErrorParts(error);
  return (
    <div className="flex gap-3 rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-900">
      <AlertTriangle className="mt-0.5 shrink-0" size={16} />
      <div className="min-w-0 space-y-1">
        <div className="flex flex-wrap items-center gap-2">
          {code ? <span className="rounded-full border border-red-200 bg-background px-2 py-0.5 font-mono text-xs">{code}</span> : null}
          <strong className="font-semibold">{message}</strong>
        </div>
        {nextStep ? <p className="text-red-800">Next step: {nextStep}</p> : null}
      </div>
    </div>
  );
}
