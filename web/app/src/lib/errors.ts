import { PygcoApiError } from "@/generated/api-client";

export function errorNextStep(message: string, code?: string, details: Record<string, unknown> = {}) {
  if (typeof details.next_step === "string" && details.next_step.trim()) return details.next_step;
  const lower = message.toLowerCase();
  if (lower.includes("read-only") || lower.includes("not readonly")) {
    return "Use a SELECT or WITH query. Writes are intentionally blocked.";
  }
  if (code === "invalid_filter") return "Adjust the filter value and retry.";
  if (code === "invalid_object_id") return "Copy an object_id from the Objects table or graph and retry.";
  if (code === "query_failed") return "Inspect the query, object id, or snapshot id and retry.";
  return "Retry the action after checking the current input.";
}

export function apiErrorParts(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  const code = error instanceof PygcoApiError ? error.code : undefined;
  const details = error instanceof PygcoApiError ? error.details : {};
  return { message, code, details, nextStep: errorNextStep(message, code, details) };
}
