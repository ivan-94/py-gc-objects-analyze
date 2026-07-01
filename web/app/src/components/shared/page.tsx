import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "@/lib/utils";

export function Page({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return <section className={cn("flex flex-col gap-4", className)} {...props} />;
}

export function PageTitle({ title, actions, meta }: { title: string; actions?: ReactNode; meta?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-3">
      <div className="min-w-0">
        <h1 className="text-2xl font-semibold tracking-normal text-foreground">{title}</h1>
        {meta ? <div className="mt-1 text-sm text-muted-foreground">{meta}</div> : null}
      </div>
      {actions ? <div className="flex flex-wrap items-center justify-end gap-2">{actions}</div> : null}
    </div>
  );
}
