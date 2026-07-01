export type PageId =
  | "overview"
  | "objects"
  | "types"
  | "modules"
  | "cohorts"
  | "graph"
  | "diff"
  | "findings"
  | "sql"
  | "report";

export type AppSearch = {
  page?: PageId;
  snapshot?: number;
  q?: string;
  type?: string;
  module?: string;
  cohort?: string;
  sort?: string;
  limit?: number;
  offset?: number;
  selected?: string;
  root?: string;
  graphDepth?: number;
  graphLimit?: number;
  graphDirection?: string;
  from?: number;
  to?: number;
  diffState?: string;
};

export type SearchHistoryMode = "replace" | "push";

export type UpdateSearch = (
  patch: Partial<AppSearch>,
  options?: { history?: SearchHistoryMode }
) => void;

export const pages = new Set<PageId>([
  "overview",
  "objects",
  "types",
  "modules",
  "cohorts",
  "graph",
  "diff",
  "findings",
  "sql",
  "report"
]);

export function parseSearchString(value: unknown): string | undefined {
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return typeof value === "string" && value.trim() ? value : undefined;
}

export function parseSearchNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value !== "string" || !value.trim()) return undefined;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

export function normalizeSearch(search: AppSearch): AppSearch {
  return Object.fromEntries(Object.entries(search).filter(([, value]) => value !== undefined && value !== "")) as AppSearch;
}

export function valueOrUndefined(value: string): string | undefined {
  return value.trim() ? value : undefined;
}

export function clampInt(value: number | undefined, fallback: number, min: number, max: number) {
  if (value === undefined || !Number.isFinite(value)) return fallback;
  return Math.max(min, Math.min(max, Math.floor(value)));
}
