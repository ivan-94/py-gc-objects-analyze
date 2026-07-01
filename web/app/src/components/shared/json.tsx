import { Braces, Copy } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Sheet } from "@/components/ui/sheet";
import { cn } from "@/lib/utils";

export function JsonBlock({ value, className }: { value: unknown; className?: string }) {
  return (
    <pre className={cn("max-w-full overflow-auto rounded-lg border border-border bg-background p-3 text-xs whitespace-pre-wrap", className)}>
      {JSON.stringify(value, null, 2)}
    </pre>
  );
}

export function JsonSheet({ title, value, onClose }: { title: string; value: unknown; onClose: () => void }) {
  const json = JSON.stringify(value, null, 2);
  return (
    <Sheet
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={title}
      description="Raw JSON evidence"
      footer={
        <Button variant="secondary" onClick={() => void navigator.clipboard?.writeText(json)}>
          <Copy size={15} />
          Copy JSON
        </Button>
      }
    >
      <JsonBlock value={value} />
    </Sheet>
  );
}

export function JsonButton({ title, value, onOpen }: { title: string; value: unknown; onOpen: (payload: { title: string; value: unknown }) => void }) {
  return (
    <Button variant="secondary" size="icon" title="Open JSON" onClick={() => onOpen({ title, value })}>
      <Braces size={14} />
    </Button>
  );
}
